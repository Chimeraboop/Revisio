//! Exercices type bac et mode examen : un sujet est rédigé par Gemini à partir des notes,
//! l'élève répond (chronométré), puis la copie est corrigée.
//!
//! Correction mixte :
//! - réponses courtes, choix, dates, nombres : corrigés par Révisio (hors ligne) ;
//!   en maths, la valeur attendue est RECALCULÉE par Révisio à partir de la formule fournie ;
//! - rédactions : corrigées par Gemini, à partir des extraits des notes.

use std::collections::HashMap;

use serde::{Deserialize, Serialize};
use serde_json::{json, Value};

use crate::calcul;

/// Un type d'exercice proposé pour certaines matières.
#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct TypeExercice {
    pub id: &'static str,
    pub nom: &'static str,
    pub description: &'static str,
    /// Consigne donnée à Gemini pour rédiger le sujet.
    #[serde(skip)]
    pub consigne: &'static str,
    pub duree_minutes: u32,
    /// Mots cherchés dans les titres des notes de méthodologie / fiches de référence à joindre.
    #[serde(skip)]
    pub methodologie: &'static [&'static str],
}

const ETUDE_LINEAIRE: TypeExercice = TypeExercice {
    id: "etude_lineaire",
    nom: "Étude linéaire",
    description: "Questions d'analyse sur un passage étudié en classe (procédés, effets, interprétation).",
    consigne: "Crée un exercice de type bac de français sur une étude linéaire présente dans les extraits : recopie le passage \
               étudié (ou ses citations clés) dans « contexte », puis pose 4 à 6 questions d'analyse de type « texte » (repérage \
               d'un procédé, effet produit, interprétation, mouvement du texte), avec les éléments attendus.",
    duree_minutes: 30,
    methodologie: &[],
};

const PLAN_DISSERTATION: TypeExercice = TypeExercice {
    id: "plan_dissertation",
    nom: "Plan de dissertation",
    description: "Un sujet de dissertation : tu proposes problématique et plan détaillé, l'IA les critique selon ta méthodologie.",
    consigne: "Propose un sujet de dissertation littéraire (bac de français) sur l'œuvre ou le parcours des extraits. Une question \
               « texte » demande la problématique, une autre le plan détaillé (2 ou 3 parties, sous-parties, exemples). Les \
               éléments attendus décrivent ce qu'un bon plan doit contenir selon la note de méthodologie de la dissertation.",
    duree_minutes: 40,
    methodologie: &["dissertation"],
};

const ORAL: TypeExercice = TypeExercice {
    id: "oral",
    nom: "Simulation de l'oral",
    description: "Explication linéaire, question de grammaire, puis entretien avec questions de relance (20 min).",
    consigne: "Simule l'oral du bac de français en 3 parties, en suivant la note de méthodologie de l'oral : \
               1) partie « explication » : une question « texte » demandant l'explication linéaire d'un passage des extraits \
               (recopie le passage dans « contexte ») ; 2) partie « grammaire » : une question de grammaire courte sur ce passage \
               (type « texte ») ; 3) partie « entretien » : 3 questions de relance de l'examinateur (type « texte ») sur l'œuvre \
               et la lecture de l'élève, comme dans la note de méthodologie de l'entretien.",
    duree_minutes: 20,
    methodologie: &["oral", "entretien"],
};

const PHRASES_A_COMPLETER: TypeExercice = TypeExercice {
    id: "phrases_a_completer",
    nom: "Phrases à compléter",
    description: "Phrases dans la langue étudiée avec un trou (vocabulaire, grammaire), corrigées par l'app.",
    consigne: "Crée 8 phrases dans la langue étudiée utilisant le vocabulaire et la grammaire des extraits, chacune avec un seul \
               mot ou groupe de mots manquant noté « ___ » dans l'énoncé. Type « court » ; « reponse_attendue » = le mot exact \
               attendu ; ajoute à la fin de l'énoncé, entre parenthèses, une aide en français (le sens du mot manquant).",
    duree_minutes: 10,
    methodologie: &[],
};

const CONNECTEURS: TypeExercice = TypeExercice {
    id: "connecteurs",
    nom: "Connecteurs logiques",
    description: "Choisir le bon connecteur dans des phrases dans la langue étudiée, corrigé par l'app.",
    consigne: "Crée 8 phrases dans la langue étudiée où il manque un connecteur logique (« ___ »), avec 4 propositions de \
               connecteurs (type « choix »), une seule juste, en t'appuyant sur les connecteurs et le vocabulaire des extraits.",
    duree_minutes: 8,
    methodologie: &[],
};

const EXPRESSION_ECRITE: TypeExercice = TypeExercice {
    id: "expression_ecrite",
    nom: "Expression écrite",
    description: "Un sujet d'expression écrite dans la langue étudiée, corrigé par l'IA (langue et contenu).",
    consigne: "Propose un sujet d'expression écrite (type bac, niveau B1) dans la langue étudiée, lié au thème des extraits. \
               Une seule question « texte » (120 à 180 mots attendus). Les éléments attendus listent les idées, le vocabulaire et \
               les structures des extraits qu'une bonne copie devrait réutiliser.",
    duree_minutes: 30,
    methodologie: &["expression"],
};

const ANALYSE_GRAPHIQUE: TypeExercice = TypeExercice {
    id: "analyse_graphique",
    nom: "Analyse de graphique",
    description: "Des données liées au cours sont tracées par l'app ; tu les lis, les décris et les interprètes.",
    consigne: "Crée un exercice d'analyse de graphique : fournis dans « graphique » une ou deux séries de données numériques \
               plausibles et cohérentes avec un phénomène décrit dans les extraits (5 à 12 points par série), puis 3 à 5 questions : \
               lecture d'une valeur (type « nombre », « expression » = la valeur lue), description de l'évolution et \
               interprétation avec le cours (type « texte »). Suis la méthode de la fiche « analyser et exploiter un graphique » \
               si elle est fournie.",
    duree_minutes: 20,
    methodologie: &["graphique"],
};

const EXERCICE_APPLICATION: TypeExercice = TypeExercice {
    id: "exercice_application",
    nom: "Exercice d'application",
    description: "Une situation concrète où il faut appliquer le cours (calculs et justifications).",
    consigne: "Crée un exercice d'application type bac utilisant les définitions, lois et méthodes des extraits, sur une \
               situation concrète. 3 à 5 questions progressives. Pour une question de calcul : type « nombre », les données \
               dans « variables » et le calcul dans « expression » (avec les noms des variables) ; sinon type « texte » \
               ou « choix ».",
    duree_minutes: 30,
    methodologie: &[],
};

const VALEURS_MODIFIEES: TypeExercice = TypeExercice {
    id: "valeurs_modifiees",
    nom: "Exercice à valeurs modifiées",
    description: "Une méthode du cours avec d'autres valeurs ; chaque résultat est recalculé et vérifié par l'app.",
    consigne: "Reprends une méthode ou un exemple des extraits (par ex. discriminant et racines d'un trinôme, forme canonique, \
               coordonnées, colinéarité, dérivée en un point) et crée un exercice avec des valeurs DIFFÉRENTES, choisies pour \
               donner des résultats simples. Mets chaque donnée dans « variables ». Pose 3 à 5 questions de calcul de type \
               « nombre », plus éventuellement une question « texte » de justification.",
    duree_minutes: 25,
    methodologie: &[],
};

const DEFINITIONS: TypeExercice = TypeExercice {
    id: "definitions",
    nom: "Définitions",
    description: "Définir des notions clés du chapitre, corrigé par l'IA.",
    consigne: "Choisis 5 notions clés des extraits et demande d'en donner la définition (type « texte », réponse en 1 à 3 \
               phrases), avec les éléments attendus tirés des extraits.",
    duree_minutes: 15,
    methodologie: &[],
};

const REPERES: TypeExercice = TypeExercice {
    id: "reperes",
    nom: "Repères (dates, lieux, acteurs)",
    description: "Retrouver les repères chronologiques et spatiaux du chapitre, corrigé par l'app.",
    consigne: "Crée 8 questions de repères tirés des extraits (date d'un événement, acteur, lieu, chiffre clé). Type « court », \
               « reponse_attendue » = la réponse exacte la plus courte possible (ex. « 1789 », « 20 juin 1789 », « Necker »).",
    duree_minutes: 10,
    methodologie: &[],
};

const PLAN_COMPOSITION: TypeExercice = TypeExercice {
    id: "plan_composition",
    nom: "Plan de composition",
    description: "Un sujet de composition d'histoire-géographie : introduction et plan, critiqués par l'IA.",
    consigne: "Propose un sujet de composition (bac d'histoire-géographie) sur le chapitre des extraits. Une question « texte » \
               pour l'introduction (définition des termes, problématique), une autre pour le plan détaillé (2 ou 3 parties avec \
               arguments et exemples datés). Les éléments attendus listent les connaissances des extraits à mobiliser.",
    duree_minutes: 40,
    methodologie: &["composition"],
};

pub const EXAMEN: TypeExercice = TypeExercice {
    id: "examen",
    nom: "Mode examen",
    description: "Un sujet chronométré sur un ou plusieurs chapitres, noté sur 20.",
    consigne: "Crée un sujet d'examen type contrôle de Première sur les extraits (qui peuvent couvrir plusieurs chapitres : \
               interroge chacun d'eux) : des questions « choix » (QCM à 4 propositions), des questions « court » (un mot, \
               un nom, une date), des questions « texte » (rédaction courte) et, si le cours contient des calculs, une ou deux \
               questions « nombre ». Répartis les points pour un total de 20.",
    duree_minutes: 30,
    methodologie: &[],
};

const TOUS: &[TypeExercice] = &[
    ETUDE_LINEAIRE,
    PLAN_DISSERTATION,
    ORAL,
    PHRASES_A_COMPLETER,
    CONNECTEURS,
    EXPRESSION_ECRITE,
    ANALYSE_GRAPHIQUE,
    EXERCICE_APPLICATION,
    VALEURS_MODIFIEES,
    DEFINITIONS,
    REPERES,
    PLAN_COMPOSITION,
    EXAMEN,
];

pub fn nom_du_type(id: &str) -> &'static str {
    TOUS.iter().find(|t| t.id == id).map_or("Exercice", |t| t.nom)
}

/// Types d'exercices proposés pour une matière (d'après son nom).
pub fn types_pour(matiere: &str) -> Vec<TypeExercice> {
    let m = matiere.to_lowercase();
    let mut types = if m.contains("français") || m.contains("francais") {
        vec![ETUDE_LINEAIRE, PLAN_DISSERTATION, ORAL]
    } else if m.contains("dnl") {
        vec![DEFINITIONS, REPERES, EXPRESSION_ECRITE]
    } else if m.contains("allemand") || m.contains("anglais") || m.contains("espagnol") {
        vec![PHRASES_A_COMPLETER, CONNECTEURS, EXPRESSION_ECRITE]
    } else if m.contains("maths") || m.contains("mathématiques") {
        vec![VALEURS_MODIFIEES, EXERCICE_APPLICATION]
    } else if m.contains("histoire") || m.contains("géo") {
        vec![DEFINITIONS, REPERES, PLAN_COMPOSITION]
    } else if m.contains("svt") || m.contains("physique") || m.contains("chimie") || m.contains("scientifique") {
        vec![ANALYSE_GRAPHIQUE, EXERCICE_APPLICATION, DEFINITIONS]
    } else {
        vec![EXERCICE_APPLICATION, DEFINITIONS]
    };
    types.push(EXAMEN);
    types
}

pub fn type_par_id(matiere: &str, id: &str) -> Option<TypeExercice> {
    types_pour(matiere).into_iter().find(|t| t.id == id)
}

/// Matière de langue étrangère : les accents (Umlaut…) comptent dans les réponses courtes.
pub fn est_langue(matiere: &str) -> bool {
    let m = matiere.to_lowercase();
    ["allemand", "anglais", "espagnol", "italien"].iter().any(|l| m.contains(l))
}

/// Consigne système : ancrage sur les notes, adapté aux exercices (pas de citation mot pour mot).
pub const CONSIGNE_EXERCICES: &str = "Tu es un professeur de lycée (classe de Première, France) qui rédige des sujets d'entraînement type bac.
Règles absolues :
- Appuie-toi UNIQUEMENT sur les notions, définitions, méthodes, textes et données des extraits fournis (les notes de cours de l'élève). N'ajoute aucune connaissance de cours extérieure. Tu peux inventer une situation ou des valeurs numériques nouvelles pour appliquer le cours.
- Pour chaque question, indique dans « source » l'identifiant de l'extrait sur lequel elle repose (ex. « E3 »).
- Les extraits dont le titre commence par « Méthodologie » ou qui sont des fiches de référence donnent la méthode attendue : respecte-la dans le sujet et dans les éléments attendus.
- Écris les consignes en français (sauf les phrases d'exercice dans une langue étrangère). Garde les formules LaTeX entre $…$.";

/// Consigne système pour corriger une copie.
pub const CONSIGNE_CORRECTION: &str = "Tu es un professeur de lycée (classe de Première, France) qui corrige la copie d'un élève.
Règles absolues :
- Juge UNIQUEMENT d'après les extraits fournis (les notes de cours de l'élève), la réponse modèle et le barème. N'exige rien qui ne soit pas dans les extraits.
- Les extraits de méthodologie ou de référence indiquent la méthode attendue : vérifie qu'elle est respectée.
- Adresse-toi à l'élève en le tutoyant, en français clair. Garde les formules LaTeX entre $…$.";

/// Consigne utilisateur complète pour la génération d'un sujet.
pub fn consigne_generation(t: &TypeExercice, duree_minutes: u32, matiere: &str) -> String {
    format!(
        "{}\n\nMatière : {matiere}. Le sujet doit pouvoir être traité en {duree_minutes} minutes environ par un élève de Première.\n\
         Règles du format :\n\
         - « texte » : rédaction corrigée par un professeur ; donne une réponse modèle dans « reponse_attendue » et le barème dans « elements_attendus ».\n\
         - « court » : un mot, un nom, une date ou un nombre simple, corrigé automatiquement ; « reponse_attendue » = la réponse exacte la plus courte.\n\
         - « choix » : exactement 4 propositions dans « choix » ; « reponse_attendue » = le texte EXACT de la bonne proposition.\n\
         - « nombre » : une seule valeur numérique attendue ; « expression » = la formule qui la calcule à partir des noms de « variables » \
           (opérations + - * / ^, sqrt, abs, ln, exp, parenthèses ; pas de « = ») ; « reponse_attendue » = ta valeur. Les valeurs des \
           variables doivent être exactement celles écrites dans l'énoncé. Pour deux résultats (deux racines…), pose deux questions.\n\
         - Les points de toutes les questions doivent totaliser 20.",
        t.consigne
    )
}

// ---------- Le sujet ----------

#[derive(Debug, Clone, Deserialize, Serialize, PartialEq)]
pub struct Variable {
    pub nom: String,
    pub valeur: f64,
}

#[derive(Debug, Clone, Deserialize, Serialize, PartialEq)]
pub struct Serie {
    pub nom: String,
    /// Points (x, y).
    pub points: Vec<[f64; 2]>,
}

#[derive(Debug, Clone, Deserialize, Serialize, PartialEq)]
pub struct Graphique {
    pub titre: String,
    pub axe_x: String,
    pub axe_y: String,
    pub series: Vec<Serie>,
}

#[derive(Debug, Clone, Deserialize, Serialize, PartialEq)]
pub struct QuestionSujet {
    pub enonce: String,
    /// « texte » (rédaction, corrigée par l'IA), « court » (mot ou date), « choix » (QCM), « nombre » (calcul vérifié).
    pub type_reponse: String,
    #[serde(default)]
    pub choix: Vec<String>,
    #[serde(default)]
    pub reponse_attendue: String,
    /// Pour « nombre » : le calcul à partir des variables, refait par Révisio.
    #[serde(default)]
    pub expression: String,
    pub points: u32,
    #[serde(default)]
    pub elements_attendus: Vec<String>,
    #[serde(default)]
    pub partie: String,
    #[serde(default)]
    pub source: String,
    /// Valeur calculée par Révisio (questions « nombre ») : c'est elle qui sert à corriger.
    #[serde(default)]
    pub valeur: Option<f64>,
    /// Valeur annoncée par l'IA quand elle diffère du calcul de Révisio.
    #[serde(default)]
    pub valeur_ia: Option<String>,
}

#[derive(Debug, Clone, Deserialize, Serialize, PartialEq)]
pub struct Sujet {
    pub titre: String,
    pub consigne: String,
    #[serde(default)]
    pub contexte: String,
    #[serde(default)]
    pub variables: Vec<Variable>,
    #[serde(default)]
    pub graphique: Option<Graphique>,
    pub questions: Vec<QuestionSujet>,
}

impl Sujet {
    fn table_variables(&self) -> HashMap<String, f64> {
        self.variables.iter().map(|v| (v.nom.trim().to_string(), v.valeur)).collect()
    }
}

pub fn schema_sujet() -> Value {
    json!({
        "type": "object",
        "properties": {
            "titre": { "type": "string" },
            "consigne": { "type": "string", "description": "Consigne générale de l'exercice." },
            "contexte": { "type": "string", "description": "Texte, document ou situation de départ (Markdown, LaTeX entre $…$). Vide si inutile." },
            "variables": {
                "type": "array",
                "description": "Données numériques de l'énoncé (exercices de calcul) ; liste vide sinon.",
                "items": {
                    "type": "object",
                    "properties": { "nom": { "type": "string", "description": "Nom court sans espace : a, b, c, xA, m…" }, "valeur": { "type": "number" } },
                    "required": ["nom", "valeur"]
                }
            },
            "graphique": {
                "type": ["object", "null"],
                "description": "Données à tracer pour une analyse de graphique ; null sinon.",
                "properties": {
                    "titre": { "type": "string" },
                    "axe_x": { "type": "string", "description": "Grandeur et unité de l'axe horizontal." },
                    "axe_y": { "type": "string", "description": "Grandeur et unité de l'axe vertical." },
                    "series": {
                        "type": "array",
                        "items": {
                            "type": "object",
                            "properties": {
                                "nom": { "type": "string" },
                                "points": { "type": "array", "items": { "type": "array", "items": { "type": "number" }, "minItems": 2, "maxItems": 2 } }
                            },
                            "required": ["nom", "points"]
                        }
                    }
                },
                "required": ["titre", "axe_x", "axe_y", "series"]
            },
            "questions": {
                "type": "array",
                "items": {
                    "type": "object",
                    "properties": {
                        "enonce": { "type": "string" },
                        "type_reponse": { "type": "string", "enum": ["texte", "court", "choix", "nombre"] },
                        "choix": { "type": "array", "items": { "type": "string" }, "description": "4 propositions pour « choix », vide sinon." },
                        "reponse_attendue": { "type": "string", "description": "Réponse exacte (court, nombre), texte du bon choix (choix), réponse modèle (texte)." },
                        "expression": { "type": "string", "description": "Pour « nombre » : calcul avec les noms des variables, ex. (-b+sqrt(b^2-4*a*c))/(2*a) ; vide sinon." },
                        "points": { "type": "integer", "minimum": 1, "maximum": 20 },
                        "elements_attendus": { "type": "array", "items": { "type": "string" }, "description": "Points que la réponse doit contenir (barème)." },
                        "partie": { "type": "string", "description": "Partie de l'épreuve (ex. explication, grammaire, entretien), vide sinon." },
                        "source": { "type": "string", "description": "Identifiant de l'extrait sur lequel repose la question (ex. E2)." }
                    },
                    "required": ["enonce", "type_reponse", "choix", "reponse_attendue", "expression", "points", "elements_attendus", "partie", "source"]
                }
            }
        },
        "required": ["titre", "consigne", "contexte", "variables", "graphique", "questions"]
    })
}

/// Vérifie le sujet renvoyé par l'IA (sinon il est redemandé) : formats, sources, calculs faisables.
pub fn valider_sujet(s: &Sujet, sources: &[String]) -> Result<(), String> {
    if s.questions.is_empty() {
        return Err("aucune question".into());
    }
    let variables = s.table_variables();
    for (i, q) in s.questions.iter().enumerate() {
        let n = i + 1;
        if q.enonce.trim().is_empty() {
            return Err(format!("question {n} : énoncé vide"));
        }
        let source = q.source.trim().trim_start_matches('[').trim_end_matches(']');
        if !sources.iter().any(|id| id.eq_ignore_ascii_case(source)) {
            return Err(format!("question {n} : « source » doit être l'identifiant d'un extrait fourni (E1, E2…)"));
        }
        match q.type_reponse.as_str() {
            "texte" => {}
            "court" => {
                if q.reponse_attendue.trim().is_empty() {
                    return Err(format!("question {n} : réponse attendue manquante"));
                }
            }
            "choix" => {
                if q.choix.len() < 2 || !q.choix.iter().any(|c| c.trim() == q.reponse_attendue.trim()) {
                    return Err(format!("question {n} : « reponse_attendue » doit être le texte exact d'une des propositions"));
                }
            }
            "nombre" => {
                if let Err(e) = calcul::evaluer(&q.expression, &variables) {
                    return Err(format!("question {n} : « expression » incalculable ({e})"));
                }
            }
            autre => return Err(format!("question {n} : type inconnu {autre}")),
        }
    }
    if let Some(g) = &s.graphique {
        if g.series.is_empty() || g.series.iter().any(|serie| serie.points.len() < 2) {
            return Err("graphique : chaque série doit avoir au moins 2 points".into());
        }
    }
    Ok(())
}

/// Refait chaque calcul : la valeur de Révisio devient la réponse attendue.
/// Si l'IA avait annoncé autre chose, on le garde pour l'afficher à la correction.
pub fn verifier_calculs(s: &mut Sujet) {
    let variables = s.table_variables();
    for q in s.questions.iter_mut().filter(|q| q.type_reponse == "nombre") {
        let Ok(v) = calcul::evaluer(&q.expression, &variables) else { continue };
        let annonce = calcul::lire_nombre(&q.reponse_attendue);
        if !annonce.is_some_and(|a| calcul::proche(a, v, calcul::decimales(&q.reponse_attendue))) {
            q.valeur_ia = Some(q.reponse_attendue.clone());
        }
        q.valeur = Some(v);
        q.reponse_attendue = calcul::afficher(v);
    }
}

// ---------- Correction ----------

#[derive(Debug, Clone, Deserialize, Serialize, PartialEq)]
pub struct ResultatQuestion {
    pub numero: u32,
    pub points: f64,
    pub points_max: f64,
    /// « juste », « partiel », « faux » ou « a_corriger » (rédaction pas encore corrigée).
    pub verdict: String,
    /// Question signalée « mal posée » par l'élève : elle ne compte plus dans la note.
    #[serde(default)]
    pub retiree: bool,
    pub commentaire: String,
    #[serde(default)]
    pub presents: Vec<String>,
    #[serde(default)]
    pub manquants: Vec<String>,
    /// « revisio » (corrigé par l'app, hors ligne) ou « ia ».
    pub correcteur: String,
}

fn resultat(numero: usize, q: &QuestionSujet, ratio: f64, verdict: &str, commentaire: String) -> ResultatQuestion {
    ResultatQuestion {
        numero: numero as u32,
        points: (q.points as f64 * ratio * 2.0).round() / 2.0,
        points_max: q.points as f64,
        verdict: verdict.into(),
        retiree: false,
        commentaire,
        presents: Vec::new(),
        manquants: Vec::new(),
        correcteur: "revisio".into(),
    }
}

/// Corrige tout ce qui peut l'être sans IA ; les rédactions restent « à corriger ».
pub fn corriger_automatiquement(sujet: &Sujet, reponses: &[String], langue: bool) -> Vec<ResultatQuestion> {
    sujet
        .questions
        .iter()
        .enumerate()
        .map(|(i, q)| {
            let n = i + 1;
            let r = reponses.get(i).map(|r| r.trim()).unwrap_or("");
            if q.type_reponse == "texte" {
                return resultat(n, q, 0.0, "a_corriger", String::new());
            }
            if r.is_empty() {
                return resultat(n, q, 0.0, "faux", format!("Pas de réponse. Réponse attendue : {}", q.reponse_attendue));
            }
            match q.type_reponse.as_str() {
                "choix" if r == q.reponse_attendue.trim() => resultat(n, q, 1.0, "juste", String::new()),
                "choix" => resultat(n, q, 0.0, "faux", format!("La bonne réponse était : {}", q.reponse_attendue)),
                "nombre" => {
                    let attendu = q.valeur.or_else(|| calcul::lire_nombre(&q.reponse_attendue));
                    match (calcul::lire_nombre(r), attendu) {
                        (None, _) => resultat(n, q, 0.0, "faux", "Réponse illisible : écris un nombre (ex. 2,5 ou -3/4).".into()),
                        (Some(v), Some(a)) if calcul::proche(v, a, calcul::decimales(r)) => {
                            resultat(n, q, 1.0, "juste", String::new())
                        }
                        (Some(_), Some(a)) => {
                            resultat(n, q, 0.0, "faux", format!("Résultat attendu : {}", calcul::afficher(a)))
                        }
                        (Some(_), None) => resultat(n, q, 0.0, "a_corriger", String::new()),
                    }
                }
                _ => {
                    let (ratio, verdict, commentaire) = comparer_court(r, &q.reponse_attendue, langue);
                    resultat(n, q, ratio, verdict, commentaire)
                }
            }
        })
        .collect()
}

/// Réponse courte : exacte, à un accent ou une faute de frappe près, ou contenue dans une réponse un peu plus longue.
fn comparer_court(eleve: &str, attendu: &str, langue: bool) -> (f64, &'static str, String) {
    let variantes: Vec<&str> = attendu.split([';', '/']).map(str::trim).filter(|v| !v.is_empty()).collect();
    let mut meilleur = (0.0, "faux", format!("Réponse attendue : {attendu}"));
    for v in variantes {
        if eleve.to_lowercase() == v.to_lowercase() {
            return (1.0, "juste", String::new());
        }
        let (ne, na) = (normaliser(eleve, false), normaliser(v, false));
        if na.is_empty() {
            continue;
        }
        if ne == na {
            // En langue, une erreur d'accent ou d'Umlaut compte à moitié.
            if langue && normaliser(eleve, true) != normaliser(v, true) {
                return (0.5, "partiel", format!("Attention aux accents : {v}"));
            }
            return (1.0, "juste", String::new());
        }
        // Hors langues, le singulier ou le pluriel ne compte pas (« cohésine » pour « cohésines »).
        if !langue && singulier(&ne) == singulier(&na) {
            return (1.0, "juste", String::new());
        }
        let mots_e = ne.split(' ').count();
        let mots_a = na.split(' ').count();
        if format!(" {ne} ").contains(&format!(" {na} ")) && mots_e <= mots_a + 4 {
            return (1.0, "juste", String::new());
        }
        let longueur = na.chars().count();
        let tolerance = if longueur <= 5 { 0 } else if longueur <= 10 { 1 } else { 2 };
        if tolerance > 0 && !na.chars().any(|c| c.is_ascii_digit()) && distance(&ne, &na) <= tolerance && meilleur.0 < 0.5 {
            meilleur = (0.5, "partiel", format!("Presque : attention à l'orthographe ({v})"));
        }
    }
    meilleur
}

fn singulier(t: &str) -> String {
    t.split(' ')
        .map(|m| if m.chars().count() > 3 && (m.ends_with('s') || m.ends_with('x')) { &m[..m.len() - 1] } else { m })
        .collect::<Vec<_>>()
        .join(" ")
}

pub(super) fn sans_accents(t: &str) -> String {
    t.chars()
        .flat_map(|c| {
            let r: &str = match c {
                'à' | 'â' | 'ä' | 'á' | 'ã' => "a",
                'é' | 'è' | 'ê' | 'ë' => "e",
                'î' | 'ï' | 'í' => "i",
                'ô' | 'ö' | 'ó' | 'õ' => "o",
                'ù' | 'û' | 'ü' | 'ú' => "u",
                'ç' => "c",
                'ñ' => "n",
                'ÿ' => "y",
                'ß' => "ss",
                'œ' => "oe",
                'æ' => "ae",
                _ => return vec![c],
            };
            r.chars().collect()
        })
        .collect()
}

const ARTICLES: &[&str] = &[
    "le", "la", "les", "l", "un", "une", "des", "du", "de", "d", "der", "die", "das", "den", "dem", "ein", "eine", "einen",
    "the", "a", "an", "el", "los", "las",
];

/// Minuscules, sans ponctuation ni article au début ; sans accents sauf si `garder_accents`.
fn normaliser(t: &str, garder_accents: bool) -> String {
    let t = if garder_accents { t.to_lowercase() } else { sans_accents(&t.to_lowercase()) };
    let mots: Vec<&str> = t.split(|c: char| !c.is_alphanumeric()).filter(|m| !m.is_empty()).collect();
    let debut = mots.iter().take_while(|m| ARTICLES.contains(m)).count().min(mots.len().saturating_sub(1));
    mots[debut..].join(" ")
}

fn distance(a: &str, b: &str) -> usize {
    let a: Vec<char> = a.chars().collect();
    let b: Vec<char> = b.chars().collect();
    let mut precedente: Vec<usize> = (0..=b.len()).collect();
    for i in 1..=a.len() {
        let mut ligne = vec![i; b.len() + 1];
        for j in 1..=b.len() {
            let cout = usize::from(a[i - 1] != b[j - 1]);
            ligne[j] = (precedente[j] + 1).min(ligne[j - 1] + 1).min(precedente[j - 1] + cout);
        }
        precedente = ligne;
    }
    precedente[b.len()]
}

/// Note sur 20 des questions corrigées (les rédactions pas encore corrigées et les questions retirées ne comptent pas).
pub fn note_sur_20(resultats: &[ResultatQuestion]) -> Option<f64> {
    let comptees = resultats.iter().filter(|r| r.verdict != "a_corriger" && !r.retiree);
    let (points, max) = comptees.fold((0.0, 0.0), |(p, m), r| (p + r.points, m + r.points_max));
    (max > 0.0).then(|| (20.0 * points / max * 10.0).round() / 10.0)
}

// ---------- Correction des rédactions par l'IA ----------

#[derive(Debug, Clone, Deserialize, Serialize, PartialEq)]
pub struct CorrectionQuestion {
    /// Numéro de la question (à partir de 1).
    pub numero: u32,
    pub points_obtenus: f64,
    pub commentaire: String,
    #[serde(default)]
    pub elements_presents: Vec<String>,
    #[serde(default)]
    pub elements_manquants: Vec<String>,
}

#[derive(Debug, Clone, Deserialize, Serialize, PartialEq)]
pub struct CorrectionRedactions {
    pub corrections: Vec<CorrectionQuestion>,
    pub appreciation: String,
}

pub fn schema_correction() -> Value {
    json!({
        "type": "object",
        "properties": {
            "corrections": {
                "type": "array",
                "items": {
                    "type": "object",
                    "properties": {
                        "numero": { "type": "integer" },
                        "points_obtenus": { "type": "number", "minimum": 0 },
                        "commentaire": { "type": "string", "description": "Commentaire précis, adressé à l'élève (tu)." },
                        "elements_presents": { "type": "array", "items": { "type": "string" } },
                        "elements_manquants": { "type": "array", "items": { "type": "string" } }
                    },
                    "required": ["numero", "points_obtenus", "commentaire", "elements_presents", "elements_manquants"]
                }
            },
            "appreciation": { "type": "string", "description": "Appréciation générale de la copie, bienveillante et précise, adressée à l'élève (tu)." }
        },
        "required": ["corrections", "appreciation"]
    })
}

/// Consigne de correction des questions rédigées (les autres sont déjà corrigées par Révisio,
/// elles sont seulement rappelées pour que l'appréciation porte sur toute la copie).
pub fn consigne_correction(sujet: &Sujet, reponses: &[String], resultats: &[ResultatQuestion]) -> String {
    let a_corriger: Vec<usize> =
        resultats.iter().enumerate().filter(|(_, r)| r.verdict == "a_corriger").map(|(i, _)| i).collect();
    let mut c = format!(
        "Corrige la copie d'un élève de Première pour l'exercice « {} ». Juge à partir des extraits du cours, des réponses \
         modèles et des éléments attendus (barème). Sois exigeant mais bienveillant ; une formulation différente mais juste \
         rapporte les points ; une réponse vide vaut 0. Les points peuvent être des demi-points. Ne dépasse jamais le barème. \
         Corrige UNIQUEMENT les questions ci-dessous, en reprenant leur numéro.\n\nConsigne : {}\n",
        sujet.titre, sujet.consigne
    );
    if !sujet.contexte.is_empty() {
        c += &format!("\nDocument / contexte :\n{}\n", sujet.contexte);
    }
    let deja: Vec<String> = resultats
        .iter()
        .enumerate()
        .filter(|(_, r)| r.verdict != "a_corriger" && !r.retiree)
        .filter_map(|(i, r)| {
            let q = sujet.questions.get(i)?;
            Some(format!(
                "- Question {} ({} / {} points, {}) : {} — réponse de l'élève : « {} » ; réponse attendue : « {} »",
                i + 1,
                r.points,
                r.points_max,
                r.verdict,
                q.enonce,
                reponses.get(i).map(|x| x.trim()).unwrap_or(""),
                q.reponse_attendue
            ))
        })
        .collect();
    if !deja.is_empty() {
        c += &format!(
            "\nQuestions déjà corrigées automatiquement (ne les corrige pas, mais tiens-en compte dans l'appréciation \
             générale, qui porte sur toute la copie) :\n{}\n",
            deja.join("\n")
        );
    }
    for &i in &a_corriger {
        let q = &sujet.questions[i];
        c += &format!(
            "\n--- Question {} ({} points){} ---\n{}\nRéponse modèle : {}\nÉléments attendus : {}\nRéponse de l'élève :\n{}\n",
            i + 1,
            q.points,
            if q.partie.is_empty() { String::new() } else { format!(", partie « {} »", q.partie) },
            q.enonce,
            q.reponse_attendue,
            q.elements_attendus.join(" ; "),
            reponses.get(i).map(String::as_str).filter(|r| !r.trim().is_empty()).unwrap_or("(pas de réponse)")
        );
    }
    c
}

pub fn valider_correction(c: &CorrectionRedactions, attendues: &[usize]) -> Result<(), String> {
    for &i in attendues {
        if !c.corrections.iter().any(|q| q.numero as usize == i + 1) {
            return Err(format!("correction de la question {} manquante", i + 1));
        }
    }
    Ok(())
}

/// Reporte la correction de l'IA dans les résultats (points plafonnés au barème, arrondis au demi-point).
pub fn appliquer_correction(resultats: &mut [ResultatQuestion], c: &CorrectionRedactions) {
    for cq in &c.corrections {
        let Some(r) = resultats.iter_mut().find(|r| r.numero == cq.numero && r.verdict == "a_corriger") else { continue };
        let points = (cq.points_obtenus.clamp(0.0, r.points_max) * 2.0).round() / 2.0;
        let ratio = if r.points_max > 0.0 { points / r.points_max } else { 0.0 };
        r.points = points;
        r.verdict = if ratio >= 0.75 { "juste" } else if ratio >= 0.25 { "partiel" } else { "faux" }.into();
        r.commentaire = cq.commentaire.clone();
        r.presents = cq.elements_presents.clone();
        r.manquants = cq.elements_manquants.clone();
        r.correcteur = "ia".into();
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn sujet() -> Sujet {
        let mut s: Sujet = serde_json::from_value(json!({
            "titre": "Second degré", "consigne": "Résoudre", "contexte": "", "graphique": null,
            "variables": [{ "nom": "a", "valeur": 1 }, { "nom": "b", "valeur": -3 }, { "nom": "c", "valeur": 2 }],
            "questions": [
                { "enonce": "Calculer Δ", "type_reponse": "nombre", "choix": [], "reponse_attendue": "5", "expression": "b^2-4*a*c",
                  "points": 2, "elements_attendus": [], "partie": "", "source": "E1" },
                { "enonce": "Justifier", "type_reponse": "texte", "choix": [], "reponse_attendue": "Δ>0 donc 2 racines", "expression": "",
                  "points": 3, "elements_attendus": ["signe de Δ"], "partie": "", "source": "E1" },
                { "enonce": "Quelle forme ?", "type_reponse": "choix", "choix": ["canonique", "factorisée"], "reponse_attendue": "factorisée",
                  "expression": "", "points": 1, "elements_attendus": [], "partie": "", "source": "[E2]" },
                { "enonce": "Plus grande racine", "type_reponse": "nombre", "choix": [], "reponse_attendue": "2",
                  "expression": "(-b+sqrt(b^2-4*a*c))/(2*a)", "points": 2, "elements_attendus": [], "partie": "", "source": "E1" }
            ]
        }))
        .unwrap();
        verifier_calculs(&mut s);
        s
    }

    fn ids() -> Vec<String> {
        vec!["E1".into(), "E2".into()]
    }

    #[test]
    fn types_par_matiere() {
        let liste = |m: &str| types_pour(m).into_iter().map(|t| t.id).collect::<Vec<_>>();
        assert_eq!(liste("Français"), vec!["etude_lineaire", "plan_dissertation", "oral", "examen"]);
        assert_eq!(liste("Spé Maths")[0], "valeurs_modifiees");
        assert_eq!(liste("Allemand")[1], "connecteurs");
        assert_eq!(liste("Histoire-Géo")[2], "plan_composition");
        assert_eq!(liste("Spé SVT")[0], "analyse_graphique");
        assert_eq!(liste("DNL histoire géo")[0], "definitions");
        assert!(type_par_id("Spé SVT", "oral").is_none());
        assert!(est_langue("Allemand") && !est_langue("DNL histoire géo"));
        assert_eq!(nom_du_type("oral"), "Simulation de l'oral");
    }

    #[test]
    fn validation_du_sujet() {
        let s = sujet();
        assert!(valider_sujet(&s, &ids()).is_ok());
        let mut sans_expression = s.clone();
        sans_expression.questions[0].expression = "b^2 - 4*a*d".into();
        assert!(valider_sujet(&sans_expression, &ids()).unwrap_err().contains("variable inconnue"));
        let mut mauvais_choix = s.clone();
        mauvais_choix.questions[2].reponse_attendue = "développée".into();
        assert!(valider_sujet(&mauvais_choix, &ids()).is_err());
        let mut mauvaise_source = s.clone();
        mauvaise_source.questions[1].source = "E9".into();
        assert!(valider_sujet(&mauvaise_source, &ids()).is_err());
    }

    #[test]
    fn calculs_refaits_par_revisio() {
        let s = sujet();
        // L'IA avait annoncé Δ = 5 : Révisio recalcule 1 et le signale.
        assert_eq!(s.questions[0].valeur, Some(1.0));
        assert_eq!(s.questions[0].reponse_attendue, "1");
        assert_eq!(s.questions[0].valeur_ia.as_deref(), Some("5"));
        // La racine annoncée était juste.
        assert_eq!(s.questions[3].valeur, Some(2.0));
        assert_eq!(s.questions[3].valeur_ia, None);
    }

    #[test]
    fn correction_automatique_et_note() {
        let s = sujet();
        let reponses: Vec<String> = ["Δ = 1", "Δ est positif", "factorisée", "2,0"].map(String::from).to_vec();
        let mut r = corriger_automatiquement(&s, &reponses, false);
        assert_eq!(r.iter().map(|x| x.verdict.as_str()).collect::<Vec<_>>(), ["juste", "a_corriger", "juste", "juste"]);
        assert_eq!(note_sur_20(&r), Some(20.0));
        let c = CorrectionRedactions {
            corrections: vec![CorrectionQuestion {
                numero: 2,
                points_obtenus: 7.0,
                commentaire: "ok".into(),
                elements_presents: vec![],
                elements_manquants: vec![],
            }],
            appreciation: "bien".into(),
        };
        assert!(valider_correction(&c, &[1]).is_ok());
        assert!(valider_correction(&c, &[1, 0]).is_err());
        appliquer_correction(&mut r, &c);
        assert_eq!((r[1].points, r[1].verdict.as_str(), r[1].correcteur.as_str()), (3.0, "juste", "ia"));

        let faux: Vec<String> = ["5", "", "canonique", "x = 1"].map(String::from).to_vec();
        let r = corriger_automatiquement(&s, &faux, false);
        assert_eq!(r[0].verdict, "faux");
        assert!(r[0].commentaire.contains("Résultat attendu : 1"));
        assert_eq!(note_sur_20(&r), Some(0.0));
    }

    #[test]
    fn reponses_courtes() {
        assert_eq!(comparer_court("1789", "1789", false).1, "juste");
        assert_eq!(comparer_court("le 20 juin 1789", "20 juin 1789", false).1, "juste");
        assert_eq!(comparer_court("Assemblee nationale", "l'Assemblée nationale", false).1, "juste");
        assert_eq!(comparer_court("1788", "1789", false).1, "faux");
        assert_eq!(comparer_court("Robespiere", "Robespierre", false).1, "partiel");
        assert_eq!(comparer_court("cohésine", "cohésines", false).1, "juste");
        assert_eq!(comparer_court("Hund", "Hunde", true).1, "faux");
        assert_eq!(comparer_court("Muller", "Müller", true).1, "partiel");
        assert_eq!(comparer_court("müller", "Müller", true).1, "juste");
        assert_eq!(comparer_court("obwohl", "trotzdem / obwohl", true).1, "juste");
    }

    #[test]
    fn consigne_de_correction() {
        let s = sujet();
        let reponses: Vec<String> = ["1", "Δ est positif", "canonique", ""].map(String::from).to_vec();
        let resultats = corriger_automatiquement(&s, &reponses, false);
        let texte = consigne_correction(&s, &reponses, &resultats);
        assert!(texte.contains("--- Question 2 (3 points) ---"));
        assert!(texte.contains("Δ est positif"));
        // Les questions déjà corrigées sont rappelées pour l'appréciation, pas à corriger.
        assert!(!texte.contains("--- Question 1 "));
        assert!(texte.contains("- Question 3 (0 / 1 points, faux)"));
    }
}
