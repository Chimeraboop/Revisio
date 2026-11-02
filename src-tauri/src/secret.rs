//! Clé API Gemini et jeton d'accès à Google Drive (synchronisation), rangés dans le coffre à secrets
//! du système : le Gestionnaire d'identification sous Windows, le Trousseau sous macOS. Ils ne sont
//! jamais écrits dans un fichier et ne passent jamais par l'interface : seul ce module les lit.

use std::sync::Once;

use keyring_core::{Entry, Error};

const SERVICE: &str = "Revisio";
const COMPTE: &str = "cle-api-gemini";
/// Jeton d'actualisation Google (Drive, dossier propre à Révisio).
pub const JETON_GOOGLE: &str = "jeton-google-drive";

/// Nom du coffre à secrets, pour les messages.
#[cfg(windows)]
pub const NOM_COFFRE: &str = "Gestionnaire d'identification Windows";
#[cfg(target_os = "macos")]
pub const NOM_COFFRE: &str = "Trousseau de macOS";
#[cfg(not(any(windows, target_os = "macos")))]
compile_error!("Révisio fonctionne sous Windows et macOS seulement.");

static INITIALISATION: Once = Once::new();

fn entree() -> Result<Entry, String> {
    entree_de(COMPTE)
}

fn entree_de(compte: &str) -> Result<Entry, String> {
    INITIALISATION.call_once(|| {
        #[cfg(windows)]
        let store = windows_native_keyring_store::Store::new();
        #[cfg(target_os = "macos")]
        let store = apple_native_keyring_store::keychain::Store::new();
        match store {
            Ok(store) => keyring_core::set_default_store(store),
            Err(e) => eprintln!("{NOM_COFFRE} indisponible : {e}"),
        }
    });
    Entry::new(SERVICE, compte).map_err(|e| format!("{NOM_COFFRE} : {e}"))
}

pub fn enregistrer_cle(cle: &str) -> Result<(), String> {
    let cle = cle.trim();
    if cle.is_empty() {
        return Err("La clé est vide.".into());
    }
    entree()?.set_password(cle).map_err(|e| format!("Impossible d'enregistrer la clé : {e}"))
}

/// La clé, ou `None` si aucune n'est enregistrée.
pub fn lire_cle() -> Result<Option<String>, String> {
    match entree()?.get_password() {
        Ok(cle) => Ok(Some(cle)),
        Err(Error::NoEntry) => Ok(None),
        Err(e) => Err(format!("Impossible de lire la clé : {e}")),
    }
}

pub fn supprimer_cle() -> Result<(), String> {
    match entree()?.delete_credential() {
        Ok(()) | Err(Error::NoEntry) => Ok(()),
        Err(e) => Err(format!("Impossible de supprimer la clé : {e}")),
    }
}

/// Autre secret que la clé Gemini (`compte` : par ex. `JETON_GOOGLE`).
pub fn enregistrer(compte: &str, valeur: &str) -> Result<(), String> {
    entree_de(compte)?.set_password(valeur).map_err(|e| format!("Impossible d'enregistrer dans le {NOM_COFFRE} : {e}"))
}

pub fn lire(compte: &str) -> Result<Option<String>, String> {
    match entree_de(compte)?.get_password() {
        Ok(v) => Ok(Some(v)),
        Err(Error::NoEntry) => Ok(None),
        Err(e) => Err(format!("Impossible de lire dans le {NOM_COFFRE} : {e}")),
    }
}

pub fn supprimer(compte: &str) -> Result<(), String> {
    match entree_de(compte)?.delete_credential() {
        Ok(()) | Err(Error::NoEntry) => Ok(()),
        Err(e) => Err(format!("Impossible de supprimer du {NOM_COFFRE} : {e}")),
    }
}
