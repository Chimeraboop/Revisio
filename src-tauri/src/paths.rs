//! Emplacements des données de l'app et garde-fou « jamais dans le vault ».
//!
//! Toutes les écritures de Révisio (réglages, base, cache…) passent par
//! `verifier_hors_vault` avant d'écrire : si le chemin cible est dans le vault,
//! l'écriture est refusée.

use std::path::{Component, Path, PathBuf};

/// Nom du dossier dans %APPDATA% (sans accent pour éviter tout souci de chemin).
pub const NOM_DOSSIER_APP: &str = "Revisio";

/// `%APPDATA%\Revisio`, ou le dossier indiqué par la variable d'environnement
/// `REVISIO_DOSSIER_DONNEES` (utilisée pour tester l'app sans toucher aux vraies données).
pub fn dossier_donnees_par_defaut() -> Result<PathBuf, String> {
    if let Some(dossier) = std::env::var_os("REVISIO_DOSSIER_DONNEES").filter(|d| !d.is_empty()) {
        return Ok(PathBuf::from(dossier));
    }
    let appdata = std::env::var_os("APPDATA").ok_or("La variable APPDATA est introuvable.")?;
    Ok(PathBuf::from(appdata).join(NOM_DOSSIER_APP))
}

/// Chemin absolu « propre », comparable même si le fichier (ou ses dossiers) n'existe pas encore :
/// on remonte jusqu'au premier dossier existant, on le normalise, puis on rajoute la fin du chemin.
fn normaliser(p: &Path) -> PathBuf {
    let mut existant = p;
    let mut reste: Vec<&std::ffi::OsStr> = Vec::new();
    loop {
        if let Ok(c) = std::fs::canonicalize(existant) {
            return reste.iter().rev().fold(c, |acc, nom| acc.join(nom));
        }
        match (existant.parent(), existant.file_name()) {
            (Some(parent), Some(nom)) => {
                reste.push(nom);
                existant = parent;
            }
            _ => return p.to_path_buf(),
        }
    }
}

/// Windows ne distingue pas majuscules et minuscules dans les chemins.
fn composants_minuscules(p: &Path) -> Vec<String> {
    p.components()
        .filter(|c| !matches!(c, Component::CurDir))
        .map(|c| c.as_os_str().to_string_lossy().to_lowercase())
        .collect()
}

/// Vrai si `chemin` est le vault lui-même ou se trouve à l'intérieur.
pub fn est_dans(chemin: &Path, vault: &Path) -> bool {
    let c = composants_minuscules(&normaliser(chemin));
    let v = composants_minuscules(&normaliser(vault));
    !v.is_empty() && c.len() >= v.len() && c[..v.len()] == v[..]
}

/// Refuse toute écriture dans le vault.
pub fn verifier_hors_vault(cible: &Path, vault: Option<&Path>) -> Result<(), String> {
    match vault {
        Some(v) if est_dans(cible, v) => Err(format!(
            "Écriture refusée : {} est dans le vault (le vault est en lecture seule).",
            cible.display()
        )),
        _ => Ok(()),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn detecte_un_chemin_dans_le_vault() {
        let vault = Path::new(r"C:\Coffre\MonVault");
        assert!(est_dans(Path::new(r"C:\Coffre\MonVault\Maths\note.md"), vault));
        assert!(est_dans(Path::new(r"c:\coffre\MONVAULT\x.md"), vault));
        assert!(est_dans(vault, vault));
        assert!(!est_dans(Path::new(r"C:\Coffre\MonVault-copie\x.md"), vault));
        assert!(!est_dans(Path::new(r"C:\Users\moi\AppData\Roaming\Revisio\app.db"), vault));
    }

    #[test]
    fn refuse_une_ecriture_dans_le_vault() {
        let vault = Path::new(r"C:\Coffre\MonVault");
        assert!(verifier_hors_vault(Path::new(r"C:\Coffre\MonVault\a.json"), Some(vault)).is_err());
        assert!(verifier_hors_vault(Path::new(r"C:\Autre\a.json"), Some(vault)).is_ok());
        assert!(verifier_hors_vault(Path::new(r"C:\Autre\a.json"), None).is_ok());
    }
}
