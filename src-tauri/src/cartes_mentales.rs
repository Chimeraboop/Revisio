//! Cartes mentales d'un chapitre :
//! - « plan du chapitre », construit hors ligne à partir des notes (ordre de la fiche de synthèse,
//!   puis les encadrés de chaque note) ;
//! - carte rédigée par Gemini à partir des extraits, enregistrée pour être consultée hors ligne.
//!
//! Chaque branche garde le chemin de sa note source (un clic l'ouvre).

use rusqlite::{params, Connection, OptionalExtension};
use serde::{Deserialize, Serialize};
use serde_json::{json, Value};

use crate::cartes::titre_lisible;
use crate::ia::ancrage::{extrait_par_id, Extrait};

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct Noeud {
    pub texte: String,
    /// Phrase d'explication (affichée au survol).
    #[serde(default)]
    pub detail: String,
    /// Note source (chemin dans le vault).
    #[serde(default)]
    pub note: Option<String>,
    #[serde(default)]
    pub enfants: Vec<Noeud>,
}

impl Noeud {
    fn new(texte: impl Into<String>, note: Option<String>) -> Self {
        Self { texte: texte.into(), detail: String::new(), note, enfants: Vec::new() }
    }

    pub fn nombre(&self) -> usize {
        1 + self.enfants.iter().map(Noeud::nombre).sum::<usize>()
    }
}

// ---------- Plan du chapitre (sans IA) ----------

/// Sections qui n'apportent rien à une carte (liens vers d'autres notes, légendes de schéma…).
fn section_utile(genre: &str, titre: &str, titre_note: &str) -> bool {
    let t = titre.trim();
    !(genre == "intro"
        || genre == "legendes"
        || t.is_empty()
        || t.to_lowercase().starts_with("notes liées")
        || t.eq_ignore_ascii_case(titre_note))
}

pub fn plan_du_chapitre(conn: &Connection, matiere: &str, chapitre: Option<&str>) -> rusqlite::Result<Option<Noeud>> {
    let mut req = conn.prepare(
        "SELECT n.id, n.chemin, n.nom, n.titre, n.genre FROM notes n
         WHERE n.matiere = ?1 AND n.chapitre IS ?2 AND n.genre NOT IN ('accueil', 'revision', 'schema')
         ORDER BY n.titre",
    )?;
    let notes: Vec<(i64, String, String, String, String)> = req
        .query_map(params![matiere, chapitre], |r| Ok((r.get(0)?, r.get(1)?, r.get(2)?, r.get(3)?, r.get(4)?)))?
        .collect::<rusqlite::Result<_>>()?;
    if notes.is_empty() {
        return Ok(None);
    }
    let sections = |id: i64| -> rusqlite::Result<Vec<(String, String)>> {
        conn.prepare("SELECT genre, titre FROM sections WHERE note_id = ?1 ORDER BY ordre")?
            .query_map([id], |r| Ok((r.get(0)?, r.get(1)?)))?
            .collect()
    };

    // L'ordre des notes suit les titres de la fiche de synthèse (« 1. [[La méiose…]] »).
    let mut ordre_fiche: Vec<String> = Vec::new();
    for (id, _, _, _, genre) in &notes {
        if genre == "fiche_synthese" {
            for (_, titre) in sections(*id)? {
                if let Some(debut) = titre.find("[[") {
                    let cible = titre[debut + 2..].split([']', '|', '#']).next().unwrap_or("").trim().to_lowercase();
                    ordre_fiche.push(cible);
                }
            }
        }
    }
    let rang = |nom: &str, titre: &str| {
        ordre_fiche
            .iter()
            .position(|c| c == &nom.to_lowercase() || c == &titre.to_lowercase())
            .unwrap_or(usize::MAX)
    };
    let mut cours: Vec<_> = notes.iter().filter(|n| n.4 != "fiche_synthese").collect();
    cours.sort_by_key(|n| rang(&n.2, &n.3));

    let fiche = notes.iter().find(|n| n.4 == "fiche_synthese").map(|n| n.1.clone());
    let mut racine = Noeud::new(chapitre.unwrap_or(matiere), fiche);
    for (id, chemin, _, titre, _) in cours {
        let mut n = Noeud::new(titre.clone(), Some(chemin.clone()));
        for (genre, s) in sections(*id)? {
            if section_utile(&genre, &s, titre) {
                n.enfants.push(Noeud::new(titre_lisible(&s), Some(chemin.clone())));
            }
        }
        racine.enfants.push(n);
    }
    Ok(Some(racine))
}

// ---------- Carte rédigée par l'IA ----------

#[derive(Debug, Clone, Deserialize)]
pub struct FeuilleIa {
    pub texte: String,
    #[serde(default)]
    pub source: String,
}

#[derive(Debug, Clone, Deserialize)]
pub struct SousBrancheIa {
    pub texte: String,
    #[serde(default)]
    pub detail: String,
    #[serde(default)]
    pub source: String,
    #[serde(default)]
    pub feuilles: Vec<FeuilleIa>,
}

#[derive(Debug, Clone, Deserialize)]
pub struct BrancheIa {
    pub texte: String,
    #[serde(default)]
    pub detail: String,
    pub source: String,
    #[serde(default)]
    pub sous_branches: Vec<SousBrancheIa>,
}

#[derive(Debug, Clone, Deserialize)]
pub struct CarteIa {
    pub titre: String,
    pub branches: Vec<BrancheIa>,
}

pub const CONSIGNE_SYSTEME: &str = "Tu es un professeur de lycée (classe de Première, France) qui dessine des cartes mentales de révision.
Règles absolues :
- Utilise UNIQUEMENT les informations des extraits fournis (les notes de cours de l'élève). N'ajoute aucune connaissance extérieure.
- Pour chaque branche, indique dans « source » l'identifiant de l'extrait d'où elle vient (ex. « E3 »).
- Textes très courts (mots-clés, 8 mots au plus), en français correct. Garde les formules LaTeX entre $…$.";

pub fn consigne(nom_chapitre: &str) -> String {
    format!(
        "Construis la carte mentale du chapitre « {nom_chapitre} » à partir des extraits : 4 à 7 branches principales \
         (les grandes notions, dans un ordre logique pour apprendre), chacune avec 2 à 5 sous-branches, et sous chaque \
         sous-branche 0 à 4 feuilles (mots-clés, formules, dates, exemples). « detail » : une phrase d'explication \
         facultative (vide sinon). Relie les notions comme dans le cours, sans répéter la même idée."
    )
}

pub fn schema() -> Value {
    let source = json!({ "type": "string", "description": "Identifiant de l'extrait source (ex. E2)." });
    json!({
        "type": "object",
        "properties": {
            "titre": { "type": "string" },
            "branches": {
                "type": "array",
                "items": {
                    "type": "object",
                    "properties": {
                        "texte": { "type": "string" },
                        "detail": { "type": "string" },
                        "source": source,
                        "sous_branches": {
                            "type": "array",
                            "items": {
                                "type": "object",
                                "properties": {
                                    "texte": { "type": "string" },
                                    "detail": { "type": "string" },
                                    "source": source,
                                    "feuilles": {
                                        "type": "array",
                                        "items": {
                                            "type": "object",
                                            "properties": { "texte": { "type": "string" }, "source": source },
                                            "required": ["texte", "source"]
                                        }
                                    }
                                },
                                "required": ["texte", "detail", "source", "feuilles"]
                            }
                        }
                    },
                    "required": ["texte", "detail", "source", "sous_branches"]
                }
            }
        },
        "required": ["titre", "branches"]
    })
}

/// Refuse une carte trop pauvre ou dont les branches ne citent pas un extrait fourni.
pub fn valider(c: &CarteIa, extraits: &[Extrait]) -> Result<(), String> {
    if c.branches.len() < 3 {
        return Err("au moins 3 branches principales".into());
    }
    for (i, b) in c.branches.iter().enumerate() {
        if b.texte.trim().is_empty() {
            return Err(format!("branche {} sans texte", i + 1));
        }
        if extrait_par_id(extraits, &b.source).is_none() {
            return Err(format!("branche « {} » : « source » doit être un extrait fourni (E1, E2…)", b.texte));
        }
        if b.sous_branches.is_empty() {
            return Err(format!("branche « {} » sans sous-branche", b.texte));
        }
    }
    Ok(())
}

/// Transforme la réponse de l'IA en arbre affichable ; une source inconnue donne simplement un nœud sans lien.
pub fn en_noeud(c: CarteIa, extraits: &[Extrait]) -> Noeud {
    let note = |source: &str| extrait_par_id(extraits, source).map(|e| e.note_chemin.clone());
    let mut racine = Noeud::new(c.titre.trim(), None);
    for b in c.branches {
        let mut nb = Noeud { texte: b.texte.trim().into(), detail: b.detail.trim().into(), note: note(&b.source), enfants: vec![] };
        for s in b.sous_branches.into_iter().filter(|s| !s.texte.trim().is_empty()) {
            let mut ns = Noeud { texte: s.texte.trim().into(), detail: s.detail.trim().into(), note: note(&s.source), enfants: vec![] };
            for f in s.feuilles.into_iter().filter(|f| !f.texte.trim().is_empty()) {
                ns.enfants.push(Noeud::new(f.texte.trim(), note(&f.source).or_else(|| ns.note.clone())));
            }
            nb.enfants.push(ns);
        }
        racine.enfants.push(nb);
    }
    racine
}

// ---------- Cartes enregistrées ----------

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct CarteMentale {
    pub id: i64,
    pub date: i64,
    pub matiere: String,
    pub chapitre: Option<String>,
    pub modele: String,
    pub arbre: Noeud,
}

pub fn enregistrer(
    conn: &Connection,
    matiere: &str,
    chapitre: Option<&str>,
    modele: &str,
    arbre: &Noeud,
    date: i64,
) -> rusqlite::Result<CarteMentale> {
    conn.execute(
        "INSERT INTO cartes_mentales(date, matiere, chapitre, modele, arbre) VALUES (?1, ?2, ?3, ?4, ?5)",
        params![date, matiere, chapitre, modele, serde_json::to_string(arbre).unwrap_or_default()],
    )?;
    Ok(CarteMentale {
        id: conn.last_insert_rowid(),
        date,
        matiere: matiere.into(),
        chapitre: chapitre.map(str::to_owned),
        modele: modele.into(),
        arbre: arbre.clone(),
    })
}

fn lire(r: &rusqlite::Row) -> rusqlite::Result<CarteMentale> {
    let arbre: String = r.get(5)?;
    Ok(CarteMentale {
        id: r.get(0)?,
        date: r.get(1)?,
        matiere: r.get(2)?,
        chapitre: r.get(3)?,
        modele: r.get(4)?,
        arbre: serde_json::from_str(&arbre)
            .map_err(|e| rusqlite::Error::FromSqlConversionFailure(5, rusqlite::types::Type::Text, Box::new(e)))?,
    })
}

/// Cartes IA d'un chapitre, la plus récente d'abord.
pub fn lister(conn: &Connection, matiere: &str, chapitre: Option<&str>) -> rusqlite::Result<Vec<CarteMentale>> {
    conn.prepare(
        "SELECT id, date, matiere, chapitre, modele, arbre FROM cartes_mentales
         WHERE matiere = ?1 AND chapitre IS ?2 ORDER BY date DESC",
    )?
    .query_map(params![matiere, chapitre], lire)?
    .collect()
}

pub fn lire_carte(conn: &Connection, id: i64) -> rusqlite::Result<Option<CarteMentale>> {
    conn.query_row("SELECT id, date, matiere, chapitre, modele, arbre FROM cartes_mentales WHERE id = ?1", [id], lire)
        .optional()
}

pub fn supprimer(conn: &Connection, id: i64) -> rusqlite::Result<bool> {
    Ok(conn.execute("DELETE FROM cartes_mentales WHERE id = ?1", [id])? > 0)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{db, indexer};
    use std::path::Path;

    fn extraits() -> Vec<Extrait> {
        (1..=3)
            .map(|i| Extrait {
                id: format!("E{i}"),
                note_chemin: format!("SVT/note{i}.md"),
                titre_note: format!("Note {i}"),
                section: String::new(),
                texte: "texte".into(),
                markdown: "texte".into(),
            })
            .collect()
    }

    fn carte() -> CarteIa {
        let branches: Vec<Value> = (1..=3)
            .map(|i| {
                json!({
                    "texte": format!("Branche {i}"), "detail": "", "source": format!("E{i}"),
                    "sous_branches": [{ "texte": "Sous", "detail": "d", "source": "E9", "feuilles": [{ "texte": "$2n$", "source": "" }] }]
                })
            })
            .collect();
        serde_json::from_value(json!({ "titre": "La méiose", "branches": branches })).unwrap()
    }

    #[test]
    fn carte_ia_validee_et_convertie() {
        let e = extraits();
        assert!(valider(&carte(), &e).is_ok());
        let mut sans_source = carte();
        sans_source.branches[1].source = "E7".into();
        assert!(valider(&sans_source, &e).is_err());
        let mut pauvre = carte();
        pauvre.branches.truncate(2);
        assert!(valider(&pauvre, &e).is_err());

        let n = en_noeud(carte(), &e);
        assert_eq!(n.enfants.len(), 3);
        assert_eq!(n.enfants[1].note.as_deref(), Some("SVT/note2.md"));
        // Source inconnue : pas de lien (et la feuille n'hérite de rien).
        assert_eq!(n.enfants[0].enfants[0].note, None);
        assert_eq!(n.enfants[0].enfants[0].enfants[0].texte, "$2n$");
        assert_eq!(n.nombre(), 1 + 3 * 3);
    }

    #[test]
    fn plan_dans_l_ordre_de_la_fiche() {
        let mut conn = db::ouvrir_en_memoire().unwrap();
        let racine = Path::new(env!("CARGO_MANIFEST_DIR")).join("tests").join("vault_exemple");
        indexer::indexer(&mut conn, &racine).unwrap();
        let plan = plan_du_chapitre(&conn, "Spé SVT", Some("Chapitre 3 - La méiose")).unwrap().unwrap();
        assert_eq!(plan.texte, "Chapitre 3 - La méiose");
        assert!(plan.note.as_deref().is_some_and(|c| c.contains("Fiche de synthèse")));
        assert!(!plan.enfants.is_empty());
        assert!(plan.enfants.iter().all(|n| n.note.is_some()));
        // Ni « Notes liées » ni légendes de schéma.
        assert!(plan.enfants.iter().flat_map(|n| &n.enfants).all(|s| !s.texte.starts_with("Notes liées")));
        assert!(plan_du_chapitre(&conn, "Spé SVT", Some("Chapitre inexistant")).unwrap().is_none());
    }

    #[test]
    fn cartes_enregistrees() {
        let conn = db::ouvrir_en_memoire().unwrap();
        let arbre = en_noeud(carte(), &extraits());
        let c = enregistrer(&conn, "Spé SVT", Some("Méiose"), "m", &arbre, 5).unwrap();
        assert_eq!(lister(&conn, "Spé SVT", Some("Méiose")).unwrap()[0].arbre, arbre);
        assert!(lister(&conn, "Spé SVT", None).unwrap().is_empty());
        assert_eq!(lire_carte(&conn, c.id).unwrap().unwrap().id, c.id);
        assert!(supprimer(&conn, c.id).unwrap());
        assert!(lire_carte(&conn, c.id).unwrap().is_none());
    }
}
