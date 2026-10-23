//! Clé API Gemini, rangée dans le coffre à secrets du système : le Gestionnaire d'identification
//! sous Windows, le Trousseau sous macOS. Elle n'est jamais écrite dans un fichier
//! et ne passe jamais par l'interface : seul ce module la lit.

use std::sync::Once;

use keyring_core::{Entry, Error};

const SERVICE: &str = "Revisio";
const COMPTE: &str = "cle-api-gemini";

/// Nom du coffre à secrets, pour les messages.
#[cfg(windows)]
pub const NOM_COFFRE: &str = "Gestionnaire d'identification Windows";
#[cfg(target_os = "macos")]
pub const NOM_COFFRE: &str = "Trousseau de macOS";
#[cfg(not(any(windows, target_os = "macos")))]
compile_error!("Révisio fonctionne sous Windows et macOS seulement.");

static INITIALISATION: Once = Once::new();

fn entree() -> Result<Entry, String> {
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
    Entry::new(SERVICE, COMPTE).map_err(|e| format!("{NOM_COFFRE} : {e}"))
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
