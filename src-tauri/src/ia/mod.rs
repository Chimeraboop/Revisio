//! Intelligence artificielle : Gemini (en ligne) et l'IA locale Ollama (sur le PC, phase 6).
//! Clé, limites de débit, journal, réponses JSON validées.
//!
//! Règles du cahier des charges appliquées ici :
//! - sorties en JSON structuré validées, avec réessai si invalide ;
//! - limitation du débit pour rester dans le quota gratuit ;
//! - toute erreur (hors ligne, quota…) devient un message clair, sans jamais planter ;
//! - les tâches simples peuvent passer par l'IA locale (toujours, ou en secours quand Gemini est indisponible).

pub mod ancrage;
pub mod copies;
pub mod correction;
pub mod exercices;
pub mod gemini;
pub mod ollama;
pub mod photos;
pub mod questions;
pub mod vision;

use std::collections::VecDeque;
use std::sync::Mutex;
use std::time::{Duration, Instant};

use rusqlite::{params, Connection};
use serde::de::DeserializeOwned;
use serde::Serialize;

use std::future::Future;

use gemini::{ErreurIa, Gemini, Partie, Reponse, ReponseVoix, Requete, RequeteVoix};

use crate::{maintenant_ms, secret};

/// Quand utiliser l'IA locale pour les tâches simples.
#[derive(Debug, Clone, Copy, PartialEq)]
pub enum ModeLocal {
    /// Seulement quand Gemini est indisponible (pas d'Internet, pas de clé, quota épuisé, surcharge).
    Secours,
    /// Toujours (rien ne part chez Google pour ces tâches).
    Toujours,
}

/// IA locale choisie dans les réglages (absente si désactivée ou si aucun modèle n'est choisi).
#[derive(Debug, Clone, PartialEq)]
pub struct ConfigLocale {
    pub adresse: String,
    pub modele: String,
    pub mode: ModeLocal,
}

/// Préfixe des modèles locaux dans le journal et les contenus générés (« local:qwen3.5:4b »).
pub const PREFIXE_LOCAL: &str = "local:";

/// Limites réglables (valeurs par défaut prudentes pour le niveau gratuit).
#[derive(Debug, Clone, Copy)]
pub struct Limites {
    pub par_minute: u32,
    pub par_jour: u32,
}

pub struct ServiceIa {
    client: Gemini,
    pub local: ollama::Ollama,
    /// Mode local : plus aucun appel à Gemini.
    gemini_coupe: std::sync::atomic::AtomicBool,
    /// Modèle local en cours de téléchargement, et demande d'arrêt.
    pub telechargement: Mutex<Option<String>>,
    pub annuler_telechargement: std::sync::atomic::AtomicBool,
    /// La clé est lue une fois dans le Gestionnaire d'identification Windows puis gardée en mémoire.
    cle: Mutex<Option<String>>,
    appels_recents: tokio::sync::Mutex<VecDeque<tokio::time::Instant>>,
}

impl Default for ServiceIa {
    fn default() -> Self {
        Self {
            client: Gemini::new(),
            local: ollama::Ollama::new(),
            gemini_coupe: std::sync::atomic::AtomicBool::new(false),
            telechargement: Mutex::new(None),
            annuler_telechargement: std::sync::atomic::AtomicBool::new(false),
            cle: Mutex::new(None),
            appels_recents: tokio::sync::Mutex::new(VecDeque::new()),
        }
    }
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct EtatIa {
    pub cle_presente: bool,
    pub appels_aujourdhui: i64,
    pub jetons_aujourdhui: i64,
}

impl ServiceIa {
    /// Interrupteur « En ligne » : `true` coupe Gemini (mode local).
    pub fn couper_gemini(&self, coupe: bool) {
        self.gemini_coupe.store(coupe, std::sync::atomic::Ordering::SeqCst);
    }

    pub fn gemini_coupe(&self) -> bool {
        self.gemini_coupe.load(std::sync::atomic::Ordering::SeqCst)
    }

    fn cle(&self) -> Result<String, ErreurIa> {
        let mut cache = self.cle.lock().unwrap();
        if cache.is_none() {
            *cache = secret::lire_cle().map_err(ErreurIa::Serveur)?;
        }
        cache.clone().ok_or(ErreurIa::PasDeCle)
    }

    pub fn cle_presente(&self) -> bool {
        self.cle().is_ok()
    }

    pub fn enregistrer_cle(&self, cle: &str) -> Result<(), String> {
        secret::enregistrer_cle(cle)?;
        *self.cle.lock().unwrap() = Some(cle.trim().to_string());
        Ok(())
    }

    pub fn supprimer_cle(&self) -> Result<(), String> {
        secret::supprimer_cle()?;
        *self.cle.lock().unwrap() = None;
        Ok(())
    }

    pub async fn lister_modeles(&self) -> Result<Vec<gemini::ModeleInfo>, ErreurIa> {
        let cle = self.cle()?;
        self.client.lister_modeles(&cle).await
    }

    /// Internet (le serveur de Gemini) est-il vraiment joignable ? Aucune clé n'est lue ni envoyée.
    pub async fn internet_joignable(&self) -> bool {
        self.client.joignable().await
    }

    /// Vérifie une clé avant de l'enregistrer (une requête légère : la liste des modèles).
    pub async fn verifier_cle(&self, cle: &str) -> Result<(), ErreurIa> {
        self.client.lister_modeles(cle.trim()).await.map(|_| ())
    }

    /// Attend qu'une place se libère dans la limite par minute.
    async fn attendre_place(&self, par_minute: u32) {
        let fenetre = Duration::from_secs(60);
        loop {
            let mut appels = self.appels_recents.lock().await;
            let maintenant = tokio::time::Instant::now();
            while appels.front().is_some_and(|t| maintenant.duration_since(*t) >= fenetre) {
                appels.pop_front();
            }
            if (appels.len() as u32) < par_minute.max(1) {
                appels.push_back(maintenant);
                return;
            }
            let attente = fenetre - maintenant.duration_since(*appels.front().unwrap());
            drop(appels);
            tokio::time::sleep(attente).await;
        }
    }

    /// Un appel brut à Gemini (texte), avec limites et journal.
    pub async fn appeler(
        &self,
        db: &Mutex<Connection>,
        usage: &str,
        limites: Limites,
        requete: &Requete,
    ) -> Result<Reponse, ErreurIa> {
        self.executer(db, usage, &requete.modele, limites, |cle| async move { self.client.generer(&cle, requete).await }, |r| {
            (r.jetons_entree, r.jetons_sortie)
        })
        .await
    }

    /// Synthèse vocale (podcasts), avec les mêmes limites et le même journal.
    pub async fn appeler_voix(
        &self,
        db: &Mutex<Connection>,
        usage: &str,
        limites: Limites,
        requete: &RequeteVoix,
    ) -> Result<ReponseVoix, ErreurIa> {
        self.executer(db, usage, &requete.modele, limites, |cle| async move { self.client.generer_voix(&cle, requete).await }, |r| {
            (r.jetons_entree, r.jetons_sortie)
        })
        .await
    }

    /// Plafond du jour, limite par minute, journal. Si Google est momentanément surchargé,
    /// on réessaie deux fois (après 5 puis 15 secondes) avant d'abandonner.
    async fn executer<R, F, Fut>(
        &self,
        db: &Mutex<Connection>,
        usage: &str,
        modele: &str,
        limites: Limites,
        appel: F,
        jetons: impl Fn(&R) -> (i64, i64),
    ) -> Result<R, ErreurIa>
    where
        F: Fn(String) -> Fut,
        Fut: Future<Output = Result<R, ErreurIa>>,
    {
        if self.gemini_coupe() {
            return Err(ErreurIa::GeminiCoupe);
        }
        let cle = self.cle()?;
        let mut attente = Duration::from_secs(5);
        let mut essai = 0;
        loop {
            let deja = appels_du_jour(&db.lock().unwrap()).unwrap_or(0);
            if deja >= limites.par_jour as i64 {
                return Err(ErreurIa::Quota(format!(
                    "plafond de {} appels par jour réglé dans Révisio (modifiable dans les Réglages).",
                    limites.par_jour
                )));
            }
            self.attendre_place(limites.par_minute).await;
            let debut = Instant::now();
            let resultat = appel(cle.clone()).await;
            let duree = debut.elapsed().as_millis() as i64;
            let bilan = resultat.as_ref().map(&jetons);
            let _ = journaliser(&db.lock().unwrap(), usage, modele, bilan, duree);
            match resultat {
                Err(ErreurIa::Surcharge) if essai < 2 => {
                    essai += 1;
                    tokio::time::sleep(attente).await;
                    attente *= 3;
                }
                r => return r,
            }
        }
    }

    /// Appel avec sortie JSON (Gemini) : le texte est lu comme `T` puis vérifié par `valider` ;
    /// en cas d'échec, on redemande (au plus 3 essais en tout) en expliquant le problème.
    pub async fn appeler_json<T: DeserializeOwned>(
        &self,
        db: &Mutex<Connection>,
        usage: &str,
        limites: Limites,
        requete: &Requete,
        valider: impl Fn(&T) -> Result<(), String>,
    ) -> Result<T, ErreurIa> {
        json_valide(requete, |r| async move { self.appeler(db, usage, limites, &r).await }, valider).await
    }

    /// Un appel à l'IA locale (pas de clé ni de quota ; noté dans le journal avec le préfixe « local: »).
    pub async fn appeler_local(
        &self,
        db: &Mutex<Connection>,
        usage: &str,
        local: &ConfigLocale,
        requete: &Requete,
    ) -> Result<Reponse, ErreurIa> {
        let debut = Instant::now();
        let resultat = self.local.generer(&local.adresse, &local.modele, requete).await;
        let bilan = resultat.as_ref().map(|r| (r.jetons_entree, r.jetons_sortie));
        let modele = format!("{PREFIXE_LOCAL}{}", local.modele);
        let _ = journaliser(&db.lock().unwrap(), usage, &modele, bilan, debut.elapsed().as_millis() as i64);
        resultat
    }

    /// Conversation : réponse en texte libre (Markdown) lue au fil de l'eau, par Gemini ou l'IA locale selon
    /// le réglage (même logique que `appeler_json_simple` : secours local si Gemini est indisponible).
    /// `flux` reçoit le texte reçu jusqu'ici (il repart de zéro si Gemini doit être redemandé).
    /// Renvoie aussi le modèle qui a vraiment répondu (« local:… » pour l'IA locale).
    pub async fn appeler_texte_flux(
        &self,
        db: &Mutex<Connection>,
        usage: &str,
        limites: Limites,
        local: Option<&ConfigLocale>,
        requete: &Requete,
        flux: &(dyn Fn(&str) + Sync),
    ) -> Result<(Reponse, String), ErreurIa> {
        if let Some(l) = local.filter(|l| l.mode == ModeLocal::Toujours) {
            return self.local_flux(db, usage, l, requete, flux).await;
        }
        let gemini = self
            .executer(db, usage, &requete.modele, limites, |cle| async move { self.client.generer_flux(&cle, requete, flux).await }, |r| {
                (r.jetons_entree, r.jetons_sortie)
            })
            .await;
        match gemini {
            Ok(r) => Ok((r, requete.modele.clone())),
            Err(e) if e.justifie_le_secours() => match local {
                Some(l) => self.local_flux(db, usage, l, requete, flux).await.map_err(|locale| ErreurIa::Secours { gemini: Box::new(e), locale: Box::new(locale) }),
                None => Err(e),
            },
            Err(e) => Err(e),
        }
    }

    /// Réponse de l'IA locale au fil de l'eau, notée dans le journal (préfixe « local: »).
    async fn local_flux(
        &self,
        db: &Mutex<Connection>,
        usage: &str,
        l: &ConfigLocale,
        requete: &Requete,
        flux: &(dyn Fn(&str) + Sync),
    ) -> Result<(Reponse, String), ErreurIa> {
        let debut = Instant::now();
        let resultat = self.local.generer_flux(&l.adresse, &l.modele, requete, flux).await;
        let bilan = resultat.as_ref().map(|r| (r.jetons_entree, r.jetons_sortie));
        let modele = format!("{PREFIXE_LOCAL}{}", l.modele);
        let _ = journaliser(&db.lock().unwrap(), usage, &modele, bilan, debut.elapsed().as_millis() as i64);
        resultat.map(|r| (r, modele))
    }

    /// Appel JSON à l'IA locale (mêmes vérifications et réessais qu'avec Gemini).
    pub async fn appeler_json_local<T: DeserializeOwned>(
        &self,
        db: &Mutex<Connection>,
        usage: &str,
        local: &ConfigLocale,
        requete: &Requete,
        valider: impl Fn(&T) -> Result<(), String>,
    ) -> Result<T, ErreurIa> {
        json_valide(requete, |r| async move { self.appeler_local(db, usage, local, &r).await }, valider).await
    }

    /// Tâche simple (questions, corrections, Feynman, cartes mentales) : Gemini ou l'IA locale selon le réglage.
    /// En mode secours, si Gemini est indisponible, la même demande part vers l'IA locale.
    /// Renvoie aussi le modèle qui a vraiment répondu (« local:… » pour l'IA locale).
    pub async fn appeler_json_simple<T: DeserializeOwned>(
        &self,
        db: &Mutex<Connection>,
        usage: &str,
        limites: Limites,
        local: Option<&ConfigLocale>,
        requete: &Requete,
        valider: impl Fn(&T) -> Result<(), String>,
    ) -> Result<(T, String), ErreurIa> {
        let nom_local = |l: &ConfigLocale| format!("{PREFIXE_LOCAL}{}", l.modele);
        match local {
            Some(l) if l.mode == ModeLocal::Toujours => {
                self.appeler_json_local(db, usage, l, requete, &valider).await.map(|v| (v, nom_local(l)))
            }
            _ => match self.appeler_json(db, usage, limites, requete, &valider).await {
                Ok(v) => Ok((v, requete.modele.clone())),
                Err(e) if e.justifie_le_secours() => match local {
                    Some(l) => match self.appeler_json_local(db, usage, l, requete, &valider).await {
                        Ok(v) => Ok((v, nom_local(l))),
                        Err(locale) => Err(ErreurIa::Secours { gemini: Box::new(e), locale: Box::new(locale) }),
                    },
                    None => Err(e),
                },
                Err(e) => Err(e),
            },
        }
    }
}

/// Demande une réponse JSON, la lit comme `T` et la vérifie ; si elle ne convient pas, redemande
/// (3 essais en tout) en expliquant le problème au modèle. `appel` envoie la requête (Gemini ou local).
async fn json_valide<T, F, Fut>(requete: &Requete, appel: F, valider: impl Fn(&T) -> Result<(), String>) -> Result<T, ErreurIa>
where
    T: DeserializeOwned,
    F: Fn(Requete) -> Fut,
    Fut: Future<Output = Result<Reponse, ErreurIa>>,
{
    let mut requete = requete.clone();
    let mut derniere_erreur = String::new();
    for essai in 0..3 {
        if essai > 0 {
            requete.parties.push(Partie::Texte(format!(
                "Ta réponse précédente était invalide ({derniere_erreur}). Réponds de nouveau en respectant exactement le schéma JSON et les consignes."
            )));
        }
        let reponse = appel(requete.clone()).await?;
        match serde_json::from_str::<T>(&reponse.texte) {
            Ok(valeur) => match valider(&valeur) {
                Ok(()) => return Ok(valeur),
                Err(e) => derniere_erreur = e,
            },
            Err(e) => derniere_erreur = format!("JSON illisible : {e}"),
        }
    }
    Err(ErreurIa::Format(derniere_erreur))
}

/// Appels à Gemini de la journée (jour local, comme les quotas affichés) ; l'IA locale ne compte pas.
pub fn appels_du_jour(conn: &Connection) -> rusqlite::Result<i64> {
    conn.query_row(
        "SELECT count(*) FROM journal_ia
         WHERE date(date / 1000, 'unixepoch', 'localtime') = date('now', 'localtime') AND modele NOT LIKE 'local:%'",
        [],
        |r| r.get(0),
    )
}

pub fn etat(conn: &Connection, cle_presente: bool) -> rusqlite::Result<EtatIa> {
    let (appels, jetons): (i64, i64) = conn.query_row(
        "SELECT count(*), coalesce(sum(jetons_entree + jetons_sortie), 0) FROM journal_ia
         WHERE date(date / 1000, 'unixepoch', 'localtime') = date('now', 'localtime') AND modele NOT LIKE 'local:%'",
        [],
        |r| Ok((r.get(0)?, r.get(1)?)),
    )?;
    Ok(EtatIa { cle_presente, appels_aujourdhui: appels, jetons_aujourdhui: jetons })
}

/// `bilan` : jetons (entrée, sortie) si l'appel a réussi, sinon l'erreur.
fn journaliser(
    conn: &Connection,
    usage: &str,
    modele: &str,
    bilan: Result<(i64, i64), &ErreurIa>,
    duree_ms: i64,
) -> rusqlite::Result<()> {
    let (statut, entree, sortie, erreur) = match bilan {
        Ok((entree, sortie)) => ("ok", entree, sortie, None),
        Err(e) => ("erreur", 0, 0, Some(e.to_string())),
    };
    conn.execute(
        "INSERT INTO journal_ia(date, usage, modele, statut, jetons_entree, jetons_sortie, duree_ms, erreur)
         VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8)",
        params![maintenant_ms(), usage, modele, statut, entree, sortie, duree_ms, erreur],
    )?;
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::db;

    #[test]
    fn journal_et_appels_du_jour() {
        let conn = db::ouvrir_en_memoire().unwrap();
        journaliser(&conn, "test", "m", Ok((10, 5)), 3).unwrap();
        journaliser(&conn, "test", "m", Err(&ErreurIa::HorsLigne), 1).unwrap();
        journaliser(&conn, "test", "local:qwen3.5:4b", Ok((100, 50)), 3).unwrap();
        assert_eq!(appels_du_jour(&conn).unwrap(), 2, "l'IA locale ne compte pas dans le quota Gemini");
        let e = etat(&conn, true).unwrap();
        assert_eq!((e.appels_aujourdhui, e.jetons_aujourdhui), (2, 15));
    }

    /// Faux Ollama : répond aux requêtes successives avec les contenus donnés (une connexion par requête).
    fn faux_ollama(contenus: Vec<&'static str>) -> String {
        use std::io::{BufRead, BufReader, Read, Write};
        let ecoute = std::net::TcpListener::bind("127.0.0.1:0").unwrap();
        let adresse = format!("http://{}", ecoute.local_addr().unwrap());
        std::thread::spawn(move || {
            for (flux, contenu) in ecoute.incoming().zip(contenus) {
                let mut flux = flux.unwrap();
                let mut lecteur = BufReader::new(flux.try_clone().unwrap());
                let mut longueur = 0;
                loop {
                    let mut ligne = String::new();
                    lecteur.read_line(&mut ligne).unwrap();
                    if let Some(v) = ligne.to_ascii_lowercase().strip_prefix("content-length:") {
                        longueur = v.trim().parse().unwrap();
                    }
                    if ligne == "\r\n" {
                        break;
                    }
                }
                let mut corps = vec![0; longueur];
                lecteur.read_exact(&mut corps).unwrap();
                let reponse = serde_json::json!({ "message": { "content": contenu }, "prompt_eval_count": 10, "eval_count": 5 })
                    .to_string();
                write!(
                    flux,
                    "HTTP/1.1 200 OK\r\nContent-Type: application/json\r\nContent-Length: {}\r\nConnection: close\r\n\r\n{reponse}",
                    reponse.len()
                )
                .unwrap();
            }
        });
        adresse
    }

    #[tokio::test(flavor = "current_thread")]
    async fn ia_locale_reessaie_et_ne_compte_pas_dans_le_quota() {
        #[derive(serde::Deserialize)]
        struct Reponse {
            note: u8,
        }
        let db = Mutex::new(db::ouvrir_en_memoire().unwrap());
        let service = ServiceIa::default();
        // 1re réponse hors limites (refusée par la validation), 2e correcte.
        let local = ConfigLocale {
            adresse: faux_ollama(vec![r#"{"note": 42}"#, r#"{"note": 15}"#]),
            modele: "faux:1b".into(),
            mode: ModeLocal::Toujours,
        };
        let requete = Requete { modele: "gemini-inutilise".into(), parties: vec![Partie::Texte("Note ?".into())], ..Default::default() };
        let valider = |r: &Reponse| if r.note <= 20 { Ok(()) } else { Err("note sur 20".into()) };
        let (reponse, modele) = service
            .appeler_json_simple(&db, "test", Limites { par_minute: 1, par_jour: 1 }, Some(&local), &requete, valider)
            .await
            .unwrap();
        assert_eq!((reponse.note, modele.as_str()), (15, "local:faux:1b"));
        let conn = db.lock().unwrap();
        let locaux: i64 = conn.query_row("SELECT count(*) FROM journal_ia WHERE modele = 'local:faux:1b'", [], |r| r.get(0)).unwrap();
        assert_eq!((locaux, appels_du_jour(&conn).unwrap()), (2, 0));
    }

    #[tokio::test(flavor = "current_thread")]
    async fn mode_local_coupe_gemini() {
        #[derive(serde::Deserialize)]
        struct Reponse {
            note: u8,
        }
        let db = Mutex::new(db::ouvrir_en_memoire().unwrap());
        let service = ServiceIa::default();
        service.couper_gemini(true);
        let requete = Requete { modele: "gemini".into(), parties: vec![Partie::Texte("Note ?".into())], ..Default::default() };
        let limites = Limites { par_minute: 5, par_jour: 5 };
        // Sans IA locale : refus clair, sans aucun appel (ni réseau, ni clé lue).
        let sans_local = service.appeler_json_simple(&db, "test", limites, None, &requete, |_: &Reponse| Ok(())).await;
        assert!(matches!(sans_local, Err(ErreurIa::GeminiCoupe)));
        // Avec une IA locale, même en mode secours : c'est elle qui répond.
        let local = ConfigLocale { adresse: faux_ollama(vec![r#"{"note": 12}"#]), modele: "faux:1b".into(), mode: ModeLocal::Secours };
        let (r, modele) =
            service.appeler_json_simple(&db, "test", limites, Some(&local), &requete, |_: &Reponse| Ok(())).await.unwrap();
        assert_eq!((r.note, modele.as_str()), (12, "local:faux:1b"));
        assert_eq!(appels_du_jour(&db.lock().unwrap()).unwrap(), 0, "Gemini n'a jamais été appelé");
    }

    #[tokio::test(flavor = "current_thread")]
    async fn ollama_absent() {
        let service = ServiceIa::default();
        // Port fermé : message clair, pas de plantage.
        let e = service.local.version("http://127.0.0.1:9").await.unwrap_err();
        assert_eq!(e, ErreurIa::OllamaAbsent);
        assert!(e.to_string().contains("Ollama"));
    }

    #[tokio::test(flavor = "current_thread", start_paused = true)]
    async fn limite_par_minute() {
        let service = ServiceIa::default();
        let debut = tokio::time::Instant::now();
        for _ in 0..3 {
            service.attendre_place(2).await;
        }
        // Le 3e appel a dû attendre la fin de la fenêtre d'une minute.
        assert!(tokio::time::Instant::now().duration_since(debut) >= Duration::from_secs(59));
    }
}
