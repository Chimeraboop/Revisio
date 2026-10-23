//! Clé API Gemini, rangée dans le Gestionnaire d'identification Windows
//! (« Informations d'identification Windows »). Elle n'est jamais écrite dans un fichier
//! et ne passe jamais par l'interface : seul ce module la lit.

use std::sync::Once;

use keyring_core::{Entry, Error};

const SERVICE: &str = "Revisio";
const COMPTE: &str = "cle-api-gemini";

static INITIALISATION: Once = Once::new();

fn entree() -> Result<Entry, String> {
    INITIALISATION.call_once(|| match windows_native_keyring_store::Store::new() {
        Ok(store) => keyring_core::set_default_store(store),
        Err(e) => eprintln!("Gestionnaire d'identification Windows indisponible : {e}"),
    });
    Entry::new(SERVICE, COMPTE).map_err(|e| format!("Gestionnaire d'identification Windows : {e}"))
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
