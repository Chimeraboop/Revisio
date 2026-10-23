//! QCM et questions ouvertes générés par Gemini à partir des extraits d'un chapitre.
//! Chaque question validée devient une carte de révision (genre « qcm » ou « ouverte »).

use rusqlite::{params, Connection};
use serde::{Deserialize, Serialize};
use serde_json::{json, Value};

use super::ancrage::{citation_fidele, extrait_cite, Extrait};
use crate::cartes::normaliser_cle;

#[derive(Debug, Clone, Deserialize, Serialize, PartialEq)]
pub struct QuestionIa {
    /// « qcm » ou « ouverte ».
    #[serde(rename = "type")]
    pub genre: String,
    pub enonce: String,
    #[serde(default)]
    pub choix: Vec<String>,
    /// Index du bon choix (QCM), −1 sinon.
    #[serde(default = "moins_un")]
    pub bonne_reponse: i64,
    /// Réponse modèle (question ouverte) ou justification (QCM).
    pub reponse_attendue: String,
    pub source: String,
    pub citation: String,
    #[serde(default)]
    pub notion: String,
}

fn moins_un() -> i64 {
    -1
}

#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct LotQuestions {
    pub questions: Vec<QuestionIa>,
}

/// Schéma JSON imposé à l'IA (sortie structurée).
/// `nombre` questions exactement : les petits modèles locaux s'arrêtent sinon après une ou deux questions
/// (Ollama impose le nombre pendant la génération ; Gemini le respecte aussi, vérifié le 20/10/2026).
pub fn schema(nombre: usize) -> Value {
    json!({
        "type": "object",
        "properties": {
            "questions": {
                "type": "array",
                "minItems": nombre,
                "maxItems": nombre,
                "items": {
                    "type": "object",
                    "properties": {
                        "type": { "type": "string", "enum": ["qcm", "ouverte"] },
                        "enonce": { "type": "string", "description": "La question, autonome et compréhensible sans voir l'extrait." },
                        "choix": { "type": "array", "items": { "type": "string" }, "description": "Exactement 4 propositions pour un QCM (une seule juste, distracteurs plausibles) ; liste vide pour une question ouverte." },
                        "bonne_reponse": { "type": "integer", "description": "Index (0 à 3) de la bonne proposition pour un QCM ; -1 pour une question ouverte." },
                        "reponse_attendue": { "type": "string", "description": "Question ouverte : réponse modèle complète et concise. QCM : courte justification." },
                        "source": { "type": "string", "description": "Identifiant de l'extrait source, par exemple E3." },
                        "citation": { "type": "string", "description": "Phrase recopiée mot pour mot de l'extrait source, qui justifie la réponse." },
                        "notion": { "type": "string", "description": "La notion travaillée, en quelques mots." }
                    },
                    "required": ["type", "enonce", "choix", "bonne_reponse", "reponse_attendue", "source", "citation", "notion"]
                }
            }
        },
        "required": ["questions"]
    })
}

/// Consigne propre aux questions (en plus de la consigne d'ancrage).
pub fn consigne(nombre: usize, deja_posees: &[String]) -> String {
    let mut c = format!(
        "Crée {nombre} questions de révision variées sur ces extraits : environ deux tiers de QCM (4 propositions, \
         une seule juste) et un tiers de questions ouvertes courtes. Couvre des notions différentes, privilégie \
         les définitions, mécanismes, causes/conséquences et applications plutôt que les détails anecdotiques."
    );
    if !deja_posees.is_empty() {
        c += "\nÉvite de reposer ces questions déjà existantes :\n";
        for q in deja_posees.iter().take(40) {
            c += &format!("- {q}\n");
        }
    }
    c
}

/// Raison du rejet d'une question, ou `None` si elle est valide.
pub fn defaut(q: &QuestionIa, extraits: &[Extrait]) -> Option<String> {
    if q.enonce.trim().len() < 8 {
        return Some("énoncé trop court".into());
    }
    let Some(extrait) = extrait_cite(extraits, &q.source, &q.citation) else {
        return Some(format!("extrait « {} » inconnu", q.source));
    };
    if !citation_fidele(&q.citation, &extrait.texte) {
        return Some("citation introuvable dans la note (question non ancrée)".into());
    }
    match q.genre.as_str() {
        "qcm" => {
            let distincts: std::collections::HashSet<String> = q.choix.iter().map(|c| normaliser_cle(c)).collect();
            if q.choix.len() != 4 || distincts.len() != 4 || q.choix.iter().any(|c| c.trim().is_empty()) {
                return Some("un QCM doit avoir 4 propositions différentes".into());
            }
            if !(0..4).contains(&q.bonne_reponse) {
                return Some("bonne réponse du QCM invalide".into());
            }
        }
        "ouverte" => {
            if q.reponse_attendue.trim().len() < 3 {
                return Some("réponse attendue manquante".into());
            }
        }
        autre => return Some(format!("type de question inconnu : {autre}")),
    }
    None
}

#[derive(Debug, Default, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ResultatGeneration {
    pub creees: usize,
    pub rejetees: usize,
    /// Raisons des rejets (pour comprendre ce qui a été écarté).
    pub raisons: Vec<String>,
    pub appels: usize,
    /// Modèle qui a répondu (« local:… » pour l'IA locale).
    pub modele: String,
}

/// Enregistre la génération (pour la retrouver hors ligne) et crée une carte par question valide.
pub fn enregistrer(
    conn: &mut Connection,
    modele: &str,
    matiere: &str,
    chapitre: Option<&str>,
    extraits: &[Extrait],
    questions: &[QuestionIa],
    maintenant: i64,
) -> rusqlite::Result<usize> {
    let tx = conn.transaction()?;
    let sources: Vec<Value> =
        extraits.iter().map(|e| json!({ "id": e.id, "note": e.note_chemin, "section": e.section })).collect();
    tx.execute(
        "INSERT INTO generations_ia(date, usage, modele, matiere, chapitre, sources, resultat) VALUES (?1, 'questions', ?2, ?3, ?4, ?5, ?6)",
        params![maintenant, modele, matiere, chapitre, Value::Array(sources).to_string(), json!(questions).to_string()],
    )?;
    let generation = tx.last_insert_rowid();
    let mut creees = 0;
    for (i, q) in questions.iter().enumerate() {
        let Some(e) = extrait_cite(extraits, &q.source, &q.citation) else { continue };
        let reponse = if q.genre == "qcm" { q.choix[q.bonne_reponse as usize].clone() } else { q.reponse_attendue.clone() };
        let donnees = json!({
            "choix": q.choix,
            "bonneReponse": q.bonne_reponse,
            "justification": q.reponse_attendue,
            "citation": q.citation,
            "notion": q.notion,
            "modele": modele,
        });
        tx.execute(
            "INSERT INTO cartes(cle, genre, note_chemin, matiere, chapitre, section, question, reponse, donnees, extrait, creee_le)
             VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10, ?11)",
            params![
                format!("ia|{generation}|{i}"),
                q.genre,
                e.note_chemin,
                matiere,
                chapitre,
                e.section,
                q.enonce.trim(),
                reponse,
                donnees.to_string(),
                e.markdown,
                maintenant
            ],
        )?;
        creees += 1;
    }
    tx.commit()?;
    Ok(creees)
}

/// Énoncés des questions IA déjà créées pour ce chapitre (pour éviter les doublons).
pub fn deja_posees(conn: &Connection, matiere: &str, chapitre: Option<&str>) -> rusqlite::Result<Vec<String>> {
    conn.prepare(
        "SELECT question FROM cartes WHERE cle LIKE 'ia|%' AND genre IN ('qcm', 'ouverte')
         AND matiere = ?1 AND chapitre IS ?2 AND suspendue = 0 ORDER BY id DESC LIMIT 60",
    )?
    .query_map(params![matiere, chapitre], |r| r.get(0))?
    .collect()
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::db;

    fn extraits() -> Vec<Extrait> {
        vec![Extrait {
            id: "E1".into(),
            note_chemin: "SVT/Méiose.md".into(),
            titre_note: "Méiose".into(),
            section: "Définition".into(),
            texte: "La méiose permet de passer d'une cellule diploïde (2n) à 4 gamètes haploïdes (n), au niveau des gonades.".into(),
            markdown: "> [!info]+ Définition\n> La **méiose** permet…".into(),
        }]
    }

    fn qcm() -> QuestionIa {
        QuestionIa {
            genre: "qcm".into(),
            enonce: "Combien de gamètes produit une méiose ?".into(),
            choix: vec!["1".into(), "2".into(), "4".into(), "8".into()],
            bonne_reponse: 2,
            reponse_attendue: "4 gamètes haploïdes".into(),
            source: "E1".into(),
            citation: "passer d'une cellule diploïde (2n) à 4 gamètes haploïdes (n)".into(),
            notion: "méiose".into(),
        }
    }

    #[test]
    fn validation_des_questions() {
        let e = extraits();
        assert_eq!(defaut(&qcm(), &e), None);
        let inventee = QuestionIa { citation: "La méiose se déroule dans le foie des mammifères".into(), ..qcm() };
        assert!(defaut(&inventee, &e).unwrap().contains("citation"));
        let mauvais_choix = QuestionIa { choix: vec!["1".into(), "1".into(), "4".into(), "8".into()], ..qcm() };
        assert!(defaut(&mauvais_choix, &e).is_some());
        let hors_bornes = QuestionIa { bonne_reponse: 4, ..qcm() };
        assert!(defaut(&hors_bornes, &e).is_some());
        let source_inconnue = QuestionIa { source: "E9".into(), ..qcm() };
        assert!(defaut(&source_inconnue, &e).is_some());
        let ouverte = QuestionIa { genre: "ouverte".into(), choix: vec![], bonne_reponse: -1, ..qcm() };
        assert_eq!(defaut(&ouverte, &e), None);
    }

    #[test]
    fn lecture_du_json_et_enregistrement() {
        let lot: LotQuestions = serde_json::from_value(json!({ "questions": [qcm()] })).unwrap();
        let mut conn = db::ouvrir_en_memoire().unwrap();
        let n = enregistrer(&mut conn, "modele-test", "Spé SVT", Some("Méiose"), &extraits(), &lot.questions, 5).unwrap();
        assert_eq!(n, 1);
        let (cle, genre, reponse, extrait): (String, String, String, String) = conn
            .query_row("SELECT cle, genre, reponse, extrait FROM cartes", [], |r| Ok((r.get(0)?, r.get(1)?, r.get(2)?, r.get(3)?)))
            .unwrap();
        assert!(cle.starts_with("ia|"));
        assert_eq!((genre.as_str(), reponse.as_str()), ("qcm", "4"));
        assert!(extrait.contains("méiose"));
        assert_eq!(deja_posees(&conn, "Spé SVT", Some("Méiose")).unwrap(), vec!["Combien de gamètes produit une méiose ?"]);
        // La génération est conservée (réutilisable hors ligne).
        let g: i64 = conn.query_row("SELECT count(*) FROM generations_ia", [], |r| r.get(0)).unwrap();
        assert_eq!(g, 1);
    }
}
