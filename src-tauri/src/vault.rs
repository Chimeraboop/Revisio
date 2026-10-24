//! SEUL module de Révisio autorisé à accéder au vault Obsidian.
//!
//! Règle absolue : LECTURE SEULE. Ce fichier ne doit contenir aucune fonction
//! qui modifie le disque. Le test `tests/lecture_seule.rs` relit ce fichier et
//! échoue s'il y trouve un appel de ce genre.

use std::fs::{File, OpenOptions};
use std::io::Read;
#[cfg(windows)]
use std::os::windows::fs::OpenOptionsExt;
use std::path::{Component, Path, PathBuf};
use std::time::{Duration, UNIX_EPOCH};

use notify_debouncer_full::notify::{Config, RecommendedWatcher, RecursiveMode};
use notify_debouncer_full::{new_debouncer_opt, DebounceEventResult, Debouncer, NoCache};
use walkdir::WalkDir;

use crate::paths::est_dans;

/// Dossiers jamais lus (comparés en minuscules). Tout dossier commençant par « . » est aussi ignoré.
const DOSSIERS_IGNORES: &[&str] = &["_templates", "node_modules"];

/// Taille maximale lue pour un fichier (évite de charger un fichier énorme par erreur).
const TAILLE_MAX: u64 = 50 * 1024 * 1024;

/// Partage Windows : on laisse Obsidian et OneDrive lire, modifier ou supprimer
/// le fichier pendant qu'on le lit (on ne le verrouille jamais).
#[cfg(windows)]
const PARTAGE_TOTAL: u32 = 0x1 | 0x2 | 0x4; // FILE_SHARE_READ | FILE_SHARE_WRITE | FILE_SHARE_DELETE

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct FichierVault {
    /// Chemin relatif au vault, avec des « / » (ex. `Spé SVT/Chapitre 3 - La méiose/Note.md`).
    pub chemin: String,
    pub taille: u64,
    /// Date de modification, en millisecondes depuis 1970.
    pub modifie: i64,
}

/// Vrai si ce nom de dossier doit être ignoré.
pub fn dossier_ignore(nom: &str) -> bool {
    nom.starts_with('.') || DOSSIERS_IGNORES.contains(&nom.to_lowercase().as_str())
}

/// Vrai si un chemin relatif (avec « / ») passe par un dossier ignoré.
pub fn chemin_ignore(chemin_relatif: &str) -> bool {
    let morceaux: Vec<&str> = chemin_relatif.split('/').collect();
    morceaux[..morceaux.len().saturating_sub(1)].iter().any(|m| dossier_ignore(m))
}

/// Liste tous les fichiers du vault (hors dossiers ignorés), sans les ouvrir.
pub fn lister_fichiers(racine: &Path) -> Result<Vec<FichierVault>, String> {
    if !racine.is_dir() {
        return Err(format!("Le vault est introuvable : {}", racine.display()));
    }
    let mut fichiers = Vec::new();
    let parcours = WalkDir::new(racine)
        .follow_links(false)
        .into_iter()
        .filter_entry(|e| e.depth() == 0 || !(e.file_type().is_dir() && dossier_ignore(&e.file_name().to_string_lossy())));
    for entree in parcours {
        let entree = match entree {
            Ok(e) => e,
            Err(_) => continue, // dossier illisible : on l'ignore sans planter
        };
        if !entree.file_type().is_file() {
            continue;
        }
        let Ok(relatif) = entree.path().strip_prefix(racine) else { continue };
        let Ok(meta) = entree.metadata() else { continue };
        let modifie = meta
            .modified()
            .ok()
            .and_then(|t| t.duration_since(UNIX_EPOCH).ok())
            .map(|d| d.as_millis() as i64)
            .unwrap_or(0);
        fichiers.push(FichierVault {
            chemin: vers_chemin_relatif(relatif),
            taille: meta.len(),
            modifie,
        });
    }
    fichiers.sort_by(|a, b| a.chemin.cmp(&b.chemin));
    Ok(fichiers)
}

/// Dossiers du vault (hors dossiers ignorés), en chemins relatifs triés : pour choisir où ranger une note.
pub fn lister_dossiers(racine: &Path) -> Vec<String> {
    let parcours = WalkDir::new(racine)
        .min_depth(1)
        .follow_links(false)
        .into_iter()
        .filter_entry(|e| !(e.file_type().is_dir() && dossier_ignore(&e.file_name().to_string_lossy())));
    let mut dossiers: Vec<String> = parcours
        .filter_map(Result::ok)
        .filter(|e| e.file_type().is_dir())
        .filter_map(|e| e.path().strip_prefix(racine).ok().map(vers_chemin_relatif))
        .collect();
    dossiers.sort_by_key(|d| d.to_lowercase());
    dossiers
}

/// Modèles de notes du vault (fichiers `.md` d'un dossier `_Templates`, ignoré par l'index),
/// en chemins relatifs. Sert d'exemple de mise en forme pour l'IA.
pub fn lister_modeles(racine: &Path) -> Vec<String> {
    let parcours = WalkDir::new(racine)
        .max_depth(5)
        .follow_links(false)
        .into_iter()
        .filter_entry(|e| e.depth() == 0 || !(e.file_type().is_dir() && e.file_name().to_string_lossy().starts_with('.')));
    let mut modeles: Vec<String> = parcours
        .filter_map(Result::ok)
        .filter(|e| e.file_type().is_file())
        .filter_map(|e| e.path().strip_prefix(racine).ok().map(vers_chemin_relatif))
        .filter(|c| {
            let morceaux: Vec<&str> = c.split('/').collect();
            c.to_lowercase().ends_with(".md")
                && morceaux[..morceaux.len() - 1].iter().any(|m| m.eq_ignore_ascii_case("_templates"))
        })
        .collect();
    modeles.sort();
    modeles
}

/// Lit une note en texte (UTF-8, BOM retiré).
pub fn lire_texte(racine: &Path, chemin_relatif: &str) -> Result<String, String> {
    let octets = lire_octets(racine, chemin_relatif)?;
    let texte = String::from_utf8_lossy(&octets).into_owned();
    Ok(texte.strip_prefix('\u{feff}').map(str::to_owned).unwrap_or(texte))
}

/// Lit un fichier du vault (image, note…) en octets.
pub fn lire_octets(racine: &Path, chemin_relatif: &str) -> Result<Vec<u8>, String> {
    let chemin = chemin_sur(racine, chemin_relatif)?;
    let mut fichier = ouvrir_en_lecture_seule(&chemin)?;
    let taille = fichier.metadata().map(|m| m.len()).unwrap_or(0);
    if taille > TAILLE_MAX {
        return Err(format!("Fichier trop volumineux : {chemin_relatif}"));
    }
    let mut octets = Vec::with_capacity(taille as usize);
    fichier
        .read_to_end(&mut octets)
        .map_err(|e| format!("Lecture impossible de {chemin_relatif} : {e}"))?;
    Ok(octets)
}

/// Ouverture en lecture seule : seul `read(true)` est activé.
/// (Sous macOS, un fichier ouvert ne bloque jamais les autres programmes.)
fn ouvrir_en_lecture_seule(chemin: &Path) -> Result<File, String> {
    let mut options = OpenOptions::new();
    options.read(true);
    #[cfg(windows)]
    options.share_mode(PARTAGE_TOTAL);
    options.open(chemin).map_err(|e| format!("Ouverture impossible de {} : {e}", chemin.display()))
}

/// Transforme un chemin relatif venant de l'interface en chemin absolu,
/// en refusant tout ce qui sortirait du vault (`..`, chemin absolu, lien…).
fn chemin_sur(racine: &Path, chemin_relatif: &str) -> Result<PathBuf, String> {
    let relatif = Path::new(chemin_relatif);
    if relatif.components().any(|c| !matches!(c, Component::Normal(_))) {
        return Err(format!("Chemin refusé : {chemin_relatif}"));
    }
    let complet = racine.join(relatif);
    if !est_dans(&complet, racine) {
        return Err(format!("Chemin hors du vault refusé : {chemin_relatif}"));
    }
    Ok(complet)
}

/// Surveillance du vault : tant que cet objet existe, Windows prévient l'app des changements.
pub type Surveillance = Debouncer<RecommendedWatcher, NoCache>;

/// Délai d'attente après le dernier changement (OneDrive et Obsidian écrivent par rafales).
const ANTI_REBOND: Duration = Duration::from_millis(1500);

/// Surveille le vault (simple écoute, aucune modification) et appelle `rappel`
/// quand une note ou une image change. Les changements dans `.obsidian` etc. sont ignorés.
pub fn surveiller(racine: &Path, rappel: impl Fn() + Send + 'static) -> Result<Surveillance, String> {
    let racine_ecoutee = racine.to_path_buf();
    // macOS signale les changements avec le chemin réel (ex. /private/var/… pour /var/…) :
    // on compare aussi avec la forme résolue du dossier.
    let racine_reelle = std::fs::canonicalize(racine).unwrap_or_else(|_| racine.to_path_buf());
    let gestionnaire = move |resultat: DebounceEventResult| {
        let Ok(evenements) = resultat else { return };
        let utile = evenements
            .iter()
            .flat_map(|e| e.paths.iter())
            .any(|p| concerne_les_notes(&racine_ecoutee, p) || concerne_les_notes(&racine_reelle, p));
        if utile {
            rappel();
        }
    };
    let mut surveillance =
        new_debouncer_opt::<_, RecommendedWatcher, NoCache>(ANTI_REBOND, None, gestionnaire, NoCache, Config::default())
            .map_err(|e| format!("Surveillance du vault impossible : {e}"))?;
    surveillance
        .watch(racine, RecursiveMode::Recursive)
        .map_err(|e| format!("Surveillance du vault impossible : {e}"))?;
    Ok(surveillance)
}

fn concerne_les_notes(racine: &Path, chemin: &Path) -> bool {
    match chemin.strip_prefix(racine) {
        Ok(relatif) => !vers_chemin_relatif(relatif).split('/').any(dossier_ignore),
        Err(_) => false,
    }
}

fn vers_chemin_relatif(p: &Path) -> String {
    p.components()
        .map(|c| c.as_os_str().to_string_lossy())
        .collect::<Vec<_>>()
        .join("/")
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn dossiers_ignores() {
        assert!(dossier_ignore(".obsidian"));
        assert!(dossier_ignore(".git"));
        assert!(dossier_ignore(".smart-env"));
        assert!(dossier_ignore("_Templates"));
        assert!(!dossier_ignore("_Révisions"));
        assert!(!dossier_ignore("Spé SVT"));
        assert!(chemin_ignore(".obsidian/plugins/x.md"));
        assert!(chemin_ignore("_Templates/Template Cours.md"));
        assert!(!chemin_ignore("Spé SVT/Chap/Note.md"));
    }

    #[test]
    fn changements_ignores_hors_notes() {
        let racine = std::env::temp_dir().join("vault");
        assert!(concerne_les_notes(&racine, &racine.join("Spé SVT").join("Note.md")));
        assert!(!concerne_les_notes(&racine, &racine.join(".obsidian").join("workspace.json")));
        assert!(!concerne_les_notes(&racine, &racine.join(".git").join("index")));
        assert!(!concerne_les_notes(&racine, &std::env::temp_dir().join("autre").join("Note.md")));
    }

    #[test]
    fn refuse_les_chemins_qui_sortent_du_vault() {
        let racine = std::env::temp_dir();
        assert!(chemin_sur(&racine, "../secret.txt").is_err());
        let absolu = if cfg!(windows) { r"C:\Windows\win.ini" } else { "/etc/hosts" };
        assert!(chemin_sur(&racine, absolu).is_err());
        assert!(chemin_sur(&racine, "dossier/note.md").is_ok());
    }
}
