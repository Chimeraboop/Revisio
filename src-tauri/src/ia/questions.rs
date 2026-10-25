//! QCM et questions ouvertes générés par Gemini à partir des extraits d'un chapitre.
//! Chaque question validée devient une carte de révision (genre « qcm » ou « ouverte »).

use rusqlite::{params, Connection};
use serde::{Deserialize, Serialize};
use serde_json::{json, Value};

use super::ancrage::{citation_fidele, extrait_cite, Extrait};
use super::exercices::sans_accents;
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
                        "choix": { "type": "array", "items": { "type": "string" }, "description": "Exactement 4 propositions affirmatives pour un QCM (une seule juste, distracteurs plausibles, sans lettre A/B/C/D devant, jamais une question) ; liste vide pour une question ouverte." },
                        "bonne_reponse": { "type": "integer", "description": "QCM : position de la bonne proposition dans « choix », en comptant à partir de 0 (0 = la première, 3 = la quatrième) ; -1 pour une question ouverte." },
                        "reponse_attendue": { "type": "string", "description": "Question ouverte : réponse modèle complète et concise. QCM : une phrase qui explique pourquoi la bonne proposition est juste (jamais une simple lettre)." },
                        "source": { "type": "string", "description": "Identifiant de l'extrait source, par exemple E3." },
                        "citation": { "type": "string", "description": "Phrase recopiée mot pour mot de l'extrait source (sans son identifiant), qui justifie la réponse." },
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
         les définitions, mécanismes, causes/conséquences et applications plutôt que les détails anecdotiques. \
         Dans un QCM, la bonne proposition est celle que la citation confirme."
    );
    if !deja_posees.is_empty() {
        c += "\nÉvite de reposer ces questions déjà existantes :\n";
        for q in deja_posees.iter().take(40) {
            c += &format!("- {q}\n");
        }
    }
    c
}

/// Répare ce qui peut l'être dans une question de l'IA, puis la valide : la question prête à devenir
/// une carte, ou la raison du rejet.
pub fn preparer(mut q: QuestionIa, extraits: &[Extrait]) -> Result<QuestionIa, String> {
    reparer(&mut q);
    match defaut(&q, extraits) {
        Some(raison) => Err(raison),
        None => Ok(q),
    }
}

/// Petites erreurs de forme fréquentes avec les modèles locaux (qwen3.5:4b, constaté le 25/10/2026).
fn reparer(q: &mut QuestionIa) {
    q.citation = sans_identifiant(&q.citation);
    if q.genre != "qcm" {
        return;
    }
    for c in &mut q.choix {
        *c = c.trim().to_string();
    }
    retirer_etiquettes(&mut q.choix);
    // Justification réduite à une lettre (« B ») : le modèle répondait par lettre. Elle n'explique rien
    // (vidée), mais désigne la bonne proposition : on la préfère à l'indice si la citation la soutient mieux.
    if let Some(lettre) = lettre_seule(&q.reponse_attendue) {
        q.reponse_attendue.clear();
        if q.choix.len() == 4 {
            let s = scores(q);
            let indice = usize::try_from(q.bonne_reponse).ok().filter(|i| *i < 4);
            if indice.is_none_or(|i| s[lettre] > s[i]) {
                q.bonne_reponse = lettre as i64;
            }
        }
    }
}

/// Citation sans l'identifiant de l'extrait recopié devant (« E6, L'eau… », « [E6] L'eau… ») ni guillemets.
fn sans_identifiant(citation: &str) -> String {
    let mut reste = citation.trim();
    if let Some(apres) = reste.trim_start_matches(['[', '(']).strip_prefix(['E', 'e']) {
        let chiffres = apres.len() - apres.trim_start_matches(|c: char| c.is_ascii_digit()).len();
        let suite = &apres[chiffres..];
        if chiffres > 0 && !suite.starts_with(char::is_alphanumeric) {
            reste = suite.trim_start_matches(|c: char| matches!(c, ']' | ')' | ',' | ';' | ':' | '.' | '-' | '—' | '–') || c.is_whitespace());
        }
    }
    reste.trim_matches(|c: char| matches!(c, '«' | '»' | '"' | '“' | '”') || c.is_whitespace()).to_string()
}

/// « A) …, B) …, C) …, D) … » : étiquettes retirées, seulement si les quatre propositions en ont, dans l'ordre.
fn retirer_etiquettes(choix: &mut [String]) {
    let sans: Vec<Option<String>> = choix
        .iter()
        .zip('A'..='D')
        .map(|(c, lettre)| {
            let mut car = c.chars();
            let l = car.next()?.to_ascii_uppercase();
            let sep = car.next()?;
            (l == lettre && matches!(sep, ')' | '.' | ':' | '-')).then(|| car.as_str().trim().to_string())
        })
        .collect();
    if choix.len() == 4 && sans.iter().all(|s| s.as_ref().is_some_and(|s| !s.is_empty())) {
        for (c, s) in choix.iter_mut().zip(sans) {
            *c = s.unwrap();
        }
    }
}

/// Indice désigné par une justification qui n'est qu'une lettre (« A », « (c) », « Réponse B. »).
fn lettre_seule(justification: &str) -> Option<usize> {
    let pas_lettre = |c: char| !c.is_alphanumeric();
    let t = justification.trim_matches(pas_lettre).to_lowercase();
    let t = ["réponse", "reponse", "choix", "proposition"]
        .iter()
        .find_map(|p| t.strip_prefix(p))
        .unwrap_or(&t)
        .trim_matches(pas_lettre);
    let mut car = t.chars();
    match (car.next(), car.next()) {
        (Some(l @ 'a'..='d'), None) => Some(l as usize - 'a' as usize),
        _ => None,
    }
}

/// Mots outils, sans accents (les mots de moins de 3 lettres sont déjà ignorés).
const MOTS_OUTILS: &[&str] = &[
    "les", "des", "une", "aux", "ces", "cet", "cette", "son", "ses", "sur", "leur", "leurs", "elle", "elles", "ils",
    "qui", "que", "quoi", "dont", "est", "sont", "etre", "ont", "avoir", "pas", "plus", "moins", "par", "pour",
    "dans", "avec", "sans", "sous", "entre", "vers", "chez", "tres", "tout", "tous", "toute", "toutes", "lors",
    "comme", "mais", "donc", "car", "ainsi", "aussi", "peut", "fait", "cela", "celle", "celui", "ceux", "quel",
    "quelle", "quels", "quelles", "comment", "pourquoi", "combien", "lequel", "laquelle",
];

/// Mots qui comptent pour comparer une proposition à la citation : en minuscules sans accents,
/// sans mots outils ; les nombres sont gardés même courts (« 4 gamètes »).
fn mots_significatifs(t: &str) -> Vec<String> {
    sans_accents(&t.to_lowercase())
        .split(|c: char| !c.is_alphanumeric())
        .filter(|m| m.chars().any(|c| c.is_ascii_digit()) || (m.chars().count() > 2 && !MOTS_OUTILS.contains(m)))
        .map(str::to_owned)
        .collect()
}

/// Ressemblance de chaque proposition avec la citation : 2 points par mot retrouvé tel quel, 1 point pour un mot
/// de la même famille (4 premières lettres communes : « lumineuse » / « lumière »). Les mots de l'énoncé ne
/// comptent pas (ils n'aident pas à choisir).
fn scores(q: &QuestionIa) -> Vec<u32> {
    let citation = mots_significatifs(&q.citation);
    let enonce = mots_significatifs(&q.enonce);
    let famille = |a: &str, b: &str| {
        let lettres = |m: &str| m.chars().all(char::is_alphabetic) && m.chars().count() >= 5;
        lettres(a) && lettres(b) && a.chars().zip(b.chars()).take_while(|(x, y)| x == y).count() >= 4
    };
    q.choix
        .iter()
        .map(|c| {
            let mut mots = mots_significatifs(c);
            mots.sort();
            mots.dedup();
            mots.iter()
                .filter(|m| !enonce.contains(m))
                .map(|m| {
                    if citation.contains(m) {
                        2
                    } else if citation.iter().any(|x| famille(m, x)) {
                        1
                    } else {
                        0
                    }
                })
                .sum()
        })
        .collect()
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
            if q.choix.iter().any(|c| c.trim_end().ends_with('?')) {
                return Some("une proposition du QCM est une question".into());
            }
            if !(0..4).contains(&q.bonne_reponse) {
                return Some("bonne réponse du QCM invalide".into());
            }
            // La proposition désignée comme juste doit être celle que la citation soutient le mieux.
            let s = scores(q);
            if s.iter().any(|x| *x > s[q.bonne_reponse as usize]) {
                return Some("la bonne réponse désignée ne correspond pas à la citation".into());
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

    /// Notes de photosynthèse (fictives) sur lesquelles qwen3.5:4b a produit des QCM faux le 25/10/2026.
    fn photosynthese() -> Vec<Extrait> {
        let extrait = |id: &str, texte: &str| Extrait {
            id: id.into(),
            note_chemin: "SVT/Photosynthèse.md".into(),
            titre_note: "Photosynthèse".into(),
            section: "Définition".into(),
            texte: texte.into(),
            markdown: texte.into(),
        };
        vec![
            extrait("E1", "La photosynthèse utilise l'énergie de la lumière pour produire de la matière organique à partir du dioxyde de carbone et de l'eau."),
            extrait("E3", "La chlorophylle est le pigment qui absorbe la lumière dans les chloroplastes."),
            extrait("E6", "Au cours de la phase photochimique, l'eau est décomposée et le dioxygène est libéré dans l'atmosphère."),
        ]
    }

    /// Les QCM tels que le modèle local les a écrits (JSON de la réponse).
    fn qcm_du_modele(enonce: &str, choix: [&str; 4], bonne: i64, justification: &str, source: &str, citation: &str) -> QuestionIa {
        serde_json::from_value(json!({
            "type": "qcm", "enonce": enonce, "choix": choix, "bonne_reponse": bonne,
            "reponse_attendue": justification, "source": source, "citation": citation, "notion": "photosynthèse"
        }))
        .unwrap()
    }

    fn energie(bonne: i64, justification: &str) -> QuestionIa {
        qcm_du_modele(
            "Quelle source d'énergie la photosynthèse utilise-t-elle ?",
            ["La lumière du Soleil", "La chaleur de la Terre", "L'énergie chimique du sol", "L'énergie cinétique du vent"],
            bonne,
            justification,
            "E1",
            "La photosynthèse utilise l'énergie de la lumière pour produire de la matière organique",
        )
    }

    fn dioxygene(bonne: i64, justification: &str) -> QuestionIa {
        qcm_du_modele(
            "Lors de la photosynthèse, que devient le dioxygène ?",
            [
                "Le dioxygène est libéré dans l'atmosphère",
                "Le dioxygène est stocké dans l'amidon",
                "Le dioxygène est transformé en glucose",
                "Le dioxygène n'est pas un produit de la photosynthèse",
            ],
            bonne,
            justification,
            "E6",
            "E6, L'eau est décomposée et le dioxygène est libéré",
        )
    }

    #[test]
    fn bonne_reponse_contredite_par_la_citation() {
        let e = photosynthese();
        // Indice 3 = « le vent » alors que la citation parle de lumière : rejetée.
        let r = preparer(energie(3, "Le vent fournit l'énergie."), &e).unwrap_err();
        assert!(r.contains("ne correspond pas à la citation"), "{r}");
        assert!(preparer(energie(0, "La lumière fournit l'énergie."), &e).is_ok());
        // Indice 3 = « n'est pas un produit » alors que la citation dit « libéré » : rejetée.
        assert!(preparer(dioxygene(3, "Le dioxygène n'est pas produit."), &e).is_err());
        assert!(preparer(dioxygene(0, "Il est libéré."), &e).is_ok());
        // Mot de la même famille que la citation (« lumineuse » / « lumière ») : compte un peu.
        let lumineuse = QuestionIa { choix: vec!["L'énergie lumineuse".into(), "Le vent".into(), "Le sol".into(), "La chaleur".into()], ..energie(0, "") };
        assert_eq!(scores(&lumineuse), vec![1, 0, 0, 0]);
        // Aucune proposition n'est soutenue plus qu'une autre (paraphrase) : acceptée telle quelle.
        let paraphrase = QuestionIa { choix: vec!["Les photons".into(), "Le vent".into(), "Le sol".into(), "La chaleur".into()], ..energie(0, "") };
        assert!(preparer(paraphrase, &e).is_ok());
    }

    #[test]
    fn proposition_qui_est_une_question() {
        let q = qcm_du_modele(
            "Quel pigment absorbe la lumière dans les chloroplastes ?",
            ["La chlorophylle", "Quel pigment est responsable de la couleur verte ?", "Le carotène", "L'amidon"],
            0,
            "La chlorophylle absorbe la lumière.",
            "E3",
            "La chlorophylle est le pigment qui absorbe la lumière",
        );
        let r = preparer(q, &photosynthese()).unwrap_err();
        assert!(r.contains("est une question"), "{r}");
    }

    #[test]
    fn justification_reduite_a_une_lettre() {
        let e = photosynthese();
        // La lettre désigne le choix soutenu par la citation : l'indice est corrigé, la lettre retirée.
        let q = preparer(dioxygene(3, "A"), &e).unwrap();
        assert_eq!((q.bonne_reponse, q.reponse_attendue.as_str()), (0, ""));
        // La lettre confirme le mauvais indice : rejetée quand même.
        assert!(preparer(energie(3, "D"), &e).is_err());
        // Indice hors bornes, lettre valide : la lettre sert d'indice.
        assert_eq!(preparer(energie(4, "(a)"), &e).unwrap().bonne_reponse, 0);
        assert_eq!(lettre_seule("Réponse B."), Some(1));
        assert_eq!(lettre_seule(" c) "), Some(2));
        assert_eq!(lettre_seule("A cause de la lumière"), None);
        assert_eq!(lettre_seule("E"), None);
        // Une vraie justification n'est pas touchée.
        assert_eq!(preparer(energie(0, "La lumière fournit l'énergie."), &e).unwrap().reponse_attendue, "La lumière fournit l'énergie.");
    }

    #[test]
    fn citation_et_propositions_nettoyees() {
        let q = preparer(dioxygene(0, "Il est libéré."), &photosynthese()).unwrap();
        assert_eq!(q.citation, "L'eau est décomposée et le dioxygène est libéré");
        assert_eq!(sans_identifiant("[E12] « La chlorophylle est le pigment »"), "La chlorophylle est le pigment");
        assert_eq!(sans_identifiant("(e3) : texte"), "texte");
        assert_eq!(sans_identifiant("Eau et sels minéraux"), "Eau et sels minéraux");
        assert_eq!(sans_identifiant("E. coli est une bactérie"), "E. coli est une bactérie");
        // Étiquettes A) B) C) D) retirées (sinon « A) La lumière » ≠ la réponse attendue de l'élève).
        let etiquetees = QuestionIa {
            choix: vec!["A) La lumière du Soleil".into(), "B) La chaleur".into(), "C. Le sol".into(), "D- Le vent".into()],
            ..energie(0, "")
        };
        assert_eq!(preparer(etiquetees, &photosynthese()).unwrap().choix, vec!["La lumière du Soleil", "La chaleur", "Le sol", "Le vent"]);
        // Une seule proposition commence par une lettre : rien n'est retiré.
        let mut choix = vec!["A. Lavoisier".to_string(), "Pasteur".into(), "Darwin".into(), "Mendel".into()];
        retirer_etiquettes(&mut choix);
        assert_eq!(choix[0], "A. Lavoisier");
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
