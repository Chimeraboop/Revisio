//! Google Drive, pour synchroniser Révisio avec le téléphone.
//!
//! Connexion : OAuth 2.0 pour « application de bureau » (le navigateur s'ouvre, Google renvoie vers un
//! petit serveur sur ce PC, `127.0.0.1`, protégé par PKCE). Seul le niveau d'accès `drive.appdata` est
//! demandé : un dossier caché propre à Révisio dans le Drive de l'élève (`appDataFolder`). Révisio ne
//! voit aucun autre fichier du Drive. Le jeton d'actualisation est gardé dans le coffre du système.

use std::collections::hash_map::RandomState;
use std::hash::{BuildHasher, Hasher};
use std::time::{Duration, Instant};

use base64::Engine;
use serde::Deserialize;
use sha2::{Digest, Sha256};
use tokio::io::{AsyncReadExt, AsyncWriteExt};

use crate::secret;

pub const CLIENT_ID: &str = "703196424411-dlefdj8pn9ufj8qrg8scifofstfngf12.apps.googleusercontent.com";
/// Code du client « application de bureau ». Google précise qu'il ne protège rien pour une application
/// distribuée, mais il n'a pas sa place dans un dépôt public : il est intégré à la compilation (voir build.rs).
const SECRET: Option<&str> = option_env!("REVISIO_GOOGLE_SECRET");
const PORTEE: &str = "https://www.googleapis.com/auth/drive.appdata";
const AUTORISATION: &str = "https://accounts.google.com/o/oauth2/v2/auth";
const JETON: &str = "https://oauth2.googleapis.com/token";
const DRIVE: &str = "https://www.googleapis.com/drive/v3/files";
const ENVOI: &str = "https://www.googleapis.com/upload/drive/v3/files";
/// Temps laissé à l'élève pour se connecter dans le navigateur.
const DELAI_CONNEXION: Duration = Duration::from_secs(300);

pub const EXPIREE: &str = "La connexion à Google a expiré ou a été retirée : reconnecte ton Google Drive dans les Réglages.";

pub const SANS_SECRET: &str = "Cette version de Révisio a été compilée sans l'accès à Google Drive : la synchronisation est indisponible.";

fn secret_client() -> Result<&'static str, String> {
    SECRET.ok_or_else(|| SANS_SECRET.to_string())
}

/// Octets aléatoires encodés en base64 « url » (vérificateur PKCE, état anti-falsification).
fn aleatoire(octets: usize) -> String {
    let mut v = Vec::with_capacity(octets + 8);
    while v.len() < octets {
        let mut h = RandomState::new().build_hasher();
        h.write_usize(v.len());
        h.write_u128(std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).unwrap_or_default().as_nanos());
        v.extend_from_slice(&h.finish().to_le_bytes());
    }
    v.truncate(octets);
    base64::engine::general_purpose::URL_SAFE_NO_PAD.encode(v)
}

pub fn encoder(texte: &str) -> String {
    let mut r = String::new();
    for o in texte.bytes() {
        match o {
            b'A'..=b'Z' | b'a'..=b'z' | b'0'..=b'9' | b'-' | b'_' | b'.' | b'~' => r.push(o as char),
            _ => r.push_str(&format!("%{o:02X}")),
        }
    }
    r
}

fn decoder(texte: &str) -> String {
    let o = texte.as_bytes();
    let mut r = Vec::with_capacity(o.len());
    let mut i = 0;
    while i < o.len() {
        match o[i] {
            b'%' if i + 2 < o.len() => {
                let chiffre = |c: u8| (c as char).to_digit(16);
                match (chiffre(o[i + 1]), chiffre(o[i + 2])) {
                    (Some(a), Some(b)) => {
                        r.push((a * 16 + b) as u8);
                        i += 3;
                        continue;
                    }
                    _ => r.push(b'%'),
                }
            }
            b'+' => r.push(b' '),
            c => r.push(c),
        }
        i += 1;
    }
    String::from_utf8_lossy(&r).into_owned()
}

/// Paramètres de la requête reçue par le petit serveur local (`GET /?code=…&state=… HTTP/1.1`).
fn parametres(premiere_ligne: &str) -> Vec<(String, String)> {
    let chemin = premiere_ligne.split_whitespace().nth(1).unwrap_or("");
    let requete = chemin.split_once('?').map(|(_, q)| q).unwrap_or("");
    requete
        .split('&')
        .filter_map(|p| p.split_once('='))
        .map(|(k, v)| (decoder(k), decoder(v)))
        .collect()
}

fn formulaire(champs: &[(&str, &str)]) -> String {
    champs.iter().map(|(k, v)| format!("{}={}", encoder(k), encoder(v))).collect::<Vec<_>>().join("&")
}

#[derive(Deserialize)]
struct ReponseJeton {
    access_token: String,
    #[serde(default)]
    expires_in: u64,
    #[serde(default)]
    refresh_token: Option<String>,
}

#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct FichierDrive {
    pub id: String,
    pub name: String,
    #[serde(default)]
    pub modified_time: String,
}

#[derive(Deserialize)]
struct ListeFichiers {
    #[serde(default)]
    files: Vec<FichierDrive>,
}

pub struct Google {
    http: reqwest::Client,
    /// Jeton d'accès (valable environ une heure) et sa fin de validité.
    acces: tokio::sync::Mutex<Option<(String, Instant)>>,
    /// Une seule synchronisation à la fois.
    pub verrou: tokio::sync::Mutex<()>,
}

impl Default for Google {
    fn default() -> Self {
        let http = reqwest::Client::builder()
            .connect_timeout(Duration::from_secs(10))
            .timeout(Duration::from_secs(120))
            .build()
            .expect("client HTTP");
        Self { http, acces: tokio::sync::Mutex::new(None), verrou: tokio::sync::Mutex::new(()) }
    }
}

fn erreur_reseau(e: reqwest::Error) -> String {
    if e.is_connect() || e.is_timeout() {
        "Pas de connexion Internet : la synchronisation attendra.".into()
    } else {
        format!("Erreur réseau avec Google : {e}")
    }
}

impl Google {
    pub fn connecte(&self) -> bool {
        secret::lire(secret::JETON_GOOGLE).ok().flatten().is_some()
    }

    /// Connexion : ouvre le navigateur (`ouvrir`) sur la page de Google, attend que l'élève accepte,
    /// puis garde le jeton d'actualisation dans le coffre du système.
    pub async fn connecter(&self, ouvrir: impl FnOnce(&str) -> Result<(), String>) -> Result<(), String> {
        secret_client()?;
        let serveur = tokio::net::TcpListener::bind("127.0.0.1:0").await.map_err(|e| format!("Serveur local : {e}"))?;
        let port = serveur.local_addr().map_err(|e| e.to_string())?.port();
        let retour = format!("http://127.0.0.1:{port}");
        let verificateur = aleatoire(48);
        let defi = base64::engine::general_purpose::URL_SAFE_NO_PAD.encode(Sha256::digest(verificateur.as_bytes()));
        let etat = aleatoire(16);
        let url = format!(
            "{AUTORISATION}?{}",
            formulaire(&[
                ("client_id", CLIENT_ID),
                ("redirect_uri", &retour),
                ("response_type", "code"),
                ("scope", PORTEE),
                ("code_challenge", &defi),
                ("code_challenge_method", "S256"),
                ("state", &etat),
                ("access_type", "offline"),
                ("prompt", "consent"),
            ])
        );
        ouvrir(&url)?;

        let attente = async {
            loop {
                let (mut flux, _) = serveur.accept().await.map_err(|e| e.to_string())?;
                let mut tampon = vec![0u8; 8192];
                let n = flux.read(&mut tampon).await.unwrap_or(0);
                let texte = String::from_utf8_lossy(&tampon[..n]).into_owned();
                let ligne = texte.lines().next().unwrap_or("");
                let p = parametres(ligne);
                let valeur = |k: &str| p.iter().find(|(c, _)| c == k).map(|(_, v)| v.clone());
                // Le navigateur demande parfois aussi /favicon.ico : on l'ignore.
                if valeur("state").is_none() && valeur("error").is_none() {
                    let _ = flux.write_all(b"HTTP/1.1 404 Not Found\r\nContent-Length: 0\r\nConnection: close\r\n\r\n").await;
                    continue;
                }
                let reussi = valeur("code").is_some() && valeur("state").as_deref() == Some(etat.as_str());
                let message = if reussi {
                    "Révisio est connecté à ton Google Drive. Tu peux fermer cet onglet et revenir dans Révisio."
                } else {
                    "La connexion à Google n'a pas abouti. Tu peux fermer cet onglet et réessayer depuis Révisio."
                };
                let page = format!(
                    "<!doctype html><html lang=\"fr\"><meta charset=\"utf-8\"><title>Révisio</title>\
                     <body style=\"font-family:system-ui;max-width:32rem;margin:4rem auto;line-height:1.5\"><h1>Révisio</h1><p>{message}</p></body></html>"
                );
                let reponse = format!(
                    "HTTP/1.1 200 OK\r\nContent-Type: text/html; charset=utf-8\r\nContent-Length: {}\r\nConnection: close\r\n\r\n{page}",
                    page.len()
                );
                let _ = flux.write_all(reponse.as_bytes()).await;
                if let Some(e) = valeur("error") {
                    return Err(if e == "access_denied" { "Connexion refusée dans Google.".to_string() } else { format!("Google a refusé la connexion ({e}).") });
                }
                if !reussi {
                    return Err("Réponse de Google inattendue : réessaie.".to_string());
                }
                return Ok(valeur("code").unwrap_or_default());
            }
        };
        let code = tokio::time::timeout(DELAI_CONNEXION, attente)
            .await
            .map_err(|_| "Connexion à Google abandonnée (rien reçu en 5 minutes).".to_string())??;

        let corps = formulaire(&[
            ("client_id", CLIENT_ID),
            ("client_secret", secret_client()?),
            ("code", &code),
            ("code_verifier", &verificateur),
            ("redirect_uri", &retour),
            ("grant_type", "authorization_code"),
        ]);
        let jeton = self.demander_jeton(corps).await?;
        let actualisation = jeton.refresh_token.ok_or("Google n'a pas donné d'accès durable : réessaie.")?;
        secret::enregistrer(secret::JETON_GOOGLE, &actualisation)?;
        *self.acces.lock().await = Some((jeton.access_token, Instant::now() + Duration::from_secs(jeton.expires_in.max(60))));
        Ok(())
    }

    async fn demander_jeton(&self, corps: String) -> Result<ReponseJeton, String> {
        let r = self
            .http
            .post(JETON)
            .header("Content-Type", "application/x-www-form-urlencoded")
            .body(corps)
            .send()
            .await
            .map_err(erreur_reseau)?;
        let statut = r.status();
        let texte = r.text().await.map_err(erreur_reseau)?;
        if statut.is_success() {
            return serde_json::from_str(&texte).map_err(|e| format!("Réponse de Google illisible : {e}"));
        }
        if texte.contains("invalid_grant") {
            let _ = secret::supprimer(secret::JETON_GOOGLE);
            return Err(EXPIREE.into());
        }
        Err(format!("Google a refusé ({statut}) : {}", texte.chars().take(300).collect::<String>()))
    }

    /// Jeton d'accès valide (renouvelé avec le jeton d'actualisation quand il a expiré).
    async fn jeton(&self) -> Result<String, String> {
        let mut acces = self.acces.lock().await;
        if let Some((j, fin)) = acces.as_ref() {
            if Instant::now() + Duration::from_secs(60) < *fin {
                return Ok(j.clone());
            }
        }
        let actualisation = secret::lire(secret::JETON_GOOGLE)?.ok_or("Google Drive n'est pas connecté.")?;
        let corps = formulaire(&[
            ("client_id", CLIENT_ID),
            ("client_secret", secret_client()?),
            ("refresh_token", &actualisation),
            ("grant_type", "refresh_token"),
        ]);
        let r = self.demander_jeton(corps).await?;
        *acces = Some((r.access_token.clone(), Instant::now() + Duration::from_secs(r.expires_in.max(60))));
        Ok(r.access_token)
    }

    /// Déconnexion : le jeton est retiré chez Google (si possible) et du coffre du système.
    pub async fn deconnecter(&self) -> Result<(), String> {
        if let Ok(Some(j)) = secret::lire(secret::JETON_GOOGLE) {
            let _ = self
                .http
                .post("https://oauth2.googleapis.com/revoke")
                .header("Content-Type", "application/x-www-form-urlencoded")
                .body(formulaire(&[("token", &j)]))
                .send()
                .await;
        }
        *self.acces.lock().await = None;
        secret::supprimer(secret::JETON_GOOGLE)
    }

    /// Envoie une requête authentifiée ; un 401 (jeton refusé) renouvelle le jeton une fois.
    async fn requete(&self, faire: impl Fn(&str) -> reqwest::RequestBuilder) -> Result<reqwest::Response, String> {
        for essai in 0..2 {
            let jeton = self.jeton().await?;
            let r = faire(&jeton).send().await.map_err(erreur_reseau)?;
            if r.status() == reqwest::StatusCode::UNAUTHORIZED && essai == 0 {
                *self.acces.lock().await = None;
                continue;
            }
            if !r.status().is_success() {
                let statut = r.status();
                let texte = r.text().await.unwrap_or_default();
                if statut == reqwest::StatusCode::FORBIDDEN && texte.contains("insufficient") {
                    return Err(EXPIREE.into());
                }
                return Err(format!("Google Drive a refusé ({statut}) : {}", texte.chars().take(300).collect::<String>()));
            }
            return Ok(r);
        }
        Err(EXPIREE.into())
    }

    /// Fichiers du dossier caché de Révisio.
    pub async fn lister(&self) -> Result<Vec<FichierDrive>, String> {
        let r = self
            .requete(|j| {
                self.http
                    .get(format!(
                        "{DRIVE}?{}",
                        formulaire(&[("spaces", "appDataFolder"), ("fields", "files(id,name,modifiedTime)"), ("pageSize", "1000")])
                    ))
                    .bearer_auth(j)
            })
            .await?;
        let liste: ListeFichiers = r.json().await.map_err(|e| format!("Liste des fichiers illisible : {e}"))?;
        Ok(liste.files)
    }

    pub async fn telecharger(&self, id: &str) -> Result<Vec<u8>, String> {
        let url = format!("{DRIVE}/{}?alt=media", encoder(id));
        let r = self.requete(|j| self.http.get(&url).bearer_auth(j)).await?;
        Ok(r.bytes().await.map_err(erreur_reseau)?.to_vec())
    }

    /// Crée le fichier `nom` dans le dossier caché, ou remplace le contenu du fichier `existant`.
    pub async fn envoyer(&self, nom: &str, existant: Option<&str>, octets: Vec<u8>) -> Result<(), String> {
        match existant {
            Some(id) => {
                let url = format!("{ENVOI}/{}?uploadType=media", encoder(id));
                self.requete(|j| {
                    self.http
                        .patch(&url)
                        .bearer_auth(j)
                        .header("Content-Type", "application/gzip")
                        .body(octets.clone())
                })
                .await?;
            }
            None => {
                let limite = format!("revisio{}", aleatoire(12));
                let metadonnees = serde_json::json!({ "name": nom, "parents": ["appDataFolder"] }).to_string();
                let mut corps = format!(
                    "--{limite}\r\nContent-Type: application/json; charset=UTF-8\r\n\r\n{metadonnees}\r\n--{limite}\r\nContent-Type: application/gzip\r\n\r\n"
                )
                .into_bytes();
                corps.extend_from_slice(&octets);
                corps.extend_from_slice(format!("\r\n--{limite}--").as_bytes());
                self.requete(|j| {
                    self.http
                        .post(format!("{ENVOI}?uploadType=multipart"))
                        .bearer_auth(j)
                        .header("Content-Type", format!("multipart/related; boundary={limite}"))
                        .body(corps.clone())
                })
                .await?;
            }
        }
        Ok(())
    }
}

impl Google {
    pub async fn supprimer(&self, id: &str) -> Result<(), String> {
        let url = format!("{DRIVE}/{}", encoder(id));
        self.requete(|j| self.http.delete(&url).bearer_auth(j)).await?;
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Essai réel avec le compte connecté sur ce PC (lancé à la main : `cargo test -- --ignored essai_drive`).
    /// `REVISIO_ESSAI=deposer` dépose un journal factice, `REVISIO_ESSAI=effacer` l'efface ; sinon, lecture seule.
    #[tokio::test]
    #[ignore]
    async fn essai_drive() {
        let g = Google::default();
        let fichiers = g.lister().await.unwrap();
        for f in &fichiers {
            println!("{} {} {}", f.name, f.id, f.modified_time);
        }
        if let Some(f) = fichiers.iter().find(|f| f.name == crate::synchro::FICHIER_CONTENU) {
            let json = crate::synchro::decompresser(&g.telecharger(&f.id).await.unwrap()).unwrap();
            let c: crate::synchro::Contenu = serde_json::from_slice(&json).unwrap();
            println!("contenu : {} cartes, {} révisions, {} erreurs, {} notes", c.cartes.len(), c.revisions.len(), c.erreurs.len(), c.notes.len());
            if let Ok(sortie) = std::env::var("REVISIO_ESSAI_SORTIE") {
                std::fs::write(sortie, &json).unwrap();
            }
        }
        match std::env::var("REVISIO_ESSAI").as_deref() {
            Ok("deposer") => {
                let j = crate::synchro::Journal {
                    version: 1,
                    appareil: "essai".into(),
                    nom: "Essai".into(),
                    revisions: vec![crate::synchro::RevisionSynchro {
                        uid: "essai:1".into(),
                        cle: "carte-qui-n-existe-pas".into(),
                        date: 1,
                        note: 3,
                        confiance: "sur".into(),
                        correcte: true,
                        reponse_donnee: None,
                        duree_ms: 0,
                    }],
                    ..Default::default()
                };
                let gz = crate::synchro::compresser(&serde_json::to_vec(&j).unwrap());
                g.envoyer("journal-essai.json.gz", None, gz).await.unwrap();
                println!("journal factice déposé");
            }
            Ok("effacer") => {
                for f in fichiers.iter().filter(|f| f.name == "journal-essai.json.gz") {
                    g.supprimer(&f.id).await.unwrap();
                    println!("journal factice effacé");
                }
            }
            _ => {}
        }
    }

    #[test]
    fn parametres_de_retour_lus() {
        let p = parametres("GET /?state=abc&code=4%2F0Ab_c-d&scope=https%3A%2F%2Fwww.googleapis.com%2Fauth%2Fdrive.appdata HTTP/1.1");
        assert_eq!(p[0], ("state".into(), "abc".into()));
        assert_eq!(p[1], ("code".into(), "4/0Ab_c-d".into()));
        assert_eq!(p[2].1, "https://www.googleapis.com/auth/drive.appdata");
        assert!(parametres("GET /favicon.ico HTTP/1.1").is_empty());
    }

    #[test]
    fn encodage_et_aleatoire() {
        assert_eq!(encoder("a b/é"), "a%20b%2F%C3%A9");
        assert_eq!(decoder(&encoder("a b/é~")), "a b/é~");
        let (a, b) = (aleatoire(48), aleatoire(48));
        assert_ne!(a, b);
        assert_eq!(a.len(), 64);
    }
}
