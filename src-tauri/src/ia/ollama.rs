//! Client d'Ollama, l'IA locale (API REST sur le PC : `http://127.0.0.1:11434`, sans clé ni Internet).
//!
//! Même forme de requête que Gemini (`Requete` → `Reponse`) : le reste de Révisio ne voit pas la
//! différence. Sortie JSON imposée par `format` (schéma JSON) ; « réflexion » désactivée (`think: false`)
//! pour répondre vite. Seules les adresses de ce PC sont acceptées : l'IA locale reste locale.

use std::time::Duration;

use serde::Serialize;
use serde_json::{json, Value};

use super::gemini::{ErreurIa, Partie, Reponse, Requete};

pub const ADRESSE_DEFAUT: &str = "http://127.0.0.1:11434";

/// Taille de la sortie prévue (jetons) en plus de la consigne, pour régler la mémoire de contexte.
const JETONS_SORTIE: usize = 4096;

#[derive(Debug, Clone, Serialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct ModeleLocal {
    /// Nom à utiliser (ex. `qwen3.5:4b`).
    pub nom: String,
    /// Taille sur le disque (octets).
    pub taille: u64,
    /// Ex. « 4.7B ».
    pub parametres: String,
    /// Ex. « Q4_K_M ».
    pub quantification: String,
    pub famille: String,
}

/// Avancement d'un téléchargement de modèle.
#[derive(Debug, Clone, Serialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct Avancement {
    pub modele: String,
    pub statut: String,
    pub total: u64,
    pub fait: u64,
}

/// Adresse d'Ollama sur ce PC uniquement (http://127.0.0.1, localhost ou [::1], avec un port).
pub fn adresse_valide(adresse: &str) -> Result<String, String> {
    let a = adresse.trim().trim_end_matches('/');
    let url = reqwest::Url::parse(a).map_err(|_| format!("Adresse invalide : {a}"))?;
    let locale = matches!(url.host_str(), Some("127.0.0.1" | "localhost" | "[::1]"));
    if url.scheme() != "http" || !locale || url.path() != "/" || url.query().is_some() {
        return Err("L'IA locale doit tourner sur ce PC (ex. http://127.0.0.1:11434).".into());
    }
    Ok(a.to_string())
}

/// Nom de modèle Ollama acceptable (ex. `qwen3.5:4b`, `ministral-3:8b`, `utilisateur/modele:etiquette`).
pub fn nom_modele_valide(nom: &str) -> bool {
    !nom.is_empty()
        && nom.len() <= 120
        && nom.chars().all(|c| c.is_ascii_alphanumeric() || "-._:/".contains(c))
        && !nom.starts_with(['-', '.', '/'])
}

pub struct Ollama {
    http: reqwest::Client,
}

impl Default for Ollama {
    fn default() -> Self {
        Self::new()
    }
}

/// Mémoire de contexte nécessaire : consigne (≈ 3 caractères par jeton en français) + réponse,
/// arrondie au multiple de 4096, entre 8192 et 32768 (au-delà, une petite carte graphique sature).
pub fn contexte_pour(r: &Requete) -> usize {
    let caracteres: usize = r.systeme.len()
        + r.parties.iter().map(|p| if let Partie::Texte(t) = p { t.len() } else { 2000 }).sum::<usize>()
        + r.schema.as_ref().map_or(0, |s| s.to_string().len());
    (caracteres / 3 + JETONS_SORTIE).div_ceil(4096).clamp(2, 8) * 4096
}

/// Corps de `POST /api/chat`. Les textes sont réunis dans un seul message, les images jointes à part.
pub fn corps_chat(r: &Requete, modele: &str, sans_reflexion: bool) -> Value {
    let texte: Vec<&str> = r.parties.iter().filter_map(|p| if let Partie::Texte(t) = p { Some(t.as_str()) } else { None }).collect();
    let images: Vec<&str> =
        r.parties.iter().filter_map(|p| if let Partie::Image { base64, .. } = p { Some(base64.as_str()) } else { None }).collect();
    let mut utilisateur = json!({ "role": "user", "content": texte.join("\n\n") });
    if !images.is_empty() {
        utilisateur["images"] = json!(images);
    }
    let mut messages = Vec::new();
    if !r.systeme.is_empty() {
        messages.push(json!({ "role": "system", "content": r.systeme }));
    }
    messages.push(utilisateur);
    let mut options = json!({ "num_ctx": contexte_pour(r) });
    if let Some(t) = r.temperature {
        options["temperature"] = json!(t);
    }
    let mut corps = json!({
        "model": modele,
        "messages": messages,
        "stream": false,
        "options": options,
        "keep_alive": "10m",
    });
    if sans_reflexion {
        corps["think"] = json!(false);
    }
    if let Some(s) = &r.schema {
        corps["format"] = s.clone();
    }
    corps
}

pub fn lire_reponse(corps: &Value) -> Result<Reponse, ErreurIa> {
    let texte = corps["message"]["content"].as_str().unwrap_or("").to_string();
    if texte.trim().is_empty() {
        return Err(ErreurIa::Format(format!("réponse vide ({})", corps["done_reason"].as_str().unwrap_or("?"))));
    }
    Ok(Reponse {
        texte,
        jetons_entree: corps["prompt_eval_count"].as_i64().unwrap_or(0),
        jetons_sortie: corps["eval_count"].as_i64().unwrap_or(0),
    })
}

pub fn lire_modeles(corps: &Value) -> Vec<ModeleLocal> {
    let mut modeles: Vec<ModeleLocal> = corps["models"]
        .as_array()
        .into_iter()
        .flatten()
        .filter_map(|m| {
            let d = &m["details"];
            Some(ModeleLocal {
                nom: m["name"].as_str().or(m["model"].as_str())?.to_string(),
                taille: m["size"].as_u64().unwrap_or(0),
                parametres: d["parameter_size"].as_str().unwrap_or("").into(),
                quantification: d["quantization_level"].as_str().unwrap_or("").into(),
                famille: d["family"].as_str().unwrap_or("").into(),
            })
        })
        // Les modèles d'« embedding » ne savent pas écrire de texte.
        .filter(|m| !m.nom.contains("embed"))
        .collect();
    modeles.sort_by(|a, b| a.nom.cmp(&b.nom));
    modeles
}

/// Erreur renvoyée par Ollama (`{"error": "..."}`) → message en français.
pub fn erreur_http(statut: u16, corps: &Value, modele: &str) -> ErreurIa {
    let message = corps["error"].as_str().unwrap_or("").to_string();
    if statut == 404 || message.contains("not found") {
        return ErreurIa::ModeleLocalAbsent(modele.to_string());
    }
    if message.contains("memory") {
        return ErreurIa::Locale(format!(
            "pas assez de mémoire pour le modèle « {modele} » : ferme d'autres programmes ou choisis un modèle plus petit."
        ));
    }
    ErreurIa::Locale(if message.is_empty() { format!("code {statut}") } else { message })
}

fn erreur_reseau(e: reqwest::Error) -> ErreurIa {
    if e.is_timeout() {
        ErreurIa::Locale("le modèle met trop de temps à répondre (essaie un modèle plus petit).".into())
    } else if e.is_connect() || e.is_request() {
        ErreurIa::OllamaAbsent
    } else {
        ErreurIa::Locale(e.to_string())
    }
}

impl Ollama {
    pub fn new() -> Self {
        let http = reqwest::Client::builder()
            // Un petit modèle sur un portable peut mettre plusieurs minutes pour un gros lot de questions.
            .timeout(Duration::from_secs(600))
            .connect_timeout(Duration::from_secs(3))
            .no_proxy()
            .build()
            .expect("client HTTP");
        Self { http }
    }

    async fn obtenir(&self, adresse: &str, chemin: &str) -> Result<Value, ErreurIa> {
        let reponse = self
            .http
            .get(format!("{adresse}{chemin}"))
            .timeout(Duration::from_secs(5))
            .send()
            .await
            .map_err(erreur_reseau)?;
        let statut = reponse.status().as_u16();
        let corps: Value = reponse.json().await.map_err(|e| ErreurIa::Locale(e.to_string()))?;
        if statut != 200 {
            return Err(erreur_http(statut, &corps, ""));
        }
        Ok(corps)
    }

    pub async fn version(&self, adresse: &str) -> Result<String, ErreurIa> {
        Ok(self.obtenir(adresse, "/api/version").await?["version"].as_str().unwrap_or("?").to_string())
    }

    pub async fn modeles(&self, adresse: &str) -> Result<Vec<ModeleLocal>, ErreurIa> {
        Ok(lire_modeles(&self.obtenir(adresse, "/api/tags").await?))
    }

    async fn chat(&self, adresse: &str, corps: &Value, modele: &str) -> Result<Reponse, ErreurIa> {
        let reponse = self.http.post(format!("{adresse}/api/chat")).json(corps).send().await.map_err(erreur_reseau)?;
        let statut = reponse.status().as_u16();
        let corps: Value = reponse.json().await.map_err(|e| ErreurIa::Locale(e.to_string()))?;
        if statut != 200 {
            return Err(erreur_http(statut, &corps, modele));
        }
        lire_reponse(&corps)
    }

    /// Génère une réponse. Les modèles sans mode « réflexion » refusent `think` : on redemande sans.
    pub async fn generer(&self, adresse: &str, modele: &str, requete: &Requete) -> Result<Reponse, ErreurIa> {
        match self.chat(adresse, &corps_chat(requete, modele, true), modele).await {
            Err(ErreurIa::Locale(m)) if m.contains("think") => self.chat(adresse, &corps_chat(requete, modele, false), modele).await,
            r => r,
        }
    }

    /// Télécharge un modèle (`POST /api/pull`, réponses ligne par ligne). `suivre` reçoit l'avancement ;
    /// s'il renvoie `false`, le téléchargement s'arrête (Ollama le reprendra là où il en était).
    pub async fn telecharger(
        &self,
        adresse: &str,
        modele: &str,
        mut suivre: impl FnMut(Avancement) -> bool,
    ) -> Result<(), ErreurIa> {
        let mut reponse = self
            .http
            .post(format!("{adresse}/api/pull"))
            .timeout(Duration::from_secs(6 * 3600))
            .json(&json!({ "model": modele, "stream": true }))
            .send()
            .await
            .map_err(erreur_reseau)?;
        let statut = reponse.status().as_u16();
        if statut != 200 {
            let corps: Value = reponse.json().await.unwrap_or_default();
            return Err(erreur_http(statut, &corps, modele));
        }
        let mut reste = Vec::new();
        while let Some(morceau) = reponse.chunk().await.map_err(erreur_reseau)? {
            reste.extend_from_slice(&morceau);
            while let Some(fin) = reste.iter().position(|&o| o == b'\n') {
                let ligne: Vec<u8> = reste.drain(..=fin).collect();
                let Ok(v) = serde_json::from_slice::<Value>(&ligne) else { continue };
                if let Some(e) = v["error"].as_str() {
                    return Err(ErreurIa::Locale(format!("téléchargement de « {modele} » : {e}")));
                }
                let avancement = Avancement {
                    modele: modele.into(),
                    statut: v["status"].as_str().unwrap_or("").into(),
                    total: v["total"].as_u64().unwrap_or(0),
                    fait: v["completed"].as_u64().unwrap_or(0),
                };
                let fini = avancement.statut == "success";
                if !suivre(avancement) {
                    return Err(ErreurIa::Locale("téléchargement interrompu (il reprendra là où il en était).".into()));
                }
                if fini {
                    return Ok(());
                }
            }
        }
        Err(ErreurIa::Locale(format!("téléchargement de « {modele} » inachevé.")))
    }

    pub async fn supprimer(&self, adresse: &str, modele: &str) -> Result<(), ErreurIa> {
        let reponse = self
            .http
            .delete(format!("{adresse}/api/delete"))
            .timeout(Duration::from_secs(30))
            .json(&json!({ "model": modele }))
            .send()
            .await
            .map_err(erreur_reseau)?;
        let statut = reponse.status().as_u16();
        if statut != 200 {
            let corps: Value = reponse.json().await.unwrap_or_default();
            return Err(erreur_http(statut, &corps, modele));
        }
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn adresses_et_noms() {
        assert_eq!(adresse_valide(" http://127.0.0.1:11434/ ").unwrap(), "http://127.0.0.1:11434");
        assert!(adresse_valide("http://localhost:11500").is_ok());
        assert!(adresse_valide("http://192.168.1.20:11434").is_err(), "pas d'autre PC");
        assert!(adresse_valide("https://ollama.com").is_err());
        assert!(adresse_valide("http://127.0.0.1:11434/api/chat").is_err());
        assert!(nom_modele_valide("qwen3.5:4b") && nom_modele_valide("ministral-3:8b") && nom_modele_valide("hf.co/a/b:Q4"));
        assert!(!nom_modele_valide("") && !nom_modele_valide("a b") && !nom_modele_valide("-rm") && !nom_modele_valide("x;y"));
    }

    #[test]
    fn corps_de_la_requete() {
        let r = Requete {
            modele: "ignoré".into(),
            systeme: "Consignes".into(),
            parties: vec![
                Partie::Texte("Bonjour".into()),
                Partie::Image { mime: "image/png".into(), base64: "AAA".into() },
                Partie::Texte("Réponds de nouveau.".into()),
            ],
            schema: Some(json!({ "type": "object" })),
            temperature: Some(0.3),
        };
        let c = corps_chat(&r, "qwen3.5:4b", true);
        assert_eq!(c["model"], "qwen3.5:4b");
        assert_eq!(c["messages"][0], json!({ "role": "system", "content": "Consignes" }));
        assert_eq!(c["messages"][1]["content"], "Bonjour\n\nRéponds de nouveau.");
        assert_eq!(c["messages"][1]["images"], json!(["AAA"]));
        assert_eq!((c["stream"].clone(), c["think"].clone()), (json!(false), json!(false)));
        assert_eq!(c["format"], json!({ "type": "object" }));
        assert_eq!(c["options"]["num_ctx"], 8192);
        assert!((c["options"]["temperature"].as_f64().unwrap() - 0.3).abs() < 1e-6);
        assert!(corps_chat(&r, "m", false).get("think").is_none());
    }

    #[test]
    fn contexte_selon_la_longueur() {
        let avec = |n: usize| Requete { parties: vec![Partie::Texte("x".repeat(n))], ..Default::default() };
        assert_eq!(contexte_pour(&avec(100)), 8192);
        assert_eq!(contexte_pour(&avec(30_000)), 16_384); // ≈ 10 000 + 4096 jetons
        assert_eq!(contexte_pour(&avec(500_000)), 32_768);
    }

    #[test]
    fn reponses_et_erreurs() {
        let r = lire_reponse(&json!({ "message": { "content": "{\"a\":1}" }, "prompt_eval_count": 120, "eval_count": 30 })).unwrap();
        assert_eq!((r.texte.as_str(), r.jetons_entree, r.jetons_sortie), ("{\"a\":1}", 120, 30));
        assert!(lire_reponse(&json!({ "message": { "content": "" }, "done_reason": "length" })).is_err());
        let modeles = lire_modeles(&json!({ "models": [
            { "name": "qwen3.5:4b", "size": 3_300_000_000u64, "details": { "parameter_size": "4.7B", "quantization_level": "Q4_K_M", "family": "qwen35" } },
            { "name": "nomic-embed-text:latest", "size": 1 },
            { "name": "ministral-3:8b", "size": 6_000_000_000u64, "details": {} }
        ]}));
        assert_eq!(modeles.iter().map(|m| m.nom.as_str()).collect::<Vec<_>>(), ["ministral-3:8b", "qwen3.5:4b"]);
        assert_eq!(modeles[1].quantification, "Q4_K_M");
        assert_eq!(erreur_http(404, &json!({ "error": "model \"x\" not found, try pulling it first" }), "x"), ErreurIa::ModeleLocalAbsent("x".into()));
        assert!(matches!(erreur_http(500, &json!({ "error": "model requires more system memory" }), "x"), ErreurIa::Locale(m) if m.contains("mémoire")));
    }
}
