//! Ancrage strict : l'IA ne travaille QUE sur des extraits de tes notes.
//!
//! 1. On choisit les extraits d'un chapitre (fiche de synthèse en premier) et on les numérote (E1, E2…).
//! 2. On demande à Gemini de citer, pour chaque question, l'extrait et la phrase exacte qui la justifient.
//! 3. On vérifie que la phrase citée existe vraiment dans l'extrait : sinon la question est rejetée.

use rusqlite::{params, Connection};
use serde::{Deserialize, Serialize};

use crate::parser::texte_brut;

/// Taille maximale des extraits envoyés en un seul appel (≈ 8 000 jetons).
pub const CARACTERES_PAR_APPEL: usize = 30_000;

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct Extrait {
    /// « E1 », « E2 »… (identifiant cité par l'IA).
    pub id: String,
    pub note_chemin: String,
    pub titre_note: String,
    pub section: String,
    /// Texte de l'extrait (Markdown simplifié, formules LaTeX conservées).
    pub texte: String,
    /// Markdown d'origine, pour l'afficher dans la correction et le carnet d'erreurs.
    pub markdown: String,
}

/// Extraits d'un chapitre (ou des notes d'une matière hors chapitre), fiche de synthèse d'abord.
pub fn extraits_du_chapitre(conn: &Connection, matiere: &str, chapitre: Option<&str>) -> rusqlite::Result<Vec<Extrait>> {
    let mut req = conn.prepare(
        "SELECT n.chemin, n.titre, s.titre, s.texte, s.genre
         FROM sections s JOIN notes n ON n.id = s.note_id
         WHERE n.matiere = ?1 AND (n.chapitre IS ?2)
           AND n.genre NOT IN ('accueil', 'schema', 'revision')
         ORDER BY (n.genre = 'fiche_synthese') DESC, n.chemin, s.ordre",
    )?;
    let lignes = req.query_map(params![matiere, chapitre], ligne_section)?;
    construire(lignes.collect::<rusqlite::Result<Vec<_>>>()?)
}

/// Extraits d'une seule note (source choisie dans une conversation).
pub fn extraits_de_la_note(conn: &Connection, chemin: &str) -> rusqlite::Result<Vec<Extrait>> {
    let mut req = conn.prepare(
        "SELECT n.chemin, n.titre, s.titre, s.texte, s.genre
         FROM sections s JOIN notes n ON n.id = s.note_id
         WHERE n.chemin = ?1 ORDER BY s.ordre",
    )?;
    let lignes = req.query_map([chemin], ligne_section)?;
    construire(lignes.collect::<rusqlite::Result<Vec<_>>>()?)
}

/// Les extraits qui tiennent dans `limite` caractères, les plus proches de la question d'abord (mots en
/// commun, titre compté triple). Les identifiants et l'ordre d'origine sont gardés : une même source
/// garde le même numéro d'une question à l'autre, et l'IA peut s'y référer dans la suite de la conversation.
pub fn pour_la_question(extraits: &[Extrait], question: &str, limite: usize) -> Vec<Extrait> {
    if extraits.iter().map(|e| e.texte.len()).sum::<usize>() <= limite {
        return extraits.to_vec();
    }
    let cles = mots(question);
    let score = |e: &Extrait| {
        let titre = mots(&format!("{} {}", e.titre_note, e.section));
        let texte = mots(&e.texte);
        cles.iter().map(|c| 3 * titre.contains(c) as usize + texte.contains(c) as usize).sum::<usize>()
    };
    let mut rangs: Vec<usize> = (0..extraits.len()).collect();
    rangs.sort_by_key(|&i| std::cmp::Reverse(score(&extraits[i])));
    let mut gardes = Vec::new();
    let mut taille = 0;
    for i in rangs {
        if taille + extraits[i].texte.len() <= limite {
            taille += extraits[i].texte.len();
            gardes.push(i);
        }
    }
    gardes.sort_unstable();
    gardes.into_iter().map(|i| extraits[i].clone()).collect()
}

type LigneSection = (String, String, String, String, String);

fn ligne_section(r: &rusqlite::Row) -> rusqlite::Result<LigneSection> {
    Ok((r.get(0)?, r.get(1)?, r.get(2)?, r.get(3)?, r.get(4)?))
}

/// Notes de méthodologie et fiches de référence (toutes matières) dont le titre contient un des mots.
pub fn extraits_methodologie(conn: &Connection, mots_cles: &[&str]) -> rusqlite::Result<Vec<Extrait>> {
    if mots_cles.is_empty() {
        return Ok(Vec::new());
    }
    let mut req = conn.prepare(
        "SELECT n.chemin, n.titre, s.titre, s.texte, s.genre
         FROM sections s JOIN notes n ON n.id = s.note_id
         WHERE n.genre IN ('methodologie', 'reference')
         ORDER BY n.chemin, s.ordre",
    )?;
    let lignes = req.query_map([], ligne_section)?.collect::<rusqlite::Result<Vec<_>>>()?;
    let choisies = lignes
        .into_iter()
        .filter(|l| {
            let titre = l.1.to_lowercase();
            mots_cles.iter().any(|m| titre.contains(m))
        })
        .collect();
    construire(choisies)
}

/// Extraits de plusieurs chapitres, pris à tour de rôle (pour qu'aucun ne soit oublié)
/// dans la limite de `limite` caractères, puis renumérotés E1, E2…
pub fn melanger(listes: Vec<Vec<Extrait>>, limite: usize) -> Vec<Extrait> {
    let mut iterateurs: Vec<_> = listes.into_iter().map(|l| l.into_iter()).collect();
    let mut choisis = Vec::new();
    let mut taille = 0;
    loop {
        let mut avance = false;
        for it in &mut iterateurs {
            if let Some(e) = it.next() {
                avance = true;
                if taille + e.texte.len() <= limite {
                    taille += e.texte.len();
                    choisis.push(e);
                }
            }
        }
        if !avance {
            break;
        }
    }
    renumeroter(choisis, 1)
}

/// Renumérote les extraits à partir de E{debut}.
pub fn renumeroter(extraits: Vec<Extrait>, debut: usize) -> Vec<Extrait> {
    extraits
        .into_iter()
        .enumerate()
        .map(|(i, mut e)| {
            e.id = format!("E{}", debut + i);
            e
        })
        .collect()
}

fn construire(lignes: Vec<LigneSection>) -> rusqlite::Result<Vec<Extrait>> {
    let mut extraits = Vec::new();
    for (chemin, titre_note, section, markdown, genre) in lignes {
        let texte = texte_brut(&markdown);
        // Les callouts « Notes liées » et les intros « Repère cours » n'apprennent rien.
        let inutile = section.to_lowercase().starts_with("notes liées")
            || (genre == "intro" && texte.trim_start().starts_with("Repère cours"));
        if inutile || texte.split_whitespace().count() < 6 {
            continue;
        }
        extraits.push(Extrait {
            id: format!("E{}", extraits.len() + 1),
            note_chemin: chemin,
            titre_note,
            section: crate::cartes::titre_lisible(&section),
            texte: texte.trim().to_string(),
            markdown,
        });
    }
    Ok(extraits)
}

/// Découpe les extraits en lots qui tiennent dans un appel.
pub fn lots(extraits: &[Extrait]) -> Vec<Vec<Extrait>> {
    let mut lots: Vec<Vec<Extrait>> = Vec::new();
    let mut taille = 0;
    for e in extraits {
        if lots.is_empty() || taille + e.texte.len() > CARACTERES_PAR_APPEL {
            lots.push(Vec::new());
            taille = 0;
        }
        taille += e.texte.len();
        lots.last_mut().unwrap().push(e.clone());
    }
    lots
}

/// Extraits les plus utiles pour une notion : ceux dont le titre ou le texte la mentionnent
/// d'abord, puis les autres, dans la limite d'un appel. Les identifiants sont renumérotés.
pub fn extraits_pour_notion(extraits: &[Extrait], notion: &str) -> Vec<Extrait> {
    let cles: Vec<String> = mots(notion);
    let score = |e: &Extrait| {
        let titre = mots(&format!("{} {}", e.titre_note, e.section));
        let texte = mots(&e.texte);
        cles.iter().map(|c| 3 * titre.contains(c) as usize + texte.contains(c) as usize).sum::<usize>()
    };
    let mut tries: Vec<&Extrait> = extraits.iter().collect();
    tries.sort_by_key(|e| std::cmp::Reverse(score(e)));
    let mut choisis = Vec::new();
    let mut taille = 0;
    for e in tries {
        if taille + e.texte.len() > CARACTERES_PAR_APPEL {
            continue;
        }
        taille += e.texte.len();
        let mut copie = e.clone();
        copie.id = format!("E{}", choisis.len() + 1);
        choisis.push(copie);
    }
    choisis
}

/// Les extraits mis en forme pour la consigne envoyée à l'IA.
pub fn texte_des_extraits(extraits: &[Extrait]) -> String {
    extraits
        .iter()
        .map(|e| format!("[{}] {} — {}\n{}", e.id, e.titre_note, e.section, e.texte))
        .collect::<Vec<_>>()
        .join("\n\n")
}

fn mots(t: &str) -> Vec<String> {
    t.to_lowercase()
        .split(|c: char| !c.is_alphanumeric())
        .filter(|m| m.chars().count() > 2)
        .map(str::to_owned)
        .collect()
}

/// La citation se trouve-t-elle (presque mot pour mot) dans l'extrait ?
/// Au moins 85 % des mots de la citation doivent apparaître dans l'extrait, dans le même ordre approximatif.
pub fn citation_fidele(citation: &str, extrait: &str) -> bool {
    let c = mots(citation);
    if c.len() < 3 {
        return false;
    }
    let e = mots(extrait);
    let mut position = 0;
    let mut trouves = 0;
    for m in &c {
        if let Some(p) = e[position..].iter().position(|x| x == m) {
            position += p + 1;
            trouves += 1;
        }
    }
    trouves * 100 >= c.len() * 85
}

pub fn extrait_par_id<'a>(extraits: &'a [Extrait], id: &str) -> Option<&'a Extrait> {
    let id = id.trim().trim_start_matches('[').trim_end_matches(']');
    extraits.iter().find(|e| e.id.eq_ignore_ascii_case(id))
}

/// L'extrait cité, même quand le modèle a écrit plusieurs identifiants dans « source » (« E2, E16 », fréquent
/// avec les petits modèles locaux) : celui qui contient la citation, sinon le premier identifiant connu.
pub fn extrait_cite<'a>(extraits: &'a [Extrait], source: &str, citation: &str) -> Option<&'a Extrait> {
    if let Some(e) = extrait_par_id(extraits, source) {
        return Some(e);
    }
    let candidats: Vec<&Extrait> = source
        .split(|c: char| !c.is_ascii_alphanumeric())
        .filter(|t| t.len() > 1 && t[..1].eq_ignore_ascii_case("e") && t[1..].chars().all(|c| c.is_ascii_digit()))
        .filter_map(|id| extrait_par_id(extraits, id))
        .collect();
    candidats.iter().find(|e| citation_fidele(citation, &e.texte)).or(candidats.first()).copied()
}

/// Consigne commune : génération uniquement à partir des extraits, en français, niveau 1re.
pub const CONSIGNE_ANCRAGE: &str = "Tu es un professeur de lycée (classe de Première, France) qui prépare des exercices de révision.
Règles absolues :
- Utilise UNIQUEMENT les informations des extraits fournis (les notes de cours de l'élève). N'ajoute aucune connaissance extérieure.
- Pour chaque élément produit, indique l'identifiant de l'extrait source (ex. « E3 ») et recopie MOT POUR MOT, dans « citation », la phrase de cet extrait qui justifie la réponse.
- Si les extraits ne permettent pas de poser une bonne question, pose-en moins plutôt que d'inventer.
- Écris en français correct, de façon claire et précise. Garde les formules LaTeX entre $…$.";

#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct Citation {
    pub source: String,
    pub citation: String,
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{db, indexer};
    use std::path::Path;

    #[test]
    fn citations_verifiees() {
        let extrait = "La méiose permet de passer d'une cellule diploïde (2n) à 4 gamètes haploïdes (n), au niveau des gonades.";
        assert!(citation_fidele("La méiose permet de passer d'une cellule diploïde (2n) à 4 gamètes haploïdes", extrait));
        // Petite reformulation tolérée (un mot changé sur une longue phrase).
        assert!(citation_fidele(
            "la méiose permet de passer d'une cellule diploïde à quatre gamètes haploïdes (n) au niveau des gonades",
            extrait
        ));
        // Phrase inventée : refusée.
        assert!(!citation_fidele("La mitose produit deux cellules filles identiques", extrait));
        assert!(!citation_fidele("méiose", extrait));
    }

    #[test]
    fn source_a_plusieurs_identifiants() {
        let extrait = |id: &str, texte: &str| Extrait {
            id: id.into(),
            note_chemin: format!("{id}.md"),
            titre_note: id.into(),
            section: String::new(),
            texte: texte.into(),
            markdown: texte.into(),
        };
        let e = vec![
            extrait("E2", "La mitose produit deux cellules filles identiques à la cellule mère."),
            extrait("E16", "La deuxième division sépare les chromatides sœurs de chaque chromosome."),
        ];
        let citation = "La deuxième division sépare les chromatides sœurs";
        assert_eq!(extrait_cite(&e, "E16", citation).unwrap().id, "E16");
        // Plusieurs identifiants (et du texte) : celui qui contient la citation.
        assert_eq!(extrait_cite(&e, "E2, E16 (bilan) — **E16**", citation).unwrap().id, "E16");
        // Citation introuvable : le premier identifiant connu (la citation sera refusée ensuite).
        assert_eq!(extrait_cite(&e, "E9, E2", "autre chose").unwrap().id, "E2");
        assert!(extrait_cite(&e, "aucun", citation).is_none());
    }

    #[test]
    fn extraits_fiche_en_premier_et_lots() {
        let mut conn = db::ouvrir_en_memoire().unwrap();
        let racine = Path::new(env!("CARGO_MANIFEST_DIR")).join("tests").join("vault_exemple");
        indexer::indexer(&mut conn, &racine).unwrap();
        let e = extraits_du_chapitre(&conn, "Spé SVT", Some("Chapitre 3 - La méiose")).unwrap();
        assert!(!e.is_empty());
        assert!(e[0].titre_note.starts_with("Fiche de synthèse"));
        assert_eq!(e[0].id, "E1");
        assert!(e.iter().all(|x| !x.section.starts_with("Notes liées")));
        assert_eq!(extrait_par_id(&e, "[E1]").unwrap().id, "E1");
        assert_eq!(lots(&e).len(), 1);
        let texte = texte_des_extraits(&e);
        assert!(texte.starts_with("[E1] Fiche de synthèse"));
    }
}
