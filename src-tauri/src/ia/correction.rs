//! Correction par l'IA : réponses aux questions ouvertes, et méthode Feynman
//! (l'élève explique une notion, l'IA compare avec ses notes).
//! Toujours ancré : l'IA ne juge qu'à partir des extraits des notes fournis.

use serde::{Deserialize, Serialize};
use serde_json::{json, Value};

use super::ancrage::{citation_fidele, extrait_cite, Extrait};

// ---------- Correction d'une réponse ouverte ----------

#[derive(Debug, Clone, Deserialize, Serialize, PartialEq)]
pub struct Correction {
    /// Note sur 20.
    pub note: u8,
    /// « juste », « partiel » ou « faux ».
    pub verdict: String,
    pub points_justes: Vec<String>,
    pub manques: Vec<String>,
    pub erreurs: Vec<String>,
    pub conseil: String,
    /// Modèle qui a corrigé (rempli par Révisio, « local:… » pour l'IA locale).
    #[serde(default)]
    pub modele: String,
}

pub fn schema_correction() -> Value {
    json!({
        "type": "object",
        "properties": {
            "note": { "type": "integer", "minimum": 0, "maximum": 20, "description": "Note sur 20 de la réponse de l'élève." },
            "verdict": { "type": "string", "enum": ["juste", "partiel", "faux"] },
            "points_justes": { "type": "array", "items": { "type": "string" }, "description": "Ce que l'élève a correctement dit." },
            "manques": { "type": "array", "items": { "type": "string" }, "description": "Éléments importants attendus mais absents." },
            "erreurs": { "type": "array", "items": { "type": "string" }, "description": "Affirmations fausses ou imprécises, avec la correction." },
            "conseil": { "type": "string", "description": "Un conseil court et bienveillant pour progresser." }
        },
        "required": ["note", "verdict", "points_justes", "manques", "erreurs", "conseil"]
    })
}

pub fn consigne_correction(question: &str, reponse_attendue: &str, extrait: &str, reponse_eleve: &str) -> String {
    format!(
        "Corrige la réponse d'un élève de Première comme le ferait un professeur bienveillant mais exigeant.\n\
         Juge UNIQUEMENT à partir de l'extrait de cours et de la réponse attendue ci-dessous ; une formulation \
         différente mais juste est acceptée.\n\n\
         Question : {question}\n\nRéponse attendue : {reponse_attendue}\n\nExtrait du cours :\n{extrait}\n\n\
         Réponse de l'élève :\n{reponse_eleve}"
    )
}

pub fn valider_correction(c: &Correction) -> Result<(), String> {
    if c.note > 20 {
        return Err("note hors de 0-20".into());
    }
    if !["juste", "partiel", "faux"].contains(&c.verdict.as_str()) {
        return Err("verdict inconnu".into());
    }
    Ok(())
}

// ---------- Méthode Feynman ----------

#[derive(Debug, Clone, Deserialize, Serialize, PartialEq)]
pub struct PointSource {
    pub texte: String,
    pub source: String,
    pub citation: String,
    /// Rempli par Révisio : la citation a-t-elle été retrouvée dans la note ?
    #[serde(default)]
    pub verifie: bool,
}

#[derive(Debug, Clone, Deserialize, Serialize, PartialEq)]
pub struct RetourFeynman {
    pub note: u8,
    pub ce_qui_est_juste: Vec<String>,
    pub ce_qui_manque: Vec<PointSource>,
    pub erreurs: Vec<PointSource>,
    pub conseil: String,
}

pub fn schema_feynman() -> Value {
    let point = json!({
        "type": "object",
        "properties": {
            "texte": { "type": "string", "description": "Le point manquant ou l'erreur, expliqué simplement (avec la correction pour une erreur)." },
            "source": { "type": "string", "description": "Identifiant de l'extrait qui le montre, ex. E2." },
            "citation": { "type": "string", "description": "Phrase de cet extrait recopiée mot pour mot." }
        },
        "required": ["texte", "source", "citation"]
    });
    json!({
        "type": "object",
        "properties": {
            "note": { "type": "integer", "minimum": 0, "maximum": 20, "description": "Qualité de l'explication sur 20 (exactitude, complétude, clarté)." },
            "ce_qui_est_juste": { "type": "array", "items": { "type": "string" } },
            "ce_qui_manque": { "type": "array", "items": point.clone() },
            "erreurs": { "type": "array", "items": point },
            "conseil": { "type": "string" }
        },
        "required": ["note", "ce_qui_est_juste", "ce_qui_manque", "erreurs", "conseil"]
    })
}

pub fn consigne_feynman(notion: &str, explication: &str) -> String {
    format!(
        "Méthode Feynman : l'élève explique avec ses mots la notion « {notion} », comme à quelqu'un qui ne la connaît pas.\n\
         Compare son explication AVEC LES EXTRAITS DE SES NOTES UNIQUEMENT : dis ce qui est juste, ce qui manque \
         d'important, et ce qui est faux ou imprécis (avec la correction). Pour chaque manque ou erreur, cite l'extrait \
         source (E1, E2…) et recopie mot pour mot la phrase qui le montre. Ne reproche pas l'absence de détails \
         qui ne figurent pas dans les extraits.\n\nExplication de l'élève :\n{explication}"
    )
}

/// Marque les citations retrouvées dans les notes ; une note 0-20 est exigée.
pub fn verifier_feynman(retour: &mut RetourFeynman, extraits: &[Extrait]) {
    for p in retour.ce_qui_manque.iter_mut().chain(retour.erreurs.iter_mut()) {
        p.verifie = extrait_cite(extraits, &p.source, &p.citation).is_some_and(|e| citation_fidele(&p.citation, &e.texte));
    }
}

pub fn valider_feynman(r: &RetourFeynman) -> Result<(), String> {
    if r.note > 20 {
        return Err("note hors de 0-20".into());
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn json_de_correction() {
        let c: Correction = serde_json::from_value(json!({
            "note": 14, "verdict": "partiel", "points_justes": ["deux divisions"], "manques": ["haploïdes"],
            "erreurs": [], "conseil": "Précise le nombre de chromosomes."
        }))
        .unwrap();
        assert!(valider_correction(&c).is_ok());
        assert!(valider_correction(&Correction { verdict: "bof".into(), ..c.clone() }).is_err());
        assert!(valider_correction(&Correction { note: 25, ..c }).is_err());
    }

    #[test]
    fn citations_feynman_verifiees() {
        let extraits = vec![Extrait {
            id: "E1".into(),
            note_chemin: "n.md".into(),
            titre_note: "n".into(),
            section: "s".into(),
            texte: "La méiose comprend deux divisions successives sans réplication entre les deux.".into(),
            markdown: String::new(),
        }];
        let mut r: RetourFeynman = serde_json::from_value(json!({
            "note": 12, "ce_qui_est_juste": [], "conseil": "",
            "ce_qui_manque": [{ "texte": "deux divisions", "source": "E1", "citation": "La méiose comprend deux divisions successives" }],
            "erreurs": [{ "texte": "invention", "source": "E1", "citation": "La méiose se produit dans le sang" }]
        }))
        .unwrap();
        verifier_feynman(&mut r, &extraits);
        assert!(r.ce_qui_manque[0].verifie);
        assert!(!r.erreurs[0].verifie);
    }
}
