//! Client de l'API Gemini (REST `generateContent`, v1beta).
//!
//! Tout l'accès réseau à Gemini passe par ce fichier : si Google remplace un jour
//! `generateContent` par l'« Interactions API », seul ce module change.
//! Les noms de modèles viennent des réglages (jamais écrits en dur ici).

use std::time::Duration;

use serde_json::{json, Value};

const BASE: &str = "https://generativelanguage.googleapis.com/v1beta";

/// Ce qu'on demande au modèle.
#[derive(Debug, Clone, Default)]
pub struct Requete {
    pub modele: String,
    /// Consignes générales (« génère uniquement à partir des extraits… »).
    pub systeme: String,
    pub parties: Vec<Partie>,
    /// Schéma JSON attendu en sortie (sortie structurée).
    pub schema: Option<Value>,
    pub temperature: Option<f32>,
}

#[derive(Debug, Clone)]
pub enum Partie {
    Texte(String),
    /// Image envoyée directement (base 64), par ex. un schéma rendu en PNG.
    Image { mime: String, base64: String },
}

#[derive(Debug, Clone, PartialEq)]
pub struct Reponse {
    pub texte: String,
    pub jetons_entree: i64,
    pub jetons_sortie: i64,
}

/// Erreurs expliquées en français, que l'interface affiche telles quelles.
#[derive(Debug, Clone, PartialEq)]
pub enum ErreurIa {
    PasDeCle,
    HorsLigne,
    CleInvalide,
    /// Limite de débit ou quota gratuit atteint (côté Google ou réglage de Révisio).
    Quota(String),
    /// Contenu refusé par les filtres de sécurité de Google.
    Bloque(String),
    Serveur(String),
    /// Service momentanément surchargé (erreurs 500, 503, 504) : Révisio réessaie tout seul.
    Surcharge,
    /// Réponse illisible ou ne respectant pas le format demandé.
    Format(String),
    /// IA locale : Ollama ne répond pas (pas installé ou pas lancé).
    OllamaAbsent,
    /// IA locale : le modèle choisi n'est pas téléchargé dans Ollama.
    ModeleLocalAbsent(String),
    /// IA locale : autre erreur d'Ollama.
    Locale(String),
    /// Mode local choisi par l'élève : Gemini est coupé.
    GeminiCoupe,
    /// Gemini indisponible, et l'IA locale n'a pas pu prendre le relais.
    Secours { gemini: Box<ErreurIa>, locale: Box<ErreurIa> },
}

impl ErreurIa {
    /// Gemini est inutilisable pour le moment (et non la demande elle-même) : l'IA locale peut prendre le relais.
    pub fn justifie_le_secours(&self) -> bool {
        matches!(
            self,
            ErreurIa::PasDeCle
                | ErreurIa::HorsLigne
                | ErreurIa::CleInvalide
                | ErreurIa::Quota(_)
                | ErreurIa::Surcharge
                | ErreurIa::GeminiCoupe
        )
    }
}

impl std::fmt::Display for ErreurIa {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            ErreurIa::PasDeCle => write!(f, "Aucune clé API Gemini : ajoute-la dans les Réglages."),
            ErreurIa::HorsLigne => write!(f, "Pas de connexion Internet : l'IA est indisponible (le reste de Révisio fonctionne)."),
            ErreurIa::CleInvalide => write!(f, "La clé API Gemini est refusée par Google : vérifie-la dans les Réglages."),
            ErreurIa::Quota(m) => write!(f, "Limite d'utilisation atteinte : {m}"),
            ErreurIa::Bloque(m) => write!(f, "Google a refusé de répondre ({m})."),
            ErreurIa::Serveur(m) => write!(f, "Erreur du service Gemini : {m}"),
            ErreurIa::Surcharge => write!(
                f,
                "Gemini est surchargé en ce moment (forte demande chez Google). Réessaie dans quelques minutes, \
                 ou choisis un autre modèle dans les Réglages."
            ),
            ErreurIa::Format(m) => write!(f, "Réponse de l'IA inutilisable : {m}"),
            ErreurIa::OllamaAbsent => write!(
                f,
                "L'IA locale (Ollama) ne répond pas : lance Ollama, ou installe-le (Réglages → IA locale)."
            ),
            ErreurIa::ModeleLocalAbsent(m) => {
                write!(f, "Le modèle « {m} » n'est pas installé dans Ollama : télécharge-le dans les Réglages (IA locale).")
            }
            ErreurIa::Locale(m) => write!(f, "Erreur de l'IA locale (Ollama) : {m}"),
            ErreurIa::GeminiCoupe => write!(
                f,
                "Mode local activé : Gemini est coupé. Les questions, corrections, Feynman et cartes mentales passent par \
                 l'IA locale (choisis un modèle dans les Réglages) ; pour le reste, repasse « En ligne » (en haut à droite)."
            ),
            ErreurIa::Secours { gemini, locale } => write!(f, "{gemini} L'IA locale n'a pas pu prendre le relais : {locale}"),
        }
    }
}

impl From<ErreurIa> for String {
    fn from(e: ErreurIa) -> String {
        e.to_string()
    }
}

/// Synthèse vocale (TTS) à deux voix au plus, par l'Interactions API.
#[derive(Debug, Clone, PartialEq)]
pub struct RequeteVoix {
    pub modele: String,
    /// (nom de la personne, voix prédéfinie de Google).
    pub orateurs: Vec<(String, String)>,
    /// (nom de la personne, texte prononcé tel quel, indication de ton).
    pub repliques: Vec<(String, String, String)>,
}

#[derive(Debug, Clone, PartialEq)]
pub struct ReponseVoix {
    pub pcm: crate::audio::Pcm,
    pub jetons_entree: i64,
    pub jetons_sortie: i64,
}

#[derive(Debug, Clone, serde::Serialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct ModeleInfo {
    /// Identifiant à utiliser (ex. `gemini-3.8-flash`).
    pub id: String,
    pub nom: String,
    pub description: String,
}

pub struct Gemini {
    http: reqwest::Client,
}

impl Default for Gemini {
    fn default() -> Self {
        Self::new()
    }
}

impl Gemini {
    pub fn new() -> Self {
        let http = reqwest::Client::builder()
            .timeout(Duration::from_secs(240))
            .connect_timeout(Duration::from_secs(10))
            .build()
            .expect("client HTTP");
        Self { http }
    }

    pub async fn generer(&self, cle: &str, requete: &Requete) -> Result<Reponse, ErreurIa> {
        let url = format!("{BASE}/models/{}:generateContent", requete.modele);
        let reponse = self
            .http
            .post(url)
            .header("x-goog-api-key", cle)
            .json(&corps_requete(requete))
            .send()
            .await
            .map_err(erreur_reseau)?;
        let statut = reponse.status().as_u16();
        let corps: Value = reponse.json().await.map_err(|e| ErreurIa::Format(e.to_string()))?;
        if statut != 200 {
            return Err(erreur_http(statut, &corps));
        }
        lire_reponse(&corps)
    }

    /// Voix de synthèse : `store: false`, Google ne conserve pas l'échange.
    pub async fn generer_voix(&self, cle: &str, requete: &RequeteVoix) -> Result<ReponseVoix, ErreurIa> {
        let reponse = self
            .http
            .post(format!("{BASE}/interactions"))
            .header("x-goog-api-key", cle)
            .timeout(Duration::from_secs(300))
            .json(&corps_voix(requete))
            .send()
            .await
            .map_err(erreur_reseau)?;
        let statut = reponse.status().as_u16();
        let corps: Value = reponse.json().await.map_err(|e| ErreurIa::Format(e.to_string()))?;
        if statut != 200 {
            return Err(erreur_http(statut, &corps));
        }
        lire_reponse_voix(&corps)
    }

    /// Le serveur de Gemini répond-il ? Une requête HEAD sans clé (4 s au plus), par le même client que les
    /// vrais appels (mêmes réglages réseau, proxy compris) : n'importe quelle réponse HTTP suffit.
    pub async fn joignable(&self) -> bool {
        let racine = BASE.split("/v1beta").next().unwrap_or(BASE);
        self.http.head(racine).timeout(Duration::from_secs(4)).send().await.is_ok()
    }

    /// Modèles disponibles pour cette clé qui savent générer du texte.
    pub async fn lister_modeles(&self, cle: &str) -> Result<Vec<ModeleInfo>, ErreurIa> {
        let reponse = self
            .http
            .get(format!("{BASE}/models?pageSize=200"))
            .header("x-goog-api-key", cle)
            .send()
            .await
            .map_err(erreur_reseau)?;
        let statut = reponse.status().as_u16();
        let corps: Value = reponse.json().await.map_err(|e| ErreurIa::Format(e.to_string()))?;
        if statut != 200 {
            return Err(erreur_http(statut, &corps));
        }
        Ok(lire_modeles(&corps))
    }
}

/// Corps JSON de la requête `generateContent`.
pub fn corps_requete(r: &Requete) -> Value {
    let parties: Vec<Value> = r
        .parties
        .iter()
        .map(|p| match p {
            Partie::Texte(t) => json!({ "text": t }),
            Partie::Image { mime, base64 } => json!({ "inlineData": { "mimeType": mime, "data": base64 } }),
        })
        .collect();
    let mut config = json!({});
    if let Some(schema) = &r.schema {
        config["responseMimeType"] = json!("application/json");
        config["responseJsonSchema"] = schema.clone();
    }
    if let Some(t) = r.temperature {
        config["temperature"] = json!(t);
    }
    let mut corps = json!({
        "contents": [{ "role": "user", "parts": parties }],
        "generationConfig": config,
    });
    if !r.systeme.is_empty() {
        corps["systemInstruction"] = json!({ "parts": [{ "text": r.systeme }] });
    }
    corps
}

/// Texte produit (hors « pensées » du modèle) et nombre de jetons consommés.
pub fn lire_reponse(corps: &Value) -> Result<Reponse, ErreurIa> {
    if let Some(raison) = corps["promptFeedback"]["blockReason"].as_str() {
        return Err(ErreurIa::Bloque(raison.to_string()));
    }
    let candidat = &corps["candidates"][0];
    if candidat.is_null() {
        return Err(ErreurIa::Format("aucune réponse proposée".into()));
    }
    let texte: String = candidat["content"]["parts"]
        .as_array()
        .map(|parties| {
            parties
                .iter()
                .filter(|p| p["thought"] != true)
                .filter_map(|p| p["text"].as_str())
                .collect()
        })
        .unwrap_or_default();
    if texte.trim().is_empty() {
        let raison = candidat["finishReason"].as_str().unwrap_or("réponse vide");
        return Err(if matches!(raison, "SAFETY" | "PROHIBITED_CONTENT" | "BLOCKLIST" | "RECITATION") {
            ErreurIa::Bloque(raison.to_string())
        } else {
            ErreurIa::Format(raison.to_string())
        });
    }
    Ok(Reponse {
        texte,
        jetons_entree: corps["usageMetadata"]["promptTokenCount"].as_i64().unwrap_or(0),
        jetons_sortie: corps["usageMetadata"]["candidatesTokenCount"].as_i64().unwrap_or(0),
    })
}

pub fn lire_modeles(corps: &Value) -> Vec<ModeleInfo> {
    let mut modeles: Vec<ModeleInfo> = corps["models"]
        .as_array()
        .map(|liste| {
            liste
                .iter()
                .filter(|m| {
                    m["supportedGenerationMethods"]
                        .as_array()
                        .is_some_and(|ms| ms.iter().any(|x| x == "generateContent"))
                })
                .filter_map(|m| {
                    let id = m["name"].as_str()?.trim_start_matches("models/").to_string();
                    Some(ModeleInfo {
                        nom: m["displayName"].as_str().unwrap_or(&id).to_string(),
                        description: m["description"].as_str().unwrap_or("").to_string(),
                        id,
                    })
                })
                .collect()
        })
        .unwrap_or_default();
    modeles.sort_by(|a, b| a.id.cmp(&b.id));
    modeles
}

fn erreur_reseau(e: reqwest::Error) -> ErreurIa {
    // Connexion impossible (même par dépassement du délai de connexion) : pas d'Internet, inutile de réessayer.
    // Testé en premier : une connexion trop lente est aussi un « timeout ».
    if e.is_connect() {
        ErreurIa::HorsLigne
    } else if e.is_timeout() {
        // Réponse trop lente : Google est débordé (on réessaie), pas un problème de connexion.
        ErreurIa::Surcharge
    } else if e.is_request() {
        ErreurIa::HorsLigne
    } else {
        ErreurIa::Serveur(e.to_string())
    }
}

/// Une seule personne (narration) : `speech_config` est une liste `[{voice}]`, sans nom d'orateur ;
/// plusieurs : un objet `{mode: "conversational", speakers: [...]}` (doc ai.google.dev, oct. 2026).
pub fn corps_voix(r: &RequeteVoix) -> Value {
    let seul = r.orateurs.len() == 1;
    let contenu: Vec<Value> = r
        .repliques
        .iter()
        .map(|(orateur, texte, style)| {
            let mut meta = json!({ "type": "speech_metadata" });
            if !seul {
                meta["speaker"] = json!(orateur);
            }
            if !style.trim().is_empty() {
                meta["style"] = json!(style.trim());
            }
            json!({ "type": "text", "text": texte, "annotations": [meta] })
        })
        .collect();
    let config = if seul {
        json!([{ "voice": r.orateurs[0].1 }])
    } else {
        let orateurs: Vec<Value> = r.orateurs.iter().map(|(o, v)| json!({ "speaker": o, "voice": v })).collect();
        json!({ "mode": "conversational", "speakers": orateurs })
    };
    json!({
        "model": r.modele,
        "input": [{ "type": "user_input", "content": contenu }],
        "response_format": { "type": "audio" },
        "generation_config": { "speech_config": config },
        "store": false
    })
}

/// L'audio est dans `steps[].content[]` (élément de type « audio », base64) : WAV par défaut,
/// ou PCM brut 24 kHz 16 bits mono (« audio/l16 »). Plusieurs morceaux sont mis bout à bout.
pub fn lire_reponse_voix(corps: &Value) -> Result<ReponseVoix, ErreurIa> {
    use base64::Engine;
    let mut pcm: Option<crate::audio::Pcm> = None;
    for etape in corps["steps"].as_array().into_iter().flatten() {
        for c in etape["content"].as_array().into_iter().flatten() {
            let Some(donnees) = c["data"].as_str().filter(|_| c["type"] == "audio") else { continue };
            let octets = base64::engine::general_purpose::STANDARD
                .decode(donnees)
                .map_err(|e| ErreurIa::Format(format!("audio illisible ({e})")))?;
            let morceau = if octets.starts_with(b"RIFF") {
                crate::audio::lire_wav(&octets).map_err(ErreurIa::Format)?
            } else {
                crate::audio::Pcm { frequence: 24_000, canaux: 1, bits: 16, donnees: octets }
            };
            match pcm.as_mut() {
                Some(p) => p.ajouter(&morceau).map_err(ErreurIa::Format)?,
                None => pcm = Some(morceau),
            }
        }
    }
    let pcm = pcm.ok_or_else(|| ErreurIa::Format("la réponse ne contient pas d'audio".into()))?;
    let jetons = |cle: &str| corps["usage"][cle].as_i64().unwrap_or(0);
    Ok(ReponseVoix { pcm, jetons_entree: jetons("total_input_tokens"), jetons_sortie: jetons("total_output_tokens") })
}

pub fn erreur_http(statut: u16, corps: &Value) -> ErreurIa {
    let message = corps["error"]["message"].as_str().unwrap_or("").to_string();
    let raison = corps["error"]["details"]
        .as_array()
        .and_then(|d| d.iter().find_map(|x| x["reason"].as_str()))
        .unwrap_or("");
    match statut {
        400 if raison == "API_KEY_INVALID" || message.contains("API key") => ErreurIa::CleInvalide,
        401 | 403 => ErreurIa::CleInvalide,
        429 => ErreurIa::Quota(
            "le quota gratuit de Gemini est atteint pour le moment ; réessaie dans une minute (ou demain pour le quota du jour)."
                .into(),
        ),
        404 => ErreurIa::Serveur(format!("modèle introuvable ({message}) : choisis-en un autre dans les Réglages.")),
        500 | 502 | 503 | 504 => ErreurIa::Surcharge,
        _ => ErreurIa::Serveur(if message.is_empty() { format!("code {statut}") } else { message }),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Sans Internet, une connexion qui n'aboutit pas est « hors ligne » tout de suite, pas une « surcharge »
    /// (qui ferait réessayer pendant près d'une minute avec un message trompeur).
    #[tokio::test(flavor = "current_thread")]
    async fn connexion_impossible_hors_ligne() {
        let http = reqwest::Client::builder().connect_timeout(Duration::from_millis(200)).build().unwrap();
        // Adresse non routable : la connexion échoue (délai dépassé ou réseau inaccessible).
        let e = http.get("http://10.255.255.1:81/").send().await.unwrap_err();
        assert_eq!(erreur_reseau(e), ErreurIa::HorsLigne);
    }

    #[test]
    fn voix_a_deux_personnes() {
        use base64::Engine;
        let r = RequeteVoix {
            modele: "tts".into(),
            orateurs: vec![("Camille".into(), "Kore".into()), ("Théo".into(), "Puck".into())],
            repliques: vec![("Camille".into(), "Bonjour !".into(), "enjoué".into()), ("Théo".into(), "Salut.".into(), "".into())],
        };
        let c = corps_voix(&r);
        assert_eq!(c["store"], false);
        assert_eq!(c["input"][0]["content"][0]["annotations"][0]["speaker"], "Camille");
        assert_eq!(c["input"][0]["content"][0]["annotations"][0]["style"], "enjoué");
        assert!(c["input"][0]["content"][1]["annotations"][0].get("style").is_none());
        assert_eq!(c["generation_config"]["speech_config"]["speakers"][1]["voice"], "Puck");

        let wav = crate::audio::Pcm { frequence: 24_000, canaux: 1, bits: 16, donnees: vec![7; 480] }.en_wav();
        let b64 = |o: &[u8]| base64::engine::general_purpose::STANDARD.encode(o);
        let reponse = json!({
            "steps": [
                { "type": "thought", "content": [{ "type": "text", "text": "…" }] },
                { "type": "model_output", "content": [{ "type": "audio", "data": b64(&wav) }, { "type": "audio", "data": b64(&[1, 2]) }] }
            ],
            "usage": { "total_input_tokens": 12, "total_output_tokens": 340 }
        });
        let v = lire_reponse_voix(&reponse).unwrap();
        assert_eq!(v.pcm.donnees.len(), 482);
        assert_eq!((v.jetons_entree, v.jetons_sortie), (12, 340));
        assert!(lire_reponse_voix(&json!({ "steps": [] })).is_err());
    }

    #[test]
    fn voix_d_un_seul_narrateur() {
        let r = RequeteVoix {
            modele: "tts".into(),
            orateurs: vec![("Narration".into(), "Charon".into())],
            repliques: vec![("Narration".into(), "La méiose produit quatre cellules.".into(), "posé".into())],
        };
        let c = corps_voix(&r);
        assert_eq!(c["generation_config"]["speech_config"], json!([{ "voice": "Charon" }]));
        assert!(c["input"][0]["content"][0]["annotations"][0].get("speaker").is_none());
        assert_eq!(c["input"][0]["content"][0]["annotations"][0]["style"], "posé");
    }

    #[test]
    fn corps_avec_schema_systeme_et_image() {
        let r = Requete {
            modele: "un-modele".into(),
            systeme: "Consignes".into(),
            parties: vec![
                Partie::Texte("Bonjour".into()),
                Partie::Image { mime: "image/png".into(), base64: "AAA".into() },
            ],
            schema: Some(json!({ "type": "object" })),
            temperature: Some(0.3),
        };
        let c = corps_requete(&r);
        assert_eq!(c["systemInstruction"]["parts"][0]["text"], "Consignes");
        assert_eq!(c["contents"][0]["parts"][1]["inlineData"]["mimeType"], "image/png");
        assert_eq!(c["generationConfig"]["responseMimeType"], "application/json");
        assert_eq!(c["generationConfig"]["responseJsonSchema"]["type"], "object");
    }

    #[test]
    fn lecture_de_la_reponse_sans_les_pensees() {
        let corps = json!({
            "candidates": [{ "content": { "parts": [
                { "text": "je réfléchis", "thought": true },
                { "text": "{\"ok\":" }, { "text": "true}" }
            ]}, "finishReason": "STOP" }],
            "usageMetadata": { "promptTokenCount": 120, "candidatesTokenCount": 30 }
        });
        assert_eq!(
            lire_reponse(&corps).unwrap(),
            Reponse { texte: "{\"ok\":true}".into(), jetons_entree: 120, jetons_sortie: 30 }
        );
    }

    #[test]
    fn reponses_refusees_ou_vides() {
        assert_eq!(
            lire_reponse(&json!({ "promptFeedback": { "blockReason": "SAFETY" } })),
            Err(ErreurIa::Bloque("SAFETY".into()))
        );
        assert_eq!(
            lire_reponse(&json!({ "candidates": [{ "content": { "parts": [] }, "finishReason": "MAX_TOKENS" }] })),
            Err(ErreurIa::Format("MAX_TOKENS".into()))
        );
    }

    #[test]
    fn erreurs_http_traduites() {
        let cle = json!({ "error": { "message": "API key not valid", "details": [{ "reason": "API_KEY_INVALID" }] } });
        assert_eq!(erreur_http(400, &cle), ErreurIa::CleInvalide);
        assert!(matches!(erreur_http(429, &json!({})), ErreurIa::Quota(_)));
        assert_eq!(erreur_http(503, &json!({ "error": { "message": "high demand" } })), ErreurIa::Surcharge);
        assert!(matches!(erreur_http(501, &json!({ "error": { "message": "boom" } })), ErreurIa::Serveur(m) if m == "boom"));
    }

    #[test]
    fn liste_des_modeles_de_texte() {
        let corps = json!({ "models": [
            { "name": "models/gemini-3.8-flash", "displayName": "Gemini 3.8 Flash", "supportedGenerationMethods": ["generateContent", "countTokens"] },
            { "name": "models/text-embedding", "displayName": "Embedding", "supportedGenerationMethods": ["embedContent"] },
            { "name": "models/gemini-3.5-flash-lite", "supportedGenerationMethods": ["generateContent"] }
        ]});
        let m = lire_modeles(&corps);
        assert_eq!(m.iter().map(|x| x.id.as_str()).collect::<Vec<_>>(), vec!["gemini-3.5-flash-lite", "gemini-3.8-flash"]);
        assert_eq!(m[1].nom, "Gemini 3.8 Flash");
    }
}
