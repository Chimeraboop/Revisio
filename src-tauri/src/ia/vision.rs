//! Vision : questions sur un schéma. L'interface rend le dessin Excalidraw en image (PNG) ;
//! Gemini le regarde, avec les légendes et la note de cours liée, et pose des questions dessus.
//! Contrôle : quand le schéma a des légendes écrites, chaque question doit porter sur l'une d'elles.

use rusqlite::{params, Connection, OptionalExtension};
use serde::{Deserialize, Serialize};
use serde_json::{json, Value};

use super::ancrage::{texte_des_extraits, Extrait};
use crate::cartes::titre_lisible;
use crate::parser::texte_brut;

#[derive(Debug, Clone, Serialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct SchemaInfo {
    pub chemin: String,
    pub titre: String,
    pub matiere: Option<String>,
    pub chapitre: Option<String>,
    pub legendes: Vec<String>,
    /// Note de cours dont le schéma illustre le contenu (« X (schéma) » → « X »).
    pub note_liee: Option<String>,
    pub questions_ia: i64,
}

fn lire_info(conn: &Connection, chemin: String, titre: String, matiere: Option<String>, chapitre: Option<String>) -> rusqlite::Result<SchemaInfo> {
    let legendes: Vec<String> = conn
        .query_row(
            "SELECT s.texte FROM sections s JOIN notes n ON n.id = s.note_id WHERE n.chemin = ?1 AND s.genre = 'legendes'",
            [&chemin],
            |r| r.get::<_, String>(0),
        )
        .optional()?
        .map(|t| t.lines().map(str::trim).filter(|l| !l.is_empty()).map(str::to_owned).collect())
        .unwrap_or_default();
    // La note de cours liée : celle qui intègre le schéma (![[… (schéma)]]), sinon celle dont le titre
    // commence comme celui du schéma (« Les étapes de la méiose (schéma) » → « Les étapes de la méiose (…) »).
    let nom: String = conn.query_row("SELECT nom FROM notes WHERE chemin = ?1", [&chemin], |r| r.get(0))?;
    let base = titre.trim_end_matches("(schéma)").trim_end_matches("(Schéma)").trim().to_string();
    let mut note_liee: Option<String> = conn
        .query_row(
            "SELECT n.chemin FROM liens l JOIN notes n ON n.id = l.source_id
             WHERE n.genre NOT IN ('schema', 'fiche_synthese', 'accueil')
               AND (lower(l.cible) = lower(?1) OR lower(l.cible) = lower(?2) OR lower(l.cible) = lower(?1 || '.md'))
             LIMIT 1",
            params![nom, titre],
            |r| r.get(0),
        )
        .optional()?;
    if note_liee.is_none() {
        note_liee = conn
            .query_row(
                "SELECT chemin FROM notes WHERE genre NOT IN ('schema', 'accueil') AND matiere IS ?2
                   AND (titre = ?1 OR titre LIKE ?1 || ' (%')
                 ORDER BY chapitre IS ?3 DESC, length(titre) LIMIT 1",
                params![base, matiere, chapitre],
                |r| r.get(0),
            )
            .optional()?;
    }
    let questions_ia: i64 = conn.query_row(
        "SELECT count(*) FROM cartes WHERE cle LIKE 'ia|schema|%' AND note_chemin = ?1 AND suspendue = 0",
        [&chemin],
        |r| r.get(0),
    )?;
    Ok(SchemaInfo { chemin, titre, matiere, chapitre, legendes, note_liee, questions_ia })
}

/// Schémas d'une matière (ou de toutes), rangés par chapitre puis par titre.
pub fn schemas(conn: &Connection, matiere: Option<&str>) -> rusqlite::Result<Vec<SchemaInfo>> {
    let lignes: Vec<(String, String, Option<String>, Option<String>)> = conn
        .prepare(
            "SELECT chemin, titre, matiere, chapitre FROM notes WHERE genre = 'schema' AND (?1 IS NULL OR matiere = ?1)
             ORDER BY matiere, chapitre, titre",
        )?
        .query_map([matiere], |r| Ok((r.get(0)?, r.get(1)?, r.get(2)?, r.get(3)?)))?
        .collect::<rusqlite::Result<_>>()?;
    lignes.into_iter().map(|(c, t, m, ch)| lire_info(conn, c, t, m, ch)).collect()
}

pub fn schema(conn: &Connection, chemin: &str) -> rusqlite::Result<Option<SchemaInfo>> {
    let ligne: Option<(String, String, Option<String>, Option<String>)> = conn
        .query_row(
            "SELECT chemin, titre, matiere, chapitre FROM notes WHERE chemin = ?1 AND genre = 'schema'",
            [chemin],
            |r| Ok((r.get(0)?, r.get(1)?, r.get(2)?, r.get(3)?)),
        )
        .optional()?;
    ligne.map(|(c, t, m, ch)| lire_info(conn, c, t, m, ch)).transpose()
}

/// Extraits de la note de cours liée (contexte pour l'IA), dans la limite de `limite` caractères.
pub fn extraits_lies(conn: &Connection, info: &SchemaInfo, limite: usize) -> rusqlite::Result<Vec<Extrait>> {
    let Some(chemin) = &info.note_liee else { return Ok(Vec::new()) };
    let lignes: Vec<(String, String, String, String)> = conn
        .prepare(
            "SELECT n.titre, s.titre, s.texte, s.genre FROM sections s JOIN notes n ON n.id = s.note_id
             WHERE n.chemin = ?1 ORDER BY s.ordre",
        )?
        .query_map([chemin], |r| Ok((r.get(0)?, r.get(1)?, r.get(2)?, r.get(3)?)))?
        .collect::<rusqlite::Result<_>>()?;
    let mut extraits = Vec::new();
    let mut taille = 0;
    for (titre_note, section, markdown, genre) in lignes {
        let texte = texte_brut(&markdown);
        if section.to_lowercase().starts_with("notes liées") || genre == "legendes" || texte.split_whitespace().count() < 4 {
            continue;
        }
        if taille + texte.len() > limite {
            break;
        }
        taille += texte.len();
        extraits.push(Extrait {
            id: format!("E{}", extraits.len() + 1),
            note_chemin: chemin.clone(),
            titre_note,
            section: titre_lisible(&section),
            texte: texte.trim().to_string(),
            markdown,
        });
    }
    Ok(extraits)
}

// ---------- Questions ----------

#[derive(Debug, Clone, Deserialize, Serialize, PartialEq)]
pub struct QuestionVision {
    /// « qcm » ou « ouverte ».
    #[serde(rename = "type")]
    pub genre: String,
    pub enonce: String,
    #[serde(default)]
    pub choix: Vec<String>,
    #[serde(default = "moins_un")]
    pub bonne_reponse: i64,
    pub reponse_attendue: String,
    /// La légende (ou la partie du dessin) sur laquelle porte la question.
    #[serde(default)]
    pub legende: String,
}

fn moins_un() -> i64 {
    -1
}

#[derive(Debug, Clone, Deserialize)]
pub struct LotVision {
    pub questions: Vec<QuestionVision>,
}

pub const CONSIGNE_SYSTEME: &str = "Tu es un professeur de lycée (classe de Première, France). Tu regardes un schéma dessiné par l'élève dans ses notes et tu poses des questions de révision dessus.
Règles absolues :
- Appuie-toi UNIQUEMENT sur ce que montre le schéma (formes, flèches, étapes, légendes) et sur les extraits de cours fournis. N'invente rien.
- Chaque question porte sur un élément précis du schéma : recopie dans « legende » le texte de la légende concernée, tel qu'écrit sur le schéma.
- Écris en français correct. Garde les formules LaTeX entre $…$.";

pub fn consigne(info: &SchemaInfo, nombre: usize, deja: &[String]) -> String {
    let mut c = format!(
        "Voici le schéma « {} ». Pose {nombre} questions de révision variées sur ce schéma : rôle d'une structure, \
         ordre des étapes, ce que représente une flèche ou une couleur, légende manquante, comparaison. Mélange des QCM \
         (type « qcm » : 4 propositions dans « choix », « bonne_reponse » = indice de la bonne, « reponse_attendue » = \
         courte justification) et des questions ouvertes (type « ouverte » : « reponse_attendue » = réponse modèle en 1 à \
         3 phrases ; « choix » vide, « bonne_reponse » = -1).",
        info.titre
    );
    if info.legendes.is_empty() {
        c += "\nLe schéma n'a pas de légendes écrites : décris l'élément du dessin dans « legende ».";
    } else {
        c += &format!("\nLégendes écrites sur le schéma :\n- {}", info.legendes.join("\n- "));
    }
    if !deja.is_empty() {
        c += &format!("\nQuestions déjà posées sur ce schéma (n'en refais pas) :\n- {}", deja.join("\n- "));
    }
    c
}

pub fn schema_json() -> Value {
    json!({
        "type": "object",
        "properties": {
            "questions": {
                "type": "array",
                "items": {
                    "type": "object",
                    "properties": {
                        "type": { "type": "string", "enum": ["qcm", "ouverte"] },
                        "enonce": { "type": "string" },
                        "choix": { "type": "array", "items": { "type": "string" } },
                        "bonne_reponse": { "type": "integer" },
                        "reponse_attendue": { "type": "string" },
                        "legende": { "type": "string", "description": "Texte de la légende du schéma concernée." }
                    },
                    "required": ["type", "enonce", "choix", "bonne_reponse", "reponse_attendue", "legende"]
                }
            }
        },
        "required": ["questions"]
    })
}

/// Exposants et indices Unicode ramenés à des caractères ordinaires (¹⁴C → 14C), pour comparer.
fn aplatir(t: &str) -> String {
    const EXPOSANTS: &str = "⁰¹²³⁴⁵⁶⁷⁸⁹";
    const INDICES: &str = "₀₁₂₃₄₅₆₇₈₉";
    t.chars()
        .map(|c| {
            if let Some(i) = EXPOSANTS.chars().position(|x| x == c) {
                char::from(b'0' + i as u8)
            } else if let Some(i) = INDICES.chars().position(|x| x == c) {
                char::from(b'0' + i as u8)
            } else {
                match c {
                    '⁺' | '₊' => '+',
                    '⁻' | '₋' | '−' => '-',
                    _ => c,
                }
            }
        })
        .collect()
}

fn mots(t: &str) -> Vec<String> {
    aplatir(&t.to_lowercase())
        .split(|c: char| !c.is_alphanumeric())
        .filter(|m| m.chars().count() > 1)
        .map(str::to_owned)
        .collect()
}

/// La légende citée correspond-elle à une légende du schéma ?
pub fn correspond(cite: &str, legendes: &[String]) -> bool {
    let c = mots(cite);
    if c.is_empty() {
        return false;
    }
    legendes.iter().any(|l| {
        let m = mots(l);
        if m.is_empty() {
            return false;
        }
        let (a, b) = (c.join(" "), m.join(" "));
        if a.contains(&b) || b.contains(&a) {
            return true;
        }
        let communs = c.iter().filter(|x| m.contains(x)).count();
        communs * 10 >= c.len().min(m.len()) * 7
    })
}

/// Raison de refuser une question (ou `None` si elle est bonne).
pub fn defaut(q: &QuestionVision, legendes: &[String]) -> Option<String> {
    if q.enonce.trim().is_empty() {
        return Some("énoncé vide".into());
    }
    match q.genre.as_str() {
        "qcm" => {
            if q.choix.len() < 3 || q.bonne_reponse < 0 || q.bonne_reponse as usize >= q.choix.len() {
                return Some(format!("QCM mal formé : « {} »", q.enonce));
            }
        }
        "ouverte" => {
            if q.reponse_attendue.trim().is_empty() {
                return Some(format!("réponse attendue manquante : « {} »", q.enonce));
            }
        }
        autre => return Some(format!("type inconnu {autre}")),
    }
    if !legendes.is_empty() && !correspond(&q.legende, legendes) {
        return Some(format!("« {} » ne correspond à aucune légende du schéma", q.legende));
    }
    None
}

/// Questions déjà créées sur ce schéma (pour éviter les doublons).
pub fn deja_posees(conn: &Connection, chemin: &str) -> rusqlite::Result<Vec<String>> {
    conn.prepare("SELECT question FROM cartes WHERE cle LIKE 'ia|schema|%' AND note_chemin = ?1 AND suspendue = 0")?
        .query_map([chemin], |r| r.get(0))?
        .collect()
}

/// Enregistre les questions comme cartes de révision (QCM ou ouvertes) qui affichent le schéma.
pub fn enregistrer(
    conn: &mut Connection,
    info: &SchemaInfo,
    modele: &str,
    extraits: &[Extrait],
    questions: &[QuestionVision],
    maintenant: i64,
) -> rusqlite::Result<usize> {
    let tx = conn.transaction()?;
    tx.execute(
        "INSERT INTO generations_ia(date, usage, modele, matiere, chapitre, sources, resultat) VALUES (?1, 'vision', ?2, ?3, ?4, ?5, ?6)",
        params![
            maintenant,
            modele,
            info.matiere,
            info.chapitre,
            json!([{ "schema": info.chemin }]).to_string(),
            json!(questions).to_string()
        ],
    )?;
    let generation = tx.last_insert_rowid();
    // Extrait affiché à la correction : les légendes du schéma, puis le texte de la note liée.
    let mut extrait = String::new();
    if !info.legendes.is_empty() {
        extrait += &format!("**Légendes du schéma :** {}\n\n", info.legendes.join(" · "));
    }
    extrait += &extraits.iter().map(|e| e.markdown.as_str()).collect::<Vec<_>>().join("\n\n");
    let mut creees = 0;
    for (i, q) in questions.iter().enumerate() {
        let reponse = if q.genre == "qcm" { q.choix[q.bonne_reponse as usize].clone() } else { q.reponse_attendue.clone() };
        let donnees = json!({
            "choix": q.choix,
            "bonneReponse": q.bonne_reponse,
            "justification": q.reponse_attendue,
            "citation": q.legende,
            "notion": titre_lisible(info.titre.trim_end_matches("(schéma)").trim()),
            "modele": modele,
            "schema": info.chemin,
        });
        tx.execute(
            "INSERT INTO cartes(cle, genre, note_chemin, matiere, chapitre, section, question, reponse, donnees, extrait, creee_le)
             VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10, ?11)",
            params![
                format!("ia|schema|{generation}|{i}"),
                q.genre,
                info.chemin,
                info.matiere,
                info.chapitre,
                info.titre,
                q.enonce.trim(),
                reponse,
                donnees.to_string(),
                extrait,
                maintenant
            ],
        )?;
        creees += 1;
    }
    tx.commit()?;
    Ok(creees)
}

/// Texte des extraits pour la consigne (vide si aucune note liée).
pub fn contexte(extraits: &[Extrait]) -> String {
    if extraits.is_empty() {
        String::new()
    } else {
        format!("\n\nExtraits de la note de cours liée :\n\n{}", texte_des_extraits(extraits))
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{cartes, db, indexer};
    use std::path::Path;

    fn q(genre: &str, legende: &str) -> QuestionVision {
        QuestionVision {
            genre: genre.into(),
            enonce: "Que représente cette étape ?".into(),
            choix: if genre == "qcm" { vec!["a".into(), "b".into(), "c".into(), "d".into()] } else { vec![] },
            bonne_reponse: if genre == "qcm" { 2 } else { -1 },
            reponse_attendue: "La séparation des homologues.".into(),
            legende: legende.into(),
        }
    }

    #[test]
    fn legendes_reconnues() {
        let l = vec!["Formation de ¹⁴C (n + ¹⁴N → ¹⁴C + p)".to_string(), "Prophase I".to_string()];
        assert!(correspond("formation de 14C", &l));
        assert!(correspond("Prophase I", &l));
        assert!(correspond("la prophase I", &l));
        assert!(!correspond("Métaphase II", &l));
        assert!(!correspond("", &l));
    }

    #[test]
    fn questions_controlees() {
        let l = vec!["Prophase I".to_string()];
        assert_eq!(defaut(&q("qcm", "Prophase I"), &l), None);
        assert!(defaut(&q("qcm", "Télophase"), &l).is_some(), "légende inventée");
        assert_eq!(defaut(&q("ouverte", "n'importe quelle zone"), &[]), None, "schéma sans légendes");
        let mut mauvais = q("qcm", "Prophase I");
        mauvais.bonne_reponse = 7;
        assert!(defaut(&mauvais, &l).is_some());
    }

    #[test]
    fn schemas_et_cartes() {
        let mut conn = db::ouvrir_en_memoire().unwrap();
        let racine = Path::new(env!("CARGO_MANIFEST_DIR")).join("tests").join("vault_exemple");
        indexer::indexer(&mut conn, &racine).unwrap();
        let liste = schemas(&conn, None).unwrap();
        assert!(!liste.is_empty());
        let s = liste.iter().find(|s| !s.legendes.is_empty()).expect("un schéma légendé");
        let relu = schema(&conn, &s.chemin).unwrap().unwrap();
        assert_eq!(&relu, s);
        let extraits = extraits_lies(&conn, s, 8000).unwrap();
        let questions = vec![q("qcm", &s.legendes[0]), q("ouverte", &s.legendes[0])];
        assert_eq!(enregistrer(&mut conn, s, "m", &extraits, &questions, 1).unwrap(), 2);
        assert_eq!(schema(&conn, &s.chemin).unwrap().unwrap().questions_ia, 2);
        assert_eq!(deja_posees(&conn, &s.chemin).unwrap().len(), 2);
        // Les cartes « ia|… » survivent à une réindexation.
        cartes::synchroniser(&mut conn, 2).unwrap();
        assert_eq!(schema(&conn, &s.chemin).unwrap().unwrap().questions_ia, 2);
    }
}
