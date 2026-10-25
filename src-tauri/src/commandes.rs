//! Commandes appelables depuis l'interface avec `invoke(...)`.
//! Chaque commande renvoie soit une valeur (convertie en JSON), soit un message
//! d'erreur en français affiché tel quel par l'interface.

use std::path::Path;

use serde::{Deserialize, Serialize};
use tauri::ipc::Response;
use tauri::{AppHandle, State};
use tauri_plugin_opener::OpenerExt;

use crate::ia::{self, gemini::ErreurIa};
use crate::settings::{self, Reglages};
use crate::{
    cartes_mentales, ecriture, photos, demarrer_surveillance, lancer_indexation, maintenant_ms, navigation, planning, podcasts, revision,
    search, sources_web, vault, videos, web, Etat, EtatIndexation,
};

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub struct InfosApp {
    version: String,
    dossier_donnees: String,
    /// Vaults qu'Obsidian connaît sur ce PC (proposés par le guide de démarrage).
    vaults_detectes: Vec<settings::VaultDetecte>,
}

#[tauri::command]
pub fn infos_app(etat: State<Etat>) -> InfosApp {
    InfosApp {
        version: env!("CARGO_PKG_VERSION").into(),
        dossier_donnees: etat.dossier.to_string_lossy().into(),
        vaults_detectes: settings::vaults_obsidian(),
    }
}

#[tauri::command]
pub fn lire_reglages(etat: State<Etat>) -> Reglages {
    etat.reglages.lock().unwrap().clone()
}

#[tauri::command]
pub fn definir_vault(app: AppHandle, etat: State<Etat>, chemin: String) -> Result<Reglages, String> {
    settings::valider_vault(Path::new(&chemin), &etat.dossier)?;
    let ancien = etat.reglages.lock().unwrap().chemin_vault.clone();
    let change = ancien.as_deref() != Some(chemin.as_str());
    let reglages = modifier_reglages(&etat, |r| {
        r.chemin_vault = Some(chemin.clone());
        if change {
            // Le nom utilisé par Obsidian est celui du dossier.
            r.nom_vault_obsidian = String::new();
        }
    })?;
    if change {
        // Autre vault : on repart d'un index vide.
        etat.db
            .lock()
            .unwrap()
            .execute_batch("DELETE FROM recherche; DELETE FROM notes; DELETE FROM fichiers;")
            .map_err(|e| e.to_string())?;
    }
    lancer_indexation(&app);
    demarrer_surveillance(&app);
    Ok(reglages)
}

/// Crée un nouveau dossier de notes (pour qui n'en a pas encore), avec une note d'accueil,
/// et le choisit. Révisio a le droit d'y écrire d'emblée : c'est le cahier de l'élève.
#[tauri::command]
pub fn creer_dossier_notes(app: AppHandle, etat: State<Etat>, parent: String, nom: String) -> Result<Reglages, String> {
    let nom = ecriture::nom_de_fichier(&nom);
    if nom.is_empty() {
        return Err("Donne un nom au dossier.".into());
    }
    let parent = Path::new(&parent);
    if !parent.is_dir() {
        return Err("L'emplacement choisi n'existe pas.".into());
    }
    let dossier = parent.join(&nom);
    if dossier.exists() {
        return Err(format!("Un dossier « {nom} » existe déjà à cet endroit : choisis-le, ou donne un autre nom."));
    }
    if crate::paths::est_dans(&dossier, &etat.dossier) {
        return Err("Choisis un emplacement en dehors des données de Révisio.".into());
    }
    std::fs::create_dir(&dossier).map_err(|e| format!("Création du dossier : {e}"))?;
    let ctx = ecriture::Contexte {
        racine: &dossier,
        autorisee: true,
        origine: "manuel",
        lot_id: None,
        raison: None,
        maintenant: maintenant_ms(),
        groupe: None,
    };
    ecriture::creer(&etat.db.lock().unwrap(), &ctx, "Accueil.md", settings::NOTE_ACCUEIL)?;
    modifier_reglages(&etat, |r| r.ecriture_autorisee = true)?;
    definir_vault(app, etat, dossier.to_string_lossy().into_owned())
}

/// Le dossier de notes est-il un vault Obsidian ? (sinon, pas de bouton « Ouvrir dans Obsidian »)
#[tauri::command]
pub fn est_vault_obsidian(etat: State<Etat>) -> bool {
    etat.reglages.lock().unwrap().vault().is_some_and(|v| settings::est_vault_obsidian(&v))
}

#[tauri::command]
pub fn rechercher(
    etat: State<Etat>,
    texte: String,
    matiere: Option<String>,
) -> Result<Vec<search::Resultat>, String> {
    search::rechercher(&etat.db.lock().unwrap(), &texte, matiere.as_deref(), 60).map_err(|e| e.to_string())
}

#[tauri::command]
pub fn etat_indexation(etat: State<Etat>) -> EtatIndexation {
    etat.indexation.lock().unwrap().clone()
}

#[tauri::command]
pub fn reindexer(app: AppHandle) {
    lancer_indexation(&app);
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub struct StatsMatiere {
    matiere: String,
    notes: i64,
    chapitres: i64,
    fiches_synthese: i64,
    schemas: i64,
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Statistiques {
    total_notes: i64,
    matieres: Vec<StatsMatiere>,
}

#[tauri::command]
pub fn statistiques(etat: State<Etat>) -> Result<Statistiques, String> {
    let conn = etat.db.lock().unwrap();
    let total_notes = conn
        .query_row("SELECT count(*) FROM notes", [], |r| r.get(0))
        .map_err(|e| e.to_string())?;
    let mut req = conn
        .prepare(
            "SELECT matiere, count(*), count(DISTINCT chapitre),
                    sum(genre = 'fiche_synthese'), sum(genre = 'schema')
             FROM notes WHERE matiere IS NOT NULL GROUP BY matiere ORDER BY matiere",
        )
        .map_err(|e| e.to_string())?;
    let matieres = req
        .query_map([], |r| {
            Ok(StatsMatiere {
                matiere: r.get(0)?,
                notes: r.get(1)?,
                chapitres: r.get(2)?,
                fiches_synthese: r.get(3)?,
                schemas: r.get(4)?,
            })
        })
        .map_err(|e| e.to_string())?
        .collect::<Result<Vec<_>, _>>()
        .map_err(|e| e.to_string())?;
    Ok(Statistiques { total_notes, matieres })
}

#[tauri::command]
pub fn definir_theme(etat: State<Etat>, theme: String) -> Result<Reglages, String> {
    if !["systeme", "clair", "sombre"].contains(&theme.as_str()) {
        return Err(format!("Thème inconnu : {theme}"));
    }
    modifier_reglages(&etat, |r| r.theme = theme)
}

/// Interrupteur « En ligne » : en mode local, Gemini n'est plus jamais appelé (rien ne part chez Google) ;
/// les tâches simples passent par l'IA locale.
#[tauri::command]
pub fn definir_mode_local(etat: State<Etat>, actif: bool) -> Result<Reglages, String> {
    let r = modifier_reglages(&etat, |r| r.mode_local = actif)?;
    etat.ia.couper_gemini(actif);
    Ok(r)
}

/// Guide de démarrage terminé (ou à revoir, depuis les Réglages).
#[tauri::command]
pub fn definir_accueil_termine(etat: State<Etat>, termine: bool) -> Result<Reglages, String> {
    modifier_reglages(&etat, |r| r.accueil_termine = Some(termine))
}

/// Autorise (ou non) Révisio à écrire dans le vault.
#[tauri::command]
pub fn definir_ecriture_autorisee(etat: State<Etat>, autorisee: bool) -> Result<Reglages, String> {
    modifier_reglages(&etat, |r| r.ecriture_autorisee = autorisee)
}

/// Mode Modifier d'une note : `avant` est le texte que l'élève avait sous les yeux ; si la note
/// a changé depuis (dans Obsidian), l'enregistrement est refusé. L'ancien contenu est gardé
/// dans l'historique (annulable).
#[tauri::command]
pub fn enregistrer_note(etat: State<Etat>, chemin: String, contenu: String, avant: String) -> Result<i64, String> {
    let reglages = etat.reglages.lock().unwrap().clone();
    let racine = reglages.vault().ok_or("Aucun vault choisi.")?;
    let ctx = ecriture::Contexte {
        racine: &racine,
        autorisee: reglages.ecriture_autorisee,
        origine: "manuel",
        lot_id: None,
        raison: None,
        maintenant: maintenant_ms(),
        groupe: None,
    };
    // L'éditeur travaille avec des retours à la ligne « \n » : on compare sans tenir compte des « \r\n »
    // de Windows, et on garde ceux de la note si elle en avait.
    let normaliser = |t: &str| t.replace("\r\n", "\n");
    let actuel = vault::lire_texte(&racine, &chemin)?;
    let crlf = actuel.contains("\r\n");
    let avant = normaliser(&avant);
    let pareille = |octets: &[u8]| {
        let texte = String::from_utf8_lossy(octets);
        normaliser(texte.strip_prefix('\u{feff}').unwrap_or(&texte)) == avant
    };
    let contenu = if crlf { normaliser(&contenu).replace('\n', "\r\n") } else { contenu };
    ecriture::modifier_en_seance(&etat.db.lock().unwrap(), &ctx, &chemin, &contenu, &pareille)
}

/// Image collée ou déposée dans une note (corps brut ; en-tête `x-note` = chemin de la note, encodé
/// pour une adresse web). Rangée comme Obsidian le ferait, renvoie son nom pour `![[nom]]`.
#[tauri::command]
pub fn ajouter_image(app: AppHandle, etat: State<Etat>, request: tauri::ipc::Request) -> Result<String, String> {
    let tauri::ipc::InvokeBody::Raw(octets) = request.body() else {
        return Err("Image attendue.".into());
    };
    if octets.len() > 20 * 1024 * 1024 {
        return Err("Image trop lourde (plus de 20 Mo).".into());
    }
    let note = request
        .headers()
        .get("x-note")
        .and_then(|v| v.to_str().ok())
        .map(decoder_entete)
        .ok_or("Note non précisée.")?;
    enregistrer_image(&app, &etat, &note, octets)
}

/// Image choisie sur le disque (bouton « Ajouter une image ») : copiée comme pièce jointe.
#[tauri::command]
pub fn importer_image(app: AppHandle, etat: State<Etat>, note: String, source: String) -> Result<String, String> {
    let taille = std::fs::metadata(&source).map_err(|e| format!("Image introuvable : {e}"))?.len();
    if taille > 20 * 1024 * 1024 {
        return Err("Image trop lourde (plus de 20 Mo).".into());
    }
    let octets = std::fs::read(&source).map_err(|e| format!("Lecture de l'image : {e}"))?;
    enregistrer_image(&app, &etat, &note, &octets)
}

fn enregistrer_image(app: &AppHandle, etat: &Etat, note: &str, octets: &[u8]) -> Result<String, String> {
    // Le format vient du contenu, pas du nom donné par le presse-papiers.
    let extension = if octets.starts_with(&[0x89, b'P', b'N', b'G']) {
        "png"
    } else if octets.starts_with(&[0xFF, 0xD8, 0xFF]) {
        "jpg"
    } else if octets.starts_with(b"GIF8") {
        "gif"
    } else if octets.len() > 12 && &octets[..4] == b"RIFF" && &octets[8..12] == b"WEBP" {
        "webp"
    } else {
        return Err("Format d'image non reconnu (PNG, JPEG, GIF ou WebP).".into());
    };
    let reglages = etat.reglages.lock().unwrap().clone();
    let racine = reglages.vault().ok_or("Aucun dossier de notes choisi.")?;
    let dossier = dossier_pieces_jointes(&racine, note);
    let horodatage = chrono::Local::now().format("%Y%m%d%H%M%S");
    let mut nom = String::new();
    let mut chemin = String::new();
    for n in 1..100 {
        nom = if n == 1 {
            format!("Image collée {horodatage}.{extension}")
        } else {
            format!("Image collée {horodatage} {n}.{extension}")
        };
        chemin = if dossier.is_empty() { nom.clone() } else { format!("{dossier}/{nom}") };
        if !racine.join(&chemin).exists() {
            break;
        }
    }
    ecriture::creer_piece_jointe(&etat.db.lock().unwrap(), &contexte_manuel(&racine, &reglages, None, None), &chemin, octets)?;
    lancer_indexation(app);
    Ok(nom)
}

/// En-tête HTTP encodé comme une adresse web (`%C3%A9` → « é »).
fn decoder_entete(texte: &str) -> String {
    let octets = texte.as_bytes();
    let mut sortie = Vec::with_capacity(octets.len());
    let mut i = 0;
    while i < octets.len() {
        if octets[i] == b'%' && i + 2 < octets.len() {
            if let Ok(v) = u8::from_str_radix(&texte[i + 1..i + 3], 16) {
                sortie.push(v);
                i += 3;
                continue;
            }
        }
        sortie.push(octets[i]);
        i += 1;
    }
    String::from_utf8_lossy(&sortie).into_owned()
}

/// Dossier des images jointes : celui réglé dans Obsidian (`attachmentFolderPath` de
/// `.obsidian/app.json` : racine par défaut, « ./ » = dossier de la note), sinon `_Assets`.
fn dossier_pieces_jointes(racine: &Path, note: &str) -> String {
    let dossier_note = note.rsplit_once('/').map(|(d, _)| d.to_string()).unwrap_or_default();
    if !settings::est_vault_obsidian(racine) {
        return "_Assets".into();
    }
    let reglage = vault::lire_texte(racine, ".obsidian/app.json")
        .ok()
        .and_then(|t| serde_json::from_str::<serde_json::Value>(&t).ok())
        .and_then(|v| v.get("attachmentFolderPath").and_then(|p| p.as_str()).map(str::to_string))
        .unwrap_or_default();
    let reglage = reglage.trim().trim_end_matches('/');
    let combiner = |a: &str, b: &str| match (a.is_empty(), b.is_empty()) {
        (true, _) => b.to_string(),
        (_, true) => a.to_string(),
        _ => format!("{a}/{b}"),
    };
    if reglage.is_empty() || reglage == "/" {
        String::new()
    } else if reglage == "." {
        dossier_note
    } else if let Some(sous) = reglage.strip_prefix("./") {
        combiner(&dossier_note, sous)
    } else {
        reglage.trim_start_matches('/').to_string()
    }
}

// ---------- Gestion des notes et dossiers (écriture autorisée seulement) ----------

static COMPTEUR_GROUPES: std::sync::atomic::AtomicU64 = std::sync::atomic::AtomicU64::new(0);

/// Identifiant d'un groupe d'opérations annulées ensemble.
fn nouveau_groupe() -> String {
    let n = COMPTEUR_GROUPES.fetch_add(1, std::sync::atomic::Ordering::SeqCst);
    format!("{}-{n}", maintenant_ms())
}

fn contexte_manuel<'a>(racine: &'a Path, reglages: &Reglages, groupe: Option<&'a str>, raison: Option<&'a str>) -> ecriture::Contexte<'a> {
    ecriture::Contexte {
        racine,
        autorisee: reglages.ecriture_autorisee,
        origine: "manuel",
        lot_id: None,
        raison,
        maintenant: maintenant_ms(),
        groupe,
    }
}

/// Dossiers du dossier de notes (pour choisir où ranger ou déplacer une note).
#[tauri::command]
pub fn dossiers_notes(etat: State<Etat>) -> Result<Vec<String>, String> {
    let racine = etat.reglages.lock().unwrap().vault().ok_or("Aucun dossier de notes choisi.")?;
    Ok(vault::lister_dossiers(&racine))
}

/// Crée un dossier (une matière, un chapitre…). `chemin` est relatif au dossier de notes.
#[tauri::command]
pub fn creer_dossier(app: AppHandle, etat: State<Etat>, chemin: String) -> Result<String, String> {
    let reglages = etat.reglages.lock().unwrap().clone();
    let racine = reglages.vault().ok_or("Aucun dossier de notes choisi.")?;
    let chemin = nettoyer_chemin(&chemin)?;
    ecriture::creer_dossier(&etat.db.lock().unwrap(), &contexte_manuel(&racine, &reglages, None, None), &chemin)?;
    lancer_indexation(&app);
    Ok(chemin)
}

/// Chemin relatif saisi dans l'interface : chaque morceau devient un nom de fichier valide.
fn nettoyer_chemin(chemin: &str) -> Result<String, String> {
    let morceaux: Vec<String> = chemin
        .split(['/', '\\'])
        .map(ecriture::nom_de_fichier)
        .filter(|m| !m.is_empty())
        .collect();
    if morceaux.is_empty() {
        return Err("Donne un nom.".into());
    }
    Ok(morceaux.join("/"))
}

/// Crée une note dans `dossier` (vide = à la racine) à partir d'un modèle :
/// « vide », « cours », « synthese » (fiche de synthèse) ou « vocabulaire ». Renvoie son chemin.
#[tauri::command]
pub fn creer_note(app: AppHandle, etat: State<Etat>, dossier: String, titre: String, modele: String) -> Result<String, String> {
    let reglages = etat.reglages.lock().unwrap().clone();
    let racine = reglages.vault().ok_or("Aucun dossier de notes choisi.")?;
    let titre = ecriture::nom_de_fichier(&titre);
    if titre.is_empty() {
        return Err("Donne un titre à la note.".into());
    }
    let nom = match modele.as_str() {
        "synthese" if !titre.starts_with("Fiche de synthèse") => format!("Fiche de synthèse - {titre}"),
        "vocabulaire" if !titre.starts_with("Vocabulaire") => format!("Vocabulaire - {titre}"),
        _ => titre,
    };
    let date = chrono::Local::now().format("%Y-%m-%dT%H:%M");
    let corps = match modele.as_str() {
        "cours" => "> [!info]+ Définition\n> \n\n> [!tip]+ Notes liées\n> \n",
        "synthese" => "## L'essentiel\n\n## Notions du chapitre\n- \n",
        "vocabulaire" => "%% Une ligne par mot, sous la forme : * mot = traduction %%\n\n",
        _ => "",
    };
    let contenu = format!("---\ncreated: {date}\n---\n\n{corps}");
    let dossier = if dossier.trim().is_empty() { String::new() } else { nettoyer_chemin(&dossier)? };
    // Nom déjà pris : « Note 2 », « Note 3 »…
    let mut chemin = String::new();
    for n in 1..100 {
        let fichier = if n == 1 { format!("{nom}.md") } else { format!("{nom} {n}.md") };
        chemin = if dossier.is_empty() { fichier } else { format!("{dossier}/{fichier}") };
        if !racine.join(&chemin).exists() {
            break;
        }
    }
    ecriture::creer(&etat.db.lock().unwrap(), &contexte_manuel(&racine, &reglages, None, None), &chemin, &contenu)?;
    lancer_indexation(&app);
    Ok(chemin)
}

/// Renomme une note, une image ou un dossier (même dossier parent). Les liens [[…]] vers une note
/// renommée sont mis à jour dans les autres notes ; la progression de révision suit.
/// Renvoie le nouveau chemin.
#[tauri::command]
pub fn renommer_element(app: AppHandle, etat: State<Etat>, chemin: String, nom: String) -> Result<String, String> {
    let reglages = etat.reglages.lock().unwrap().clone();
    let racine = reglages.vault().ok_or("Aucun dossier de notes choisi.")?;
    let est_dossier = racine.join(&chemin).is_dir();
    let nom = nom.trim();
    let est_note = !est_dossier && chemin.to_lowercase().ends_with(".md");
    let nom = if est_note { nom.strip_suffix(".md").unwrap_or(nom) } else { nom };
    let mut nom = ecriture::nom_de_fichier(nom);
    if nom.is_empty() {
        return Err("Donne un nom.".into());
    }
    if est_note {
        nom.push_str(".md");
    } else if !est_dossier {
        // Image ou autre fichier : on garde l'extension d'origine.
        if let Some(ext) = Path::new(&chemin).extension() {
            let ext = ext.to_string_lossy();
            if !nom.to_lowercase().ends_with(&format!(".{}", ext.to_lowercase())) {
                nom = format!("{nom}.{ext}");
            }
        }
    }
    let nouveau = match chemin.rsplit_once('/') {
        Some((parent, _)) => format!("{parent}/{nom}"),
        None => nom.clone(),
    };
    if nouveau == chemin {
        return Ok(chemin);
    }
    let groupe = nouveau_groupe();
    let conn = etat.db.lock().unwrap();
    ecriture::deplacer(&conn, &contexte_manuel(&racine, &reglages, Some(&groupe), None), &chemin, &nouveau)?;
    crate::renommage::migrer_chemins(&conn, &chemin, &nouveau).map_err(|e| e.to_string())?;
    if est_note {
        let ancien_nom = chemin.rsplit('/').next().unwrap_or(&chemin).trim_end_matches(".md").to_string();
        let nouveau_nom = nom.trim_end_matches(".md").to_string();
        if ancien_nom.to_lowercase() != nouveau_nom.to_lowercase() {
            let raison = format!("Lien vers « {nouveau_nom} » mis à jour");
            let ctx = contexte_manuel(&racine, &reglages, Some(&groupe), Some(&raison));
            for source in crate::renommage::notes_qui_lient(&conn, &ancien_nom).map_err(|e| e.to_string())? {
                let source = crate::renommage::chemin_deplace(&source, &chemin, &nouveau).unwrap_or(source);
                let Ok(octets) = vault::lire_octets(&racine, &source) else { continue };
                let texte = String::from_utf8_lossy(&octets).into_owned();
                if let Some(modifie) = crate::renommage::remplacer_liens(&texte, &ancien_nom, &nouveau_nom) {
                    // Une note qu'on ne peut pas modifier (dessin Excalidraw…) garde son ancien lien.
                    let _ = ecriture::modifier(&conn, &ctx, &source, &modifie, &|a| a == octets.as_slice());
                }
            }
        }
    }
    drop(conn);
    // Matière renommée : elle reste exclue des séances du jour si elle l'était.
    if !chemin.contains('/') && est_dossier && reglages.matieres_exclues.contains(&chemin) {
        modifier_reglages(&etat, |r| {
            for m in r.matieres_exclues.iter_mut().filter(|m| **m == chemin) {
                *m = nouveau.clone();
            }
        })?;
    }
    lancer_indexation(&app);
    Ok(nouveau)
}

/// Déplace une note, une image ou un dossier dans un autre dossier (vide = racine). Renvoie le nouveau chemin.
#[tauri::command]
pub fn deplacer_element(app: AppHandle, etat: State<Etat>, chemin: String, dossier: String) -> Result<String, String> {
    let reglages = etat.reglages.lock().unwrap().clone();
    let racine = reglages.vault().ok_or("Aucun dossier de notes choisi.")?;
    let nom = chemin.rsplit('/').next().unwrap_or(&chemin).to_string();
    let nouveau = if dossier.trim().is_empty() { nom } else { format!("{}/{nom}", dossier.trim_matches('/')) };
    if nouveau == chemin {
        return Ok(chemin);
    }
    let groupe = nouveau_groupe();
    let conn = etat.db.lock().unwrap();
    ecriture::deplacer(&conn, &contexte_manuel(&racine, &reglages, Some(&groupe), None), &chemin, &nouveau)?;
    crate::renommage::migrer_chemins(&conn, &chemin, &nouveau).map_err(|e| e.to_string())?;
    drop(conn);
    lancer_indexation(&app);
    Ok(nouveau)
}

/// Met une note, une image ou un dossier à la corbeille de Révisio (restaurable).
#[tauri::command]
pub fn supprimer_element(app: AppHandle, etat: State<Etat>, chemin: String) -> Result<String, String> {
    let reglages = etat.reglages.lock().unwrap().clone();
    let racine = reglages.vault().ok_or("Aucun dossier de notes choisi.")?;
    let groupe = nouveau_groupe();
    let n = ecriture::supprimer(&etat.db.lock().unwrap(), &contexte_manuel(&racine, &reglages, Some(&groupe), None), &chemin)?;
    lancer_indexation(&app);
    Ok(if n == 1 {
        "Mis à la corbeille de Révisio.".into()
    } else {
        format!("{n} fichiers mis à la corbeille de Révisio.")
    })
}

#[tauri::command]
pub fn corbeille(etat: State<Etat>) -> Result<Vec<ecriture::ElementCorbeille>, String> {
    ecriture::corbeille(&etat.db.lock().unwrap()).map_err(|e| e.to_string())
}

/// Restaure un élément de la corbeille.
#[tauri::command]
pub fn restaurer(app: AppHandle, etat: State<Etat>, groupe: String) -> Result<String, String> {
    let racine = etat.reglages.lock().unwrap().vault().ok_or("Aucun dossier de notes choisi.")?;
    let conn = etat.db.lock().unwrap();
    let a = ecriture::annuler_groupe(&conn, &racine, &groupe, maintenant_ms())?;
    appliquer_annulation(&conn, &a)?;
    drop(conn);
    lancer_indexation(&app);
    Ok(a.message)
}

/// Après une annulation : la base suit les fichiers remis à leur place.
fn appliquer_annulation(conn: &rusqlite::Connection, a: &ecriture::Annulation) -> Result<(), String> {
    for (de, vers) in &a.deplacements {
        crate::renommage::migrer_chemins(conn, de, vers).map_err(|e| e.to_string())?;
    }
    Ok(())
}

#[tauri::command]
pub fn historique_vault(etat: State<Etat>) -> Result<Vec<ecriture::EntreeHistorique>, String> {
    ecriture::historique(&etat.db.lock().unwrap(), 200).map_err(|e| e.to_string())
}

#[tauri::command]
pub fn annuler_ecriture(app: AppHandle, etat: State<Etat>, id: i64) -> Result<String, String> {
    let racine = etat.reglages.lock().unwrap().vault().ok_or("Aucun vault choisi.")?;
    let conn = etat.db.lock().unwrap();
    let a = ecriture::annuler(&conn, &racine, id, maintenant_ms())?;
    appliquer_annulation(&conn, &a)?;
    drop(conn);
    lancer_indexation(&app);
    Ok(a.message)
}

/// Photos choisies par l'élève : un lot (un cours), traité aussitôt. Interruptible.
#[tauri::command]
pub async fn importer_photos(
    etat: State<'_, Etat>,
    chemins: Vec<String>,
    tache: Option<String>,
) -> Result<photos::LotPhotos, String> {
    use ia::photos::{est_une_photo, PHOTOS_PAR_LOT};
    if chemins.is_empty() {
        return Err("Choisis au moins une photo.".into());
    }
    if chemins.len() > PHOTOS_PAR_LOT {
        return Err(format!("{PHOTOS_PAR_LOT} photos au plus à la fois (celles d'un même cours)."));
    }
    if let Some(c) = chemins.iter().find(|c| !Path::new(c).is_file() || !est_une_photo(Path::new(c))) {
        return Err(format!("Ce fichier n'est pas une photo (JPEG, PNG, WebP ou HEIC) : {c}"));
    }
    if !etat.reglages.lock().unwrap().ecriture_autorisee {
        return Err(ecriture::DESACTIVEE.into());
    }
    let id = photos::creer_lot(&etat.db.lock().unwrap(), "import", &chemins, maintenant_ms()).map_err(|e| e.to_string())?;
    traiter_lot_annulable(&etat, id, tache).await
}

/// Relance un lot en erreur (quota, coupure d'Internet, app fermée…).
#[tauri::command]
pub async fn relancer_lot_photos(etat: State<'_, Etat>, id: i64, tache: Option<String>) -> Result<photos::LotPhotos, String> {
    let lot = photos::lot(&etat.db.lock().unwrap(), id).map_err(|e| e.to_string())?.ok_or("Lot introuvable.")?;
    if lot.statut != "erreur" && lot.statut != "illisible" {
        return Err("Ce lot n'est pas à relancer.".into());
    }
    traiter_lot_annulable(&etat, id, tache).await
}

async fn traiter_lot_annulable(etat: &Etat, id: i64, tache: Option<String>) -> Result<photos::LotPhotos, String> {
    let resultat = etat.taches.annulable(tache, photos::traiter_lot(etat, id)).await;
    if let Err(e) = &resultat {
        // Interrompu (bouton Arrêter) : le lot ne doit pas rester « en cours ».
        let _ = etat.db.lock().unwrap().execute(
            "UPDATE lots_photos SET statut = 'erreur', message = ?1 WHERE id = ?2 AND statut IN ('attente', 'en_cours')",
            rusqlite::params![e, id],
        );
    }
    resultat
}

#[tauri::command]
pub fn lots_photos(etat: State<Etat>) -> Result<Vec<photos::LotPhotos>, String> {
    photos::lots(&etat.db.lock().unwrap(), 30).map_err(|e| e.to_string())
}

#[tauri::command]
pub fn propositions_vault(etat: State<Etat>) -> Result<Vec<photos::Proposition>, String> {
    photos::propositions(&etat.db.lock().unwrap()).map_err(|e| e.to_string())
}

#[tauri::command]
pub fn accepter_proposition(etat: State<Etat>, id: i64) -> Result<String, String> {
    photos::accepter(&etat, id)
}

#[tauri::command]
pub fn refuser_proposition(etat: State<Etat>, id: i64) -> Result<(), String> {
    photos::refuser(&etat.db.lock().unwrap(), id)
}

/// Dossier surveillé pour les photos de cours (`None` : arrêter la surveillance).
/// Les photos déjà présentes sont ignorées, sauf si `traiter_existantes`.
#[tauri::command]
pub fn definir_dossier_photos(
    etat: State<Etat>,
    chemin: Option<String>,
    traiter_existantes: bool,
) -> Result<Reglages, String> {
    if let Some(c) = &chemin {
        let dossier = Path::new(c);
        if !dossier.is_dir() {
            return Err("Ce dossier n'existe pas.".into());
        }
        if let Some(v) = etat.reglages.lock().unwrap().vault() {
            if crate::paths::est_dans(dossier, &v) || crate::paths::est_dans(&v, dossier) {
                return Err("Le dossier des photos doit être en dehors du vault.".into());
            }
        }
        if !traiter_existantes {
            photos::ignorer_photos_presentes(&etat.db.lock().unwrap(), dossier).map_err(|e| e.to_string())?;
        }
    }
    *etat.photos.lock().unwrap() = Default::default();
    modifier_reglages(&etat, |r| r.dossier_photos = chemin)
}

/// Nombre de photos d'un dossier que Révisio n'a encore jamais vues.
#[tauri::command]
pub fn compter_photos_dossier(etat: State<Etat>, chemin: String) -> usize {
    photos::nombre_nouvelles(&etat.db.lock().unwrap(), Path::new(&chemin))
}

#[tauri::command]
pub fn etat_photos(etat: State<Etat>) -> photos::EtatSurveillance {
    photos::etat_surveillance(&etat)
}

/// Dossiers « Mon Drive » de Google Drive pour ordinateur trouvés sur ce PC : un lecteur
/// (G:, H:…) sous Windows, `~/Library/CloudStorage/GoogleDrive-…` sous macOS.
#[tauri::command]
pub fn dossiers_drive() -> Vec<String> {
    let mut racines: Vec<std::path::PathBuf> = Vec::new();
    #[cfg(windows)]
    for lettre in 'D'..='Z' {
        racines.push(format!("{lettre}:\\").into());
    }
    #[cfg(target_os = "macos")]
    if let Some(maison) = std::env::var_os("HOME") {
        let stockage = Path::new(&maison).join("Library").join("CloudStorage");
        if let Ok(entrees) = std::fs::read_dir(&stockage) {
            racines.extend(
                entrees
                    .filter_map(Result::ok)
                    .filter(|e| e.file_name().to_string_lossy().starts_with("GoogleDrive"))
                    .map(|e| e.path()),
            );
        }
    }
    let mut trouves = Vec::new();
    for racine in racines {
        for nom in ["Mon Drive", "My Drive"] {
            let p = racine.join(nom);
            if p.is_dir() {
                trouves.push(p.to_string_lossy().into_owned());
            }
        }
    }
    trouves
}

/// Couleur d'accent de l'interface (monochrome par défaut).
#[tauri::command]
pub fn definir_accent(etat: State<Etat>, accent: String) -> Result<Reglages, String> {
    if !settings::ACCENTS.contains(&accent.as_str()) {
        return Err(format!("Couleur inconnue : {accent}"));
    }
    modifier_reglages(&etat, |r| r.accent = accent)
}

#[tauri::command]
pub fn arbre(etat: State<Etat>) -> Result<navigation::Arbre, String> {
    navigation::arbre(&etat.db.lock().unwrap()).map_err(|e| e.to_string())
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Note {
    #[serde(flatten)]
    fiche: navigation::FicheNote,
    /// Markdown lu dans le vault à l'instant (lecture seule).
    contenu: String,
    /// Lien `obsidian://`, seulement si le dossier de notes est un vault Obsidian.
    url_obsidian: Option<String>,
}

#[tauri::command]
pub fn note(etat: State<Etat>, chemin: String) -> Result<Note, String> {
    let fiche = navigation::fiche(&etat.db.lock().unwrap(), &chemin)
        .map_err(|e| e.to_string())?
        .ok_or_else(|| format!("Note introuvable dans l'index : {chemin}"))?;
    let reglages = etat.reglages.lock().unwrap().clone();
    let vault = reglages.vault().ok_or("Aucun vault choisi.")?;
    let contenu = vault::lire_texte(&vault, &chemin)?;
    Ok(Note {
        url_obsidian: settings::est_vault_obsidian(&vault)
            .then(|| navigation::url_obsidian(&reglages.nom_obsidian(), &chemin)),
        fiche,
        contenu,
    })
}

#[tauri::command]
pub fn resoudre_lien(
    etat: State<Etat>,
    cible: String,
    source: Option<String>,
) -> Result<Option<navigation::Cible>, String> {
    navigation::resoudre(&etat.db.lock().unwrap(), &cible, source.as_deref()).map_err(|e| e.to_string())
}

/// Octets d'un fichier du vault (images), envoyés tels quels à l'interface.
#[tauri::command]
pub fn lire_fichier_vault(etat: State<Etat>, chemin: String) -> Result<Response, String> {
    let vault = etat.reglages.lock().unwrap().vault().ok_or("Aucun vault choisi.")?;
    Ok(Response::new(vault::lire_octets(&vault, &chemin)?))
}

#[tauri::command]
pub fn ouvrir_dans_obsidian(app: AppHandle, etat: State<Etat>, chemin: String) -> Result<(), String> {
    let nom_vault = etat.reglages.lock().unwrap().nom_obsidian();
    app.opener()
        .open_url(navigation::url_obsidian(&nom_vault, &chemin), None::<&str>)
        .map_err(|e| format!("Impossible d'ouvrir Obsidian : {e}"))
}

/// Ouvre un lien web dans le navigateur par défaut (jamais dans la fenêtre de l'app).
#[tauri::command]
pub fn ouvrir_lien_externe(app: AppHandle, url: String) -> Result<(), String> {
    if !(url.starts_with("https://") || url.starts_with("http://")) {
        return Err("Seuls les liens web (http/https) peuvent être ouverts.".into());
    }
    app.opener()
        .open_url(url, None::<&str>)
        .map_err(|e| format!("Impossible d'ouvrir le lien : {e}"))
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ReglagesRevision {
    nouvelles_par_jour: usize,
    entrelacement: bool,
    vocab_vers_francais: String,
    vocab_depuis_francais: String,
    duree_seance: u32,
    nouvelles_par_jour_langues: usize,
    duree_seance_langues: u32,
}

#[tauri::command]
pub fn definir_reglages_revision(etat: State<Etat>, valeurs: ReglagesRevision) -> Result<Reglages, String> {
    let modes = ["retourner", "taper"];
    if !modes.contains(&valeurs.vocab_vers_francais.as_str()) || !modes.contains(&valeurs.vocab_depuis_francais.as_str()) {
        return Err("Mode de réponse inconnu.".into());
    }
    for n in [valeurs.nouvelles_par_jour, valeurs.nouvelles_par_jour_langues] {
        if !(0..=200).contains(&n) {
            return Err("Le nombre de nouvelles cartes par jour doit être entre 0 et 200.".into());
        }
    }
    for d in [valeurs.duree_seance, valeurs.duree_seance_langues] {
        if !(5..=180).contains(&d) {
            return Err("La durée d'une séance du jour doit être entre 5 et 180 minutes.".into());
        }
    }
    modifier_reglages(&etat, |r| {
        r.nouvelles_par_jour = valeurs.nouvelles_par_jour;
        r.entrelacement = valeurs.entrelacement;
        r.vocab_vers_francais = valeurs.vocab_vers_francais;
        r.vocab_depuis_francais = valeurs.vocab_depuis_francais;
        r.duree_seance = valeurs.duree_seance;
        r.nouvelles_par_jour_langues = valeurs.nouvelles_par_jour_langues;
        r.duree_seance_langues = valeurs.duree_seance_langues;
    })
}

fn quotas(etat: &Etat) -> revision::Quotas {
    let r = etat.reglages.lock().unwrap();
    revision::Quotas { langues: r.nouvelles_par_jour_langues, autres: r.nouvelles_par_jour }
}

#[tauri::command]
pub fn resume_revision(etat: State<Etat>, debut_jour: i64) -> Result<revision::Resume, String> {
    let q = quotas(&etat);
    let exclues = etat.reglages.lock().unwrap().matieres_exclues.clone();
    revision::resume(&etat.db.lock().unwrap(), maintenant_ms(), debut_jour, q, &exclues).map_err(|e| e.to_string())
}

#[tauri::command]
pub fn preparer_seance(etat: State<Etat>, mut filtre: revision::FiltreSeance) -> Result<revision::Seance, String> {
    let q = quotas(&etat);
    // Séances du jour : les matières exclues dans les réglages sont laissées de côté (pas en séance ciblée).
    if filtre.groupe.is_some() {
        filtre.exclues = etat.reglages.lock().unwrap().matieres_exclues.clone();
    }
    revision::preparer_seance(&etat.db.lock().unwrap(), &filtre, maintenant_ms(), q).map_err(|e| e.to_string())
}

/// Matières laissées de côté dans les séances du jour.
#[tauri::command]
pub fn definir_matieres_exclues(etat: State<Etat>, matieres: Vec<String>) -> Result<Reglages, String> {
    let mut matieres: Vec<String> =
        matieres.into_iter().map(|m| m.trim().to_string()).filter(|m| !m.is_empty()).collect();
    matieres.sort();
    matieres.dedup();
    modifier_reglages(&etat, |r| r.matieres_exclues = matieres)
}

#[tauri::command]
pub fn enregistrer_revision(etat: State<Etat>, revision: revision::NouvelleRevision) -> Result<(), String> {
    revision::enregistrer(&mut etat.db.lock().unwrap(), &revision, maintenant_ms())
}

#[tauri::command]
pub fn suspendre_carte(etat: State<Etat>, carte_id: i64) -> Result<(), String> {
    match revision::suspendre(&etat.db.lock().unwrap(), carte_id) {
        Ok(true) => Ok(()),
        Ok(false) => Err("Carte introuvable.".into()),
        Err(e) => Err(e.to_string()),
    }
}

#[tauri::command]
pub fn carnet_erreurs(
    etat: State<Etat>,
    matiere: Option<String>,
    avec_comprises: bool,
) -> Result<Vec<revision::Erreur>, String> {
    revision::erreurs(&etat.db.lock().unwrap(), matiere.as_deref(), avec_comprises).map_err(|e| e.to_string())
}

#[tauri::command]
pub fn marquer_erreur_comprise(etat: State<Etat>, erreur_id: i64, comprise: bool) -> Result<(), String> {
    revision::marquer_comprise(&etat.db.lock().unwrap(), erreur_id, comprise)
        .map(|_| ())
        .map_err(|e| e.to_string())
}

#[tauri::command]
pub fn cartes_choisies(etat: State<Etat>, ids: Vec<i64>) -> Result<Vec<revision::CarteSeance>, String> {
    revision::cartes_choisies(&etat.db.lock().unwrap(), &ids).map_err(|e| e.to_string())
}

/// Toutes les cartes jamais révisées sont considérées comme déjà apprises.
#[tauri::command]
pub fn marquer_cartes_apprises(etat: State<Etat>) -> Result<usize, String> {
    revision::marquer_apprises(&mut etat.db.lock().unwrap(), maintenant_ms()).map_err(|e| e.to_string())
}

#[tauri::command]
pub fn tableau_de_bord(etat: State<Etat>) -> Result<revision::TableauDeBord, String> {
    revision::tableau_de_bord(&etat.db.lock().unwrap()).map_err(|e| e.to_string())
}

// ---------- Dates d'évaluation et planning ----------

#[tauri::command]
pub fn enregistrer_evaluation(etat: State<Etat>, evaluation: planning::Evaluation) -> Result<i64, String> {
    planning::enregistrer(&etat.db.lock().unwrap(), &evaluation, maintenant_ms())
}

/// Évaluations à venir (et celles du jour `depuis`, AAAA-MM-JJ local).
#[tauri::command]
pub fn evaluations(etat: State<Etat>, depuis: String) -> Result<Vec<planning::Evaluation>, String> {
    planning::lister(&etat.db.lock().unwrap(), &depuis).map_err(|e| e.to_string())
}

#[tauri::command]
pub fn supprimer_evaluation(etat: State<Etat>, id: i64) -> Result<(), String> {
    planning::supprimer(&etat.db.lock().unwrap(), id).map(|_| ()).map_err(|e| e.to_string())
}

#[tauri::command]
pub fn revues_du_jour(etat: State<Etat>, debut_jour: i64) -> Result<Vec<planning::RevuesChapitre>, String> {
    planning::revues_depuis(&etat.db.lock().unwrap(), debut_jour).map_err(|e| e.to_string())
}

// ---------- IA (Gemini) ----------

#[tauri::command]
pub fn etat_ia(etat: State<Etat>) -> Result<ia::EtatIa, String> {
    let presente = etat.ia.cle_presente();
    ia::etat(&etat.db.lock().unwrap(), presente).map_err(|e| e.to_string())
}

/// La clé est vérifiée auprès de Google si possible, puis rangée dans le Gestionnaire
/// d'identification Windows. Elle ne revient jamais vers l'interface.
#[tauri::command]
pub async fn enregistrer_cle_ia(etat: State<'_, Etat>, cle: String) -> Result<String, String> {
    match etat.ia.verifier_cle(&cle).await {
        Ok(()) => {
            etat.ia.enregistrer_cle(&cle)?;
            Ok(format!("Clé vérifiée et enregistrée dans le {}.", crate::secret::NOM_COFFRE))
        }
        Err(ErreurIa::HorsLigne) => {
            etat.ia.enregistrer_cle(&cle)?;
            Ok("Clé enregistrée, mais pas encore vérifiée (pas de connexion Internet).".into())
        }
        Err(e) => Err(e.to_string()),
    }
}

#[tauri::command]
pub fn supprimer_cle_ia(etat: State<Etat>) -> Result<(), String> {
    etat.ia.supprimer_cle()
}

#[tauri::command]
pub async fn lister_modeles_ia(etat: State<'_, Etat>) -> Result<Vec<ia::gemini::ModeleInfo>, String> {
    Ok(etat.ia.lister_modeles().await?)
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ReglagesIa {
    modele_ia: String,
    modele_ia_rapide: String,
    ia_par_minute: u32,
    ia_par_jour: u32,
    modele_tts: String,
    voix_a: String,
    voix_b: String,
    voix_video: String,
}

#[tauri::command]
pub fn definir_reglages_ia(etat: State<Etat>, valeurs: ReglagesIa) -> Result<Reglages, String> {
    let nom_valide = |m: &str| !m.is_empty() && m.chars().all(|c| c.is_ascii_alphanumeric() || "-._".contains(c));
    if ![&valeurs.modele_ia, &valeurs.modele_ia_rapide, &valeurs.modele_tts].iter().all(|m| nom_valide(m)) {
        return Err("Nom de modèle invalide.".into());
    }
    let voix_valide = |v: &String| !v.is_empty() && v.chars().all(|c| c.is_ascii_alphanumeric());
    if ![&valeurs.voix_a, &valeurs.voix_b, &valeurs.voix_video].into_iter().all(voix_valide) {
        return Err("Nom de voix invalide (ex. Kore, Puck).".into());
    }
    if !(1..=60).contains(&valeurs.ia_par_minute) || !(1..=5000).contains(&valeurs.ia_par_jour) {
        return Err("Limites invalides (1 à 60 par minute, 1 à 5000 par jour).".into());
    }
    modifier_reglages(&etat, |r| {
        r.modele_ia = valeurs.modele_ia;
        r.modele_ia_rapide = valeurs.modele_ia_rapide;
        r.ia_par_minute = valeurs.ia_par_minute;
        r.ia_par_jour = valeurs.ia_par_jour;
        r.modele_tts = valeurs.modele_tts;
        r.voix_a = valeurs.voix_a;
        r.voix_b = valeurs.voix_b;
        r.voix_video = valeurs.voix_video;
    })
}

/// Petit essai réel : le modèle rapide doit répondre « OK ».
#[tauri::command]
pub async fn tester_ia(etat: State<'_, Etat>) -> Result<String, String> {
    let modele = etat.reglages.lock().unwrap().modele_ia_rapide.clone();
    let requete = ia::gemini::Requete {
        modele: modele.clone(),
        parties: vec![ia::gemini::Partie::Texte("Réponds uniquement par le mot : OK".into())],
        ..Default::default()
    };
    let debut = std::time::Instant::now();
    let reponse = etat.ia.appeler(&etat.db, "test", etat.limites_ia(), &requete).await?;
    Ok(format!(
        "Gemini répond ({modele}, {:.1} s) : « {} »",
        debut.elapsed().as_secs_f32(),
        reponse.texte.trim()
    ))
}

/// Internet est-il vraiment joignable ? (Windows se dit souvent « connecté » sans accès réel,
/// par exemple avec des cartes réseau virtuelles.) Un essai rapide auprès du serveur de Gemini.
#[tauri::command]
pub async fn verifier_internet(etat: State<'_, Etat>) -> Result<bool, String> {
    Ok(etat.ia.internet_joignable().await)
}

/// Interrompt une génération par l'IA lancée avec cet identifiant.
#[tauri::command]
pub fn annuler_tache(etat: State<Etat>, id: String) -> bool {
    etat.taches.annuler(&id)
}

// ---------- IA locale (Ollama) ----------

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub struct EtatIaLocale {
    adresse: String,
    /// Version d'Ollama s'il répond.
    version: Option<String>,
    modeles: Vec<ia::ollama::ModeleLocal>,
    /// Pourquoi Ollama ne répond pas.
    erreur: Option<String>,
    /// Téléchargement de modèle en cours (dans cette session de Révisio).
    telechargement: Option<String>,
}

#[tauri::command]
pub async fn etat_ia_locale(etat: State<'_, Etat>) -> Result<EtatIaLocale, String> {
    let adresse = etat.reglages.lock().unwrap().adresse_ollama.clone();
    let telechargement = etat.ia.telechargement.lock().unwrap().clone();
    let (version, modeles, erreur) = match etat.ia.local.version(&adresse).await {
        Ok(v) => match etat.ia.local.modeles(&adresse).await {
            Ok(m) => (Some(v), m, None),
            Err(e) => (Some(v), Vec::new(), Some(e.to_string())),
        },
        Err(e) => (None, Vec::new(), Some(e.to_string())),
    };
    Ok(EtatIaLocale { adresse, version, modeles, erreur, telechargement })
}

#[tauri::command]
pub fn definir_reglages_locaux(
    etat: State<Etat>,
    ia_locale: String,
    modele_local: String,
    adresse_ollama: String,
) -> Result<Reglages, String> {
    if !["jamais", "secours", "toujours"].contains(&ia_locale.as_str()) {
        return Err("Mode d'IA locale inconnu.".into());
    }
    let modele_local = modele_local.trim().to_string();
    if !modele_local.is_empty() && !ia::ollama::nom_modele_valide(&modele_local) {
        return Err("Nom de modèle invalide (ex. qwen3.5:4b).".into());
    }
    let adresse = ia::ollama::adresse_valide(&adresse_ollama)?;
    modifier_reglages(&etat, |r| {
        r.ia_locale = ia_locale;
        r.modele_local = modele_local;
        r.adresse_ollama = adresse;
    })
}

/// Petit essai réel de l'IA locale (le premier appel charge le modèle en mémoire : quelques dizaines de secondes).
#[tauri::command]
pub async fn tester_ia_locale(etat: State<'_, Etat>) -> Result<String, String> {
    let (adresse, modele) = {
        let r = etat.reglages.lock().unwrap();
        (r.adresse_ollama.clone(), r.modele_local.clone())
    };
    if modele.is_empty() {
        return Err("Choisis d'abord un modèle.".into());
    }
    let local = ia::ConfigLocale { adresse, modele: modele.clone(), mode: ia::ModeLocal::Toujours };
    let requete = ia::gemini::Requete {
        modele: String::new(),
        parties: vec![ia::gemini::Partie::Texte("Réponds uniquement par le mot : OK".into())],
        temperature: Some(0.0),
        ..Default::default()
    };
    let debut = std::time::Instant::now();
    let reponse = etat.ia.appeler_local(&etat.db, "test", &local, &requete).await?;
    let secondes = debut.elapsed().as_secs_f32();
    let texte: String = reponse.texte.trim().chars().take(60).collect();
    Ok(format!("L'IA locale répond ({modele}, {secondes:.1} s) : « {texte} »"))
}

/// Télécharge un modèle dans Ollama ; l'avancement arrive par l'évènement `ia-locale-telechargement`.
#[tauri::command]
pub async fn telecharger_modele_local(app: AppHandle, etat: State<'_, Etat>, modele: String) -> Result<(), String> {
    use std::sync::atomic::Ordering;
    use tauri::Emitter;
    let modele = modele.trim().to_string();
    if !ia::ollama::nom_modele_valide(&modele) {
        return Err("Nom de modèle invalide (ex. qwen3.5:4b).".into());
    }
    {
        let mut en_cours = etat.ia.telechargement.lock().unwrap();
        if let Some(m) = en_cours.as_ref() {
            return Err(format!("Un téléchargement est déjà en cours ({m})."));
        }
        *en_cours = Some(modele.clone());
    }
    etat.ia.annuler_telechargement.store(false, Ordering::SeqCst);
    let adresse = etat.reglages.lock().unwrap().adresse_ollama.clone();
    // On n'envoie un évènement que si le pourcentage ou l'étape change (sinon des milliers par seconde).
    let mut dernier = (String::new(), u64::MAX);
    let resultat = etat
        .ia
        .local
        .telecharger(&adresse, &modele, |a| {
            let pourcent = (a.fait * 100).checked_div(a.total).unwrap_or(0);
            let etape = a.statut.split(' ').next().unwrap_or("").to_string();
            if (etape.clone(), pourcent) != dernier {
                dernier = (etape, pourcent);
                let _ = app.emit_to("main", "ia-locale-telechargement", &a);
            }
            !etat.ia.annuler_telechargement.load(Ordering::SeqCst)
        })
        .await;
    *etat.ia.telechargement.lock().unwrap() = None;
    resultat.map_err(|e| e.to_string())
}

#[tauri::command]
pub fn annuler_telechargement_local(etat: State<Etat>) {
    etat.ia.annuler_telechargement.store(true, std::sync::atomic::Ordering::SeqCst);
}

/// Supprime un modèle d'Ollama (libère la place sur le disque).
#[tauri::command]
pub async fn supprimer_modele_local(etat: State<'_, Etat>, modele: String) -> Result<(), String> {
    if !ia::ollama::nom_modele_valide(&modele) {
        return Err("Nom de modèle invalide.".into());
    }
    let adresse = etat.reglages.lock().unwrap().adresse_ollama.clone();
    etat.ia.local.supprimer(&adresse, &modele).await?;
    let choisi = etat.reglages.lock().unwrap().modele_local == modele;
    if choisi {
        modifier_reglages(&etat, |r| r.modele_local.clear())?;
    }
    Ok(())
}

// ---------- Questions générées par l'IA ----------

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ChapitreIa {
    matiere: String,
    chapitre: Option<String>,
    notes: i64,
    questions_ia: i64,
    /// Questions de l'IA à revoir maintenant.
    questions_dues: i64,
    cartes_mentales: i64,
}

/// Chapitres (et notes générales de chaque matière) pour lesquels on peut générer des questions.
#[tauri::command]
pub fn chapitres_ia(etat: State<Etat>) -> Result<Vec<ChapitreIa>, String> {
    let conn = etat.db.lock().unwrap();
    let mut req = conn
        .prepare(
            "SELECT n.matiere, n.chapitre, count(*),
                    (SELECT count(*) FROM cartes c WHERE c.cle LIKE 'ia|%' AND c.suspendue = 0
                       AND c.matiere = n.matiere AND c.chapitre IS n.chapitre),
                    (SELECT count(*) FROM cartes c WHERE c.cle LIKE 'ia|%' AND c.suspendue = 0 AND c.active = 1
                       AND c.echeance IS NOT NULL AND c.echeance <= ?1
                       AND c.matiere = n.matiere AND c.chapitre IS n.chapitre),
                    (SELECT count(*) FROM cartes_mentales m WHERE m.matiere = n.matiere AND m.chapitre IS n.chapitre)
             FROM notes n WHERE n.matiere IS NOT NULL AND n.genre NOT IN ('accueil', 'schema', 'revision')
             GROUP BY n.matiere, n.chapitre ORDER BY n.matiere, n.chapitre",
        )
        .map_err(|e| e.to_string())?;
    let lignes = req
        .query_map([maintenant_ms()], |r| {
            Ok(ChapitreIa {
                matiere: r.get(0)?,
                chapitre: r.get(1)?,
                notes: r.get(2)?,
                questions_ia: r.get(3)?,
                questions_dues: r.get(4)?,
                cartes_mentales: r.get(5)?,
            })
        })
        .map_err(|e| e.to_string())?;
    lignes.collect::<Result<_, _>>().map_err(|e| e.to_string())
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Estimation {
    appels: usize,
    extraits: usize,
    caracteres: usize,
}

/// Nombre d'appels à Gemini que coûterait une génération (affiché avant de lancer).
#[tauri::command]
pub fn estimer_questions(etat: State<Etat>, matiere: String, chapitre: Option<String>) -> Result<Estimation, String> {
    let extraits = ia::ancrage::extraits_du_chapitre(&etat.db.lock().unwrap(), &matiere, chapitre.as_deref())
        .map_err(|e| e.to_string())?;
    Ok(Estimation {
        appels: ia::ancrage::lots(&extraits).len(),
        extraits: extraits.len(),
        caracteres: extraits.iter().map(|e| e.texte.len()).sum(),
    })
}

#[tauri::command]
pub async fn generer_questions(
    etat: State<'_, Etat>,
    matiere: String,
    chapitre: Option<String>,
    nombre: usize,
    avec_sources_web: Option<bool>,
    tache: Option<String>,
) -> Result<ia::questions::ResultatGeneration, String> {
    etat.taches.annulable(tache, generer_questions_(&etat, matiere, chapitre, nombre, avec_sources_web)).await
}

async fn generer_questions_(
    etat: &Etat,
    matiere: String,
    chapitre: Option<String>,
    nombre: usize,
    avec_sources_web: Option<bool>,
) -> Result<ia::questions::ResultatGeneration, String> {
    use ia::gemini::{Partie, Requete};
    use ia::questions::{self, LotQuestions};

    let nombre = nombre.clamp(1, 30);
    let (extraits, deja) = {
        let conn = etat.db.lock().unwrap();
        let mut extraits =
            ia::ancrage::extraits_du_chapitre(&conn, &matiere, chapitre.as_deref()).map_err(|e| e.to_string())?;
        // Sources web du chapitre : seulement si l'élève l'a demandé (l'IA reste ancrée sur ses notes par défaut).
        if avec_sources_web.unwrap_or(false) {
            let s = sources_web::sources(&conn, Some(&matiere), Some(chapitre.as_deref())).map_err(|e| e.to_string())?;
            let premier = extraits.len() + 1;
            extraits.extend(sources_web::en_extraits(&s, premier));
        }
        let deja = questions::deja_posees(&conn, &matiere, chapitre.as_deref()).map_err(|e| e.to_string())?;
        (extraits, deja)
    };
    if extraits.is_empty() {
        return Err("Pas assez de contenu dans ces notes pour générer des questions.".into());
    }
    let modele = etat.reglages.lock().unwrap().modele_ia.clone();
    let lots = ia::ancrage::lots(&extraits);
    let par_lot = nombre.div_ceil(lots.len());
    let mut resultat = ia::questions::ResultatGeneration::default();

    for lot in &lots {
        let requete = Requete {
            modele: modele.clone(),
            systeme: ia::ancrage::CONSIGNE_ANCRAGE.into(),
            parties: vec![Partie::Texte(format!(
                "{}\n\nExtraits des notes ({} › {}) :\n\n{}",
                questions::consigne(par_lot, &deja),
                matiere,
                chapitre.as_deref().unwrap_or("notes générales"),
                ia::ancrage::texte_des_extraits(lot)
            ))],
            schema: Some(questions::schema(par_lot)),
            temperature: Some(0.7),
        };
        // La réponse est refusée (et redemandée) si aucune question n'est correctement ancrée.
        let valider = |l: &LotQuestions| {
            let raisons: Vec<String> = l.questions.iter().filter_map(|q| questions::preparer(q.clone(), lot).err()).collect();
            if raisons.len() < l.questions.len() {
                Ok(())
            } else {
                Err(format!("aucune question valide ({})", raisons[..raisons.len().min(3)].join(" ; ")))
            }
        };
        resultat.appels += 1;
        let local = etat.config_locale();
        let (reponse, modele) =
            etat.ia.appeler_json_simple(&etat.db, "questions", etat.limites_ia(), local.as_ref(), &requete, valider).await?;
        resultat.modele = modele.clone();
        let mut bonnes = Vec::new();
        for q in reponse.questions {
            match questions::preparer(q, lot) {
                Ok(q) => bonnes.push(q),
                Err(raison) => {
                    resultat.rejetees += 1;
                    resultat.raisons.push(raison);
                }
            }
        }
        resultat.creees += questions::enregistrer(
            &mut etat.db.lock().unwrap(),
            &modele,
            &matiere,
            chapitre.as_deref(),
            lot,
            &bonnes,
            maintenant_ms(),
        )
        .map_err(|e| e.to_string())?;
    }
    Ok(resultat)
}

// ---------- Correction par l'IA et méthode Feynman ----------

/// Correction d'une réponse à une question ouverte, à partir de la réponse attendue et de l'extrait de la note.
#[tauri::command]
pub async fn corriger_reponse(
    etat: State<'_, Etat>,
    carte_id: i64,
    reponse: String,
    tache: Option<String>,
) -> Result<ia::correction::Correction, String> {
    etat.taches.annulable(tache, corriger_reponse_(&etat, carte_id, reponse)).await
}

async fn corriger_reponse_(etat: &Etat, carte_id: i64, reponse: String) -> Result<ia::correction::Correction, String> {
    use ia::correction;
    use ia::gemini::{Partie, Requete};
    if reponse.trim().len() < 2 {
        return Err("Écris d'abord ta réponse.".into());
    }
    let carte = revision::carte(&etat.db.lock().unwrap(), carte_id)
        .map_err(|e| e.to_string())?
        .ok_or("Carte introuvable.")?;
    let extrait = crate::parser::texte_brut(&carte.extrait);
    let requete = Requete {
        modele: etat.reglages.lock().unwrap().modele_ia.clone(),
        systeme: ia::ancrage::CONSIGNE_ANCRAGE.into(),
        parties: vec![Partie::Texte(correction::consigne_correction(&carte.question, &carte.reponse, &extrait, &reponse))],
        schema: Some(correction::schema_correction()),
        temperature: Some(0.2),
    };
    let local = etat.config_locale();
    let (mut resultat, modele) = etat
        .ia
        .appeler_json_simple(&etat.db, "correction", etat.limites_ia(), local.as_ref(), &requete, correction::valider_correction)
        .await?;
    resultat.modele = modele;
    enregistrer_generation(&etat, "correction", &resultat.modele, carte.matiere.as_deref(), carte.chapitre.as_deref(), &resultat);
    Ok(resultat)
}

/// Garde une trace de chaque réponse de l'IA (consultable hors ligne).
fn enregistrer_generation(
    etat: &Etat,
    usage: &str,
    modele: &str,
    matiere: Option<&str>,
    chapitre: Option<&str>,
    resultat: &impl Serialize,
) {
    let _ = etat.db.lock().unwrap().execute(
        "INSERT INTO generations_ia(date, usage, modele, matiere, chapitre, resultat) VALUES (?1, ?2, ?3, ?4, ?5, ?6)",
        rusqlite::params![maintenant_ms(), usage, modele, matiere, chapitre, serde_json::to_string(resultat).unwrap_or_default()],
    );
}

/// Notions proposées pour la méthode Feynman : les titres des sections du chapitre.
#[tauri::command]
pub fn notions_feynman(etat: State<Etat>, matiere: String, chapitre: Option<String>) -> Result<Vec<String>, String> {
    let extraits = ia::ancrage::extraits_du_chapitre(&etat.db.lock().unwrap(), &matiere, chapitre.as_deref())
        .map_err(|e| e.to_string())?;
    let mut notions: Vec<String> = Vec::new();
    for e in extraits {
        let n = if e.section.is_empty() || e.section == e.titre_note { e.titre_note.clone() } else { e.section.clone() };
        let n = n.trim_start_matches(|c: char| c.is_ascii_digit() || c == '.' || c == ' ').to_string();
        if !n.is_empty() && !notions.contains(&n) && !n.starts_with("Fiche de synthèse") {
            notions.push(n);
        }
    }
    Ok(notions)
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ResultatFeynman {
    id: i64,
    retour: ia::correction::RetourFeynman,
    /// Les extraits utilisés (pour afficher les sources citées).
    extraits: Vec<ia::ancrage::Extrait>,
    /// Modèle qui a répondu (« local:… » pour l'IA locale).
    modele: String,
}

#[tauri::command]
pub async fn evaluer_feynman(
    etat: State<'_, Etat>,
    matiere: String,
    chapitre: Option<String>,
    notion: String,
    explication: String,
    tache: Option<String>,
) -> Result<ResultatFeynman, String> {
    etat.taches.annulable(tache, evaluer_feynman_(&etat, matiere, chapitre, notion, explication)).await
}

async fn evaluer_feynman_(
    etat: &Etat,
    matiere: String,
    chapitre: Option<String>,
    notion: String,
    explication: String,
) -> Result<ResultatFeynman, String> {
    use ia::correction;
    use ia::gemini::{Partie, Requete};
    if explication.split_whitespace().count() < 15 {
        return Err("Ton explication est trop courte : développe un peu (au moins quelques phrases).".into());
    }
    let tous = ia::ancrage::extraits_du_chapitre(&etat.db.lock().unwrap(), &matiere, chapitre.as_deref())
        .map_err(|e| e.to_string())?;
    let extraits = ia::ancrage::extraits_pour_notion(&tous, &notion);
    if extraits.is_empty() {
        return Err("Pas de notes sur ce chapitre pour comparer.".into());
    }
    let requete = Requete {
        modele: etat.reglages.lock().unwrap().modele_ia.clone(),
        systeme: ia::ancrage::CONSIGNE_ANCRAGE.into(),
        parties: vec![Partie::Texte(format!(
            "{}\n\nExtraits des notes :\n\n{}",
            correction::consigne_feynman(&notion, &explication),
            ia::ancrage::texte_des_extraits(&extraits)
        ))],
        schema: Some(correction::schema_feynman()),
        temperature: Some(0.2),
    };
    let local = etat.config_locale();
    let (mut retour, modele) = etat
        .ia
        .appeler_json_simple(&etat.db, "feynman", etat.limites_ia(), local.as_ref(), &requete, correction::valider_feynman)
        .await?;
    correction::verifier_feynman(&mut retour, &extraits);
    let id = {
        let conn = etat.db.lock().unwrap();
        conn.execute(
            "INSERT INTO feynman(date, matiere, chapitre, notion, explication, note, retour) VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7)",
            rusqlite::params![
                maintenant_ms(),
                matiere,
                chapitre,
                notion,
                explication,
                retour.note,
                serde_json::to_string(&retour).unwrap_or_default()
            ],
        )
        .map_err(|e| e.to_string())?;
        conn.last_insert_rowid()
    };
    Ok(ResultatFeynman { id, retour, extraits, modele })
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub struct EssaiFeynman {
    id: i64,
    date: i64,
    matiere: String,
    chapitre: Option<String>,
    notion: String,
    explication: String,
    note: i64,
    retour: serde_json::Value,
}

/// Les explications déjà faites (consultables hors ligne).
#[tauri::command]
pub fn historique_feynman(etat: State<Etat>) -> Result<Vec<EssaiFeynman>, String> {
    let conn = etat.db.lock().unwrap();
    let mut req = conn
        .prepare("SELECT id, date, matiere, chapitre, notion, explication, note, retour FROM feynman ORDER BY date DESC LIMIT 50")
        .map_err(|e| e.to_string())?;
    let lignes = req
        .query_map([], |r| {
            Ok(EssaiFeynman {
                id: r.get(0)?,
                date: r.get(1)?,
                matiere: r.get(2)?,
                chapitre: r.get(3)?,
                notion: r.get(4)?,
                explication: r.get(5)?,
                note: r.get(6)?,
                retour: serde_json::from_str(&r.get::<_, String>(7)?).unwrap_or_default(),
            })
        })
        .map_err(|e| e.to_string())?;
    lignes.collect::<Result<_, _>>().map_err(|e| e.to_string())
}

// ---------- Espace web ----------

#[tauri::command]
pub fn web_onglets(web: State<web::EspaceWeb>) -> Vec<web::Onglet> {
    web.liste()
}

/// Asynchrone : sous Windows, créer une webview depuis une commande synchrone (exécutée sur le
/// thread principal) bloque l'app (problème connu de WebView2).
#[tauri::command]
pub async fn web_ouvrir(
    app: AppHandle,
    web: State<'_, web::EspaceWeb>,
    adresse: String,
    zone: web::Zone,
) -> Result<u32, String> {
    web.ouvrir(&app, &adresse, zone)
}

#[tauri::command]
pub fn web_naviguer(app: AppHandle, web: State<web::EspaceWeb>, id: u32, adresse: String) -> Result<(), String> {
    let url = web::vers_url(&adresse).ok_or("Adresse invalide (seules les pages web http/https sont autorisées).")?;
    web.webview(&app, id)?.navigate(url).map_err(|e| e.to_string())
}

/// Retour (-1), avance (1) ou rechargement (0) dans l'historique de l'onglet.
#[tauri::command]
pub fn web_historique(app: AppHandle, web: State<web::EspaceWeb>, id: u32, sens: i32) -> Result<(), String> {
    let w = web.webview(&app, id)?;
    match sens {
        0 => w.reload(),
        s => w.eval(format!("history.go({})", s.clamp(-1, 1))),
    }
    .map_err(|e| e.to_string())
}

#[tauri::command]
pub fn web_activer(app: AppHandle, web: State<web::EspaceWeb>, id: u32) {
    web.activer(&app, id);
}

#[tauri::command]
pub fn web_fermer(app: AppHandle, web: State<web::EspaceWeb>, id: u32) {
    web.fermer(&app, id);
}

/// La zone d'affichage a bougé (fenêtre redimensionnée, panneau ouvert…).
#[tauri::command]
pub fn web_zone(app: AppHandle, web: State<web::EspaceWeb>, zone: web::Zone) {
    web.placer(&app, zone);
}

#[tauri::command]
pub fn web_accueil(app: AppHandle, web: State<web::EspaceWeb>) {
    web.accueil(&app);
}

/// En quittant l'espace web, les pages sont masquées (elles restent chargées).
#[tauri::command]
pub fn web_masquer(app: AppHandle, web: State<web::EspaceWeb>) {
    web.montrer_seulement(&app, None);
}

/// Texte sélectionné dans la page de l'onglet (pour l'enregistrer comme source).
#[tauri::command]
pub async fn web_selection(app: AppHandle, web: State<'_, web::EspaceWeb>, id: u32) -> Result<String, String> {
    let (envoi, reception) = tokio::sync::oneshot::channel::<String>();
    let envoi = std::sync::Mutex::new(Some(envoi));
    web.webview(&app, id)?
        .eval_with_callback("(window.getSelection() || '').toString()", move |json| {
            if let Some(e) = envoi.lock().unwrap().take() {
                let _ = e.send(serde_json::from_str::<String>(&json).unwrap_or_default());
            }
        })
        .map_err(|e| e.to_string())?;
    tokio::time::timeout(std::time::Duration::from_secs(5), reception)
        .await
        .map_err(|_| "La page ne répond pas.".to_string())?
        .map_err(|_| "La page ne répond pas.".to_string())
}

/// Pour les tests manuels uniquement : évalue du JavaScript dans un onglet (refusé dans la version finale).
#[tauri::command]
pub async fn web_eval_debug(
    app: AppHandle,
    web: State<'_, web::EspaceWeb>,
    id: u32,
    js: String,
) -> Result<String, String> {
    if !cfg!(debug_assertions) {
        return Err("Indisponible dans la version finale.".into());
    }
    let (envoi, reception) = tokio::sync::oneshot::channel::<String>();
    let envoi = std::sync::Mutex::new(Some(envoi));
    web.webview(&app, id)?
        .eval_with_callback(js, move |json| {
            if let Some(e) = envoi.lock().unwrap().take() {
                let _ = e.send(json);
            }
        })
        .map_err(|e| e.to_string())?;
    tokio::time::timeout(std::time::Duration::from_secs(5), reception).await.map_err(|_| "délai".to_string())?.map_err(|e| e.to_string())
}

#[tauri::command]
pub fn favoris(etat: State<Etat>) -> Result<Vec<sources_web::Favori>, String> {
    sources_web::favoris(&etat.db.lock().unwrap()).map_err(|e| e.to_string())
}

#[tauri::command]
pub fn ajouter_favori(etat: State<Etat>, favori: sources_web::Favori) -> Result<i64, String> {
    sources_web::ajouter_favori(&etat.db.lock().unwrap(), &favori, maintenant_ms())
}

#[tauri::command]
pub fn supprimer_favori(etat: State<Etat>, id: i64) -> Result<(), String> {
    sources_web::supprimer_favori(&etat.db.lock().unwrap(), id).map(|_| ()).map_err(|e| e.to_string())
}

#[tauri::command]
pub fn sources_web(etat: State<Etat>, matiere: Option<String>) -> Result<Vec<sources_web::SourceWeb>, String> {
    sources_web::sources(&etat.db.lock().unwrap(), matiere.as_deref(), None).map_err(|e| e.to_string())
}

#[tauri::command]
pub fn ajouter_source_web(etat: State<Etat>, source: sources_web::SourceWeb) -> Result<i64, String> {
    sources_web::ajouter_source(&etat.db.lock().unwrap(), &source, maintenant_ms())
}

#[tauri::command]
pub fn supprimer_source_web(etat: State<Etat>, id: i64) -> Result<(), String> {
    sources_web::supprimer_source(&etat.db.lock().unwrap(), id).map(|_| ()).map_err(|e| e.to_string())
}

// ---------- Vision : questions sur les schémas ----------

#[tauri::command]
pub fn schemas_ia(etat: State<Etat>, matiere: Option<String>) -> Result<Vec<ia::vision::SchemaInfo>, String> {
    ia::vision::schemas(&etat.db.lock().unwrap(), matiere.as_deref()).map_err(|e| e.to_string())
}

/// Questions sur un schéma (1 appel) : l'interface envoie le dessin rendu en PNG (base64).
#[tauri::command]
pub async fn generer_questions_schema(
    etat: State<'_, Etat>,
    chemin: String,
    image: String,
    nombre: usize,
    tache: Option<String>,
) -> Result<ia::questions::ResultatGeneration, String> {
    etat.taches.annulable(tache, generer_questions_schema_(&etat, chemin, image, nombre)).await
}

async fn generer_questions_schema_(
    etat: &Etat,
    chemin: String,
    image: String,
    nombre: usize,
) -> Result<ia::questions::ResultatGeneration, String> {
    use ia::gemini::{Partie, Requete};
    use ia::vision;
    // Garde-fou : une image PNG en base64 raisonnable (≤ 8 Mo).
    if image.len() > 8_000_000 || !image.starts_with("iVBOR") {
        return Err("Image du schéma invalide.".into());
    }
    let nombre = nombre.clamp(1, 10);
    let (info, extraits, deja) = {
        let conn = etat.db.lock().unwrap();
        let info = vision::schema(&conn, &chemin).map_err(|e| e.to_string())?.ok_or("Schéma introuvable.")?;
        let extraits = vision::extraits_lies(&conn, &info, 12_000).map_err(|e| e.to_string())?;
        let deja = vision::deja_posees(&conn, &chemin).map_err(|e| e.to_string())?;
        (info, extraits, deja)
    };
    let modele = etat.reglages.lock().unwrap().modele_ia.clone();
    let requete = Requete {
        modele: modele.clone(),
        systeme: vision::CONSIGNE_SYSTEME.into(),
        parties: vec![
            Partie::Texte(format!("{}{}", vision::consigne(&info, nombre, &deja), vision::contexte(&extraits))),
            Partie::Image { mime: "image/png".into(), base64: image },
        ],
        schema: Some(vision::schema_json()),
        temperature: Some(0.5),
    };
    // Refusé (et redemandé) si aucune question ne porte sur une légende réelle du schéma.
    let valider = |l: &vision::LotVision| {
        if l.questions.iter().any(|q| vision::defaut(q, &info.legendes).is_none()) {
            Ok(())
        } else {
            let raisons: Vec<String> = l.questions.iter().filter_map(|q| vision::defaut(q, &info.legendes)).take(3).collect();
            Err(format!("aucune question valide ({})", raisons.join(" ; ")))
        }
    };
    let lot = etat.ia.appeler_json(&etat.db, "vision", etat.limites_ia(), &requete, valider).await?;
    let (bonnes, mauvaises): (Vec<_>, Vec<_>) =
        lot.questions.into_iter().partition(|q| vision::defaut(q, &info.legendes).is_none());
    let mut resultat = ia::questions::ResultatGeneration { appels: 1, ..Default::default() };
    resultat.rejetees = mauvaises.len();
    resultat.raisons = mauvaises.iter().filter_map(|q| vision::defaut(q, &info.legendes)).collect();
    resultat.creees = vision::enregistrer(&mut etat.db.lock().unwrap(), &info, &modele, &extraits, &bonnes, maintenant_ms())
        .map_err(|e| e.to_string())?;
    Ok(resultat)
}

// ---------- Cartes mentales ----------

/// « Plan du chapitre » construit à partir des notes, sans IA (toujours disponible).
#[tauri::command]
pub fn plan_chapitre(
    etat: State<Etat>,
    matiere: String,
    chapitre: Option<String>,
) -> Result<Option<cartes_mentales::Noeud>, String> {
    cartes_mentales::plan_du_chapitre(&etat.db.lock().unwrap(), &matiere, chapitre.as_deref()).map_err(|e| e.to_string())
}

/// Cartes rédigées par l'IA pour ce chapitre (consultables hors ligne).
#[tauri::command]
pub fn cartes_mentales_chapitre(
    etat: State<Etat>,
    matiere: String,
    chapitre: Option<String>,
) -> Result<Vec<cartes_mentales::CarteMentale>, String> {
    cartes_mentales::lister(&etat.db.lock().unwrap(), &matiere, chapitre.as_deref()).map_err(|e| e.to_string())
}

#[tauri::command]
pub fn supprimer_carte_mentale(etat: State<Etat>, id: i64) -> Result<(), String> {
    cartes_mentales::supprimer(&etat.db.lock().unwrap(), id).map(|_| ()).map_err(|e| e.to_string())
}

/// Carte mentale rédigée par Gemini à partir des notes du chapitre (1 appel).
#[tauri::command]
pub async fn generer_carte_mentale(
    etat: State<'_, Etat>,
    matiere: String,
    chapitre: Option<String>,
    tache: Option<String>,
) -> Result<cartes_mentales::CarteMentale, String> {
    etat.taches.annulable(tache, generer_carte_mentale_(&etat, matiere, chapitre)).await
}

async fn generer_carte_mentale_(
    etat: &Etat,
    matiere: String,
    chapitre: Option<String>,
) -> Result<cartes_mentales::CarteMentale, String> {
    use ia::gemini::{Partie, Requete};
    let tous = ia::ancrage::extraits_du_chapitre(&etat.db.lock().unwrap(), &matiere, chapitre.as_deref())
        .map_err(|e| e.to_string())?;
    // Un seul appel : la fiche de synthèse et les premières notes (dans la limite d'un appel).
    let extraits = ia::ancrage::lots(&tous).into_iter().next().unwrap_or_default();
    if extraits.is_empty() {
        return Err("Pas assez de contenu dans ces notes pour faire une carte mentale.".into());
    }
    let nom = chapitre.clone().unwrap_or_else(|| format!("{matiere} (notes générales)"));
    let modele = etat.reglages.lock().unwrap().modele_ia.clone();
    let requete = Requete {
        modele: modele.clone(),
        systeme: cartes_mentales::CONSIGNE_SYSTEME.into(),
        parties: vec![Partie::Texte(format!(
            "{}\n\nExtraits des notes :\n\n{}",
            cartes_mentales::consigne(&nom),
            ia::ancrage::texte_des_extraits(&extraits)
        ))],
        schema: Some(cartes_mentales::schema()),
        temperature: Some(0.3),
    };
    let local = etat.config_locale();
    let (carte, modele) = etat
        .ia
        .appeler_json_simple(&etat.db, "carte_mentale", etat.limites_ia(), local.as_ref(), &requete, |c| {
            cartes_mentales::valider(c, &extraits)
        })
        .await?;
    let arbre = cartes_mentales::en_noeud(carte, &extraits);
    cartes_mentales::enregistrer(&etat.db.lock().unwrap(), &matiere, chapitre.as_deref(), &modele, &arbre, maintenant_ms())
        .map_err(|e| e.to_string())
}

// ---------- Podcasts ----------

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub struct EstimationPodcast {
    appels_script: usize,
    appels_voix: usize,
}

#[tauri::command]
pub fn estimer_podcast(duree_minutes: u32) -> EstimationPodcast {
    let (appels_script, appels_voix) = podcasts::appels_estimes(duree_minutes);
    EstimationPodcast { appels_script, appels_voix }
}

#[tauri::command]
pub fn podcasts(etat: State<Etat>) -> Result<Vec<podcasts::Podcast>, String> {
    podcasts::lister(&etat.db.lock().unwrap()).map_err(|e| e.to_string())
}

fn podcast(etat: &Etat, id: i64) -> Result<podcasts::Podcast, String> {
    podcasts::lire_podcast(&etat.db.lock().unwrap(), id).map_err(|e| e.to_string())?.ok_or_else(|| "Podcast introuvable.".into())
}

/// Étape 1 : Gemini écrit le dialogue (1 appel). Rien n'est encore enregistré.
#[tauri::command]
pub async fn ecrire_podcast(
    etat: State<'_, Etat>,
    matiere: String,
    chapitre: Option<String>,
    options: podcasts::OptionsPodcast,
    tache: Option<String>,
) -> Result<podcasts::Podcast, String> {
    etat.taches.annulable(tache, ecrire_podcast_(&etat, matiere, chapitre, options)).await
}

async fn ecrire_podcast_(
    etat: &Etat,
    matiere: String,
    chapitre: Option<String>,
    options: podcasts::OptionsPodcast,
) -> Result<podcasts::Podcast, String> {
    use ia::gemini::{Partie, Requete};
    options.valider()?;
    let tous = ia::ancrage::extraits_du_chapitre(&etat.db.lock().unwrap(), &matiere, chapitre.as_deref())
        .map_err(|e| e.to_string())?;
    let extraits = ia::ancrage::lots(&tous).into_iter().next().unwrap_or_default();
    if extraits.is_empty() {
        return Err("Pas assez de contenu dans ces notes pour faire un podcast.".into());
    }
    let (modele, modele_tts, voix_a, voix_b) = {
        let r = etat.reglages.lock().unwrap();
        (r.modele_ia.clone(), r.modele_tts.clone(), r.voix_a.clone(), r.voix_b.clone())
    };
    let nom = chapitre.clone().unwrap_or_else(|| format!("{matiere} (notes générales)"));
    let requete = Requete {
        modele: modele.clone(),
        systeme: podcasts::CONSIGNE_SYSTEME.into(),
        parties: vec![Partie::Texte(format!(
            "{}\n\nExtraits des notes :\n\n{}",
            podcasts::consigne(&nom, &options),
            ia::ancrage::texte_des_extraits(&extraits)
        ))],
        schema: Some(podcasts::schema()),
        temperature: Some(0.8),
    };
    let script = etat
        .ia
        .appeler_json(&etat.db, "podcast_script", etat.limites_ia(), &requete, |s| podcasts::valider(s, &options, &extraits))
        .await?;
    let (titre, repliques, morceaux) = podcasts::preparer(script, &options, &extraits);
    let (a, b) = options.prenoms();
    let orateurs = vec![
        podcasts::Orateur { nom: a.into(), voix: voix_a },
        podcasts::Orateur { nom: b.into(), voix: voix_b },
    ];
    let conn = etat.db.lock().unwrap();
    let id = podcasts::creer(
        &conn,
        &matiere,
        chapitre.as_deref(),
        &titre,
        &options,
        &modele,
        &modele_tts,
        &orateurs,
        &repliques,
        &extraits,
        morceaux,
        maintenant_ms(),
    )
    .map_err(|e| e.to_string())?;
    podcasts::lire_podcast(&conn, id).map_err(|e| e.to_string())?.ok_or_else(|| "Podcast introuvable.".into())
}

/// Étape 2 : enregistre le morceau suivant (1 appel de synthèse vocale) et le sauvegarde aussitôt.
#[tauri::command]
pub async fn enregistrer_morceau_podcast(
    etat: State<'_, Etat>,
    id: i64,
    tache: Option<String>,
) -> Result<podcasts::Podcast, String> {
    etat.taches.annulable(tache, enregistrer_morceau_podcast_(&etat, id)).await
}

async fn enregistrer_morceau_podcast_(etat: &Etat, id: i64) -> Result<podcasts::Podcast, String> {
    let mut p = podcast(&etat, id)?;
    let k = p.morceaux_faits;
    if k >= p.morceaux {
        return Ok(p);
    }
    let requete = ia::gemini::RequeteVoix {
        modele: p.modele_tts.clone(),
        orateurs: p.orateurs.iter().map(|o| (o.nom.clone(), o.voix.clone())).collect(),
        repliques: p
            .repliques
            .iter()
            .filter(|r| r.morceau == k)
            .map(|r| (r.nom.clone(), r.texte.clone(), r.style.clone()))
            .collect(),
    };
    let voix = etat.ia.appeler_voix(&etat.db, "podcast_voix", etat.limites_ia(), &requete).await?;
    let dossier = podcasts::dossier_audio(&etat.dossier)?;
    std::fs::write(dossier.join(p.fichier_morceau(k)), voix.pcm.en_wav()).map_err(|e| format!("Enregistrement du morceau : {e}"))?;
    p.morceaux_faits = k + 1;
    podcasts::morceau_enregistre(&etat.db.lock().unwrap(), id, p.morceaux_faits).map_err(|e| e.to_string())?;
    Ok(p)
}

/// Étape 3 : assemble les morceaux en un seul fichier et minute la transcription.
#[tauri::command]
pub fn assembler_podcast(etat: State<Etat>, id: i64) -> Result<podcasts::Podcast, String> {
    let mut p = podcast(&etat, id)?;
    if p.morceaux_faits < p.morceaux {
        return Err("Tous les morceaux ne sont pas encore enregistrés.".into());
    }
    let dossier = podcasts::dossier_audio(&etat.dossier)?;
    let (wav, duree, durees) = podcasts::assembler(&dossier, &p)?;
    let fichier = format!("podcast-{id}.wav");
    std::fs::write(dossier.join(&fichier), wav).map_err(|e| format!("Enregistrement du podcast : {e}"))?;
    podcasts::minuter(&mut p.repliques, &durees);
    podcasts::definir_fichier(&etat.db.lock().unwrap(), id, &fichier, duree as i64, &p.repliques).map_err(|e| e.to_string())?;
    for k in 0..p.morceaux {
        let _ = std::fs::remove_file(dossier.join(p.fichier_morceau(k)));
    }
    p.fichier = Some(fichier);
    p.duree_ms = Some(duree as i64);
    Ok(p)
}

/// Octets du fichier audio (lecture hors ligne).
#[tauri::command]
pub fn lire_audio_podcast(etat: State<Etat>, id: i64) -> Result<Response, String> {
    let p = podcast(&etat, id)?;
    let fichier = p.fichier.ok_or("Ce podcast n'est pas encore prêt.")?;
    let dossier = podcasts::dossier_audio(&etat.dossier)?;
    std::fs::read(dossier.join(fichier)).map(Response::new).map_err(|e| format!("Lecture de l'audio : {e}"))
}

/// Remplace le WAV par sa version compressée (Opus dans un conteneur WebM), produite par l'interface.
/// Les octets arrivent en corps brut ; l'identifiant dans l'en-tête `x-podcast`.
#[tauri::command]
pub fn remplacer_audio_podcast(etat: State<Etat>, request: tauri::ipc::Request) -> Result<podcasts::Podcast, String> {
    let tauri::ipc::InvokeBody::Raw(octets) = request.body() else {
        return Err("Données audio attendues.".into());
    };
    let id: i64 = request
        .headers()
        .get("x-podcast")
        .and_then(|v| v.to_str().ok())
        .and_then(|v| v.parse().ok())
        .ok_or("Podcast non précisé.")?;
    // Un fichier WebM commence par la signature EBML 1A 45 DF A3.
    if octets.len() < 1000 || !octets.starts_with(&[0x1A, 0x45, 0xDF, 0xA3]) {
        return Err("Fichier compressé invalide.".into());
    }
    let mut p = podcast(&etat, id)?;
    let ancien = p.fichier.clone().ok_or("Ce podcast n'est pas encore assemblé.")?;
    let dossier = podcasts::dossier_audio(&etat.dossier)?;
    let fichier = format!("podcast-{id}.webm");
    std::fs::write(dossier.join(&fichier), octets).map_err(|e| format!("Enregistrement de l'audio : {e}"))?;
    podcasts::definir_fichier(&etat.db.lock().unwrap(), id, &fichier, p.duree_ms.unwrap_or(0), &p.repliques)
        .map_err(|e| e.to_string())?;
    if ancien != fichier {
        let _ = std::fs::remove_file(dossier.join(ancien));
    }
    p.fichier = Some(fichier);
    Ok(p)
}

#[tauri::command]
pub fn position_podcast(etat: State<Etat>, id: i64, position_ms: i64) -> Result<(), String> {
    podcasts::definir_position(&etat.db.lock().unwrap(), id, position_ms).map_err(|e| e.to_string())
}

#[tauri::command]
pub fn supprimer_podcast(etat: State<Etat>, id: i64) -> Result<(), String> {
    let dossier = podcasts::dossier_audio(&etat.dossier)?;
    podcasts::supprimer(&etat.db.lock().unwrap(), &dossier, id)
}

// ---------- Vidéos (diaporamas narrés) ----------

#[tauri::command]
pub fn videos(etat: State<Etat>) -> Result<Vec<videos::Video>, String> {
    videos::lister(&etat.db.lock().unwrap()).map_err(|e| e.to_string())
}

fn video(etat: &Etat, id: i64) -> Result<videos::Video, String> {
    videos::lire_video(&etat.db.lock().unwrap(), id).map_err(|e| e.to_string())?.ok_or_else(|| "Vidéo introuvable.".into())
}

/// Étape 1 : Gemini écrit les diapositives et leur narration (1 appel).
#[tauri::command]
pub async fn ecrire_video(
    etat: State<'_, Etat>,
    matiere: String,
    chapitre: Option<String>,
    options: videos::OptionsVideo,
    tache: Option<String>,
) -> Result<videos::Video, String> {
    etat.taches.annulable(tache, ecrire_video_(&etat, matiere, chapitre, options)).await
}

async fn ecrire_video_(
    etat: &Etat,
    matiere: String,
    chapitre: Option<String>,
    options: videos::OptionsVideo,
) -> Result<videos::Video, String> {
    use ia::gemini::{Partie, Requete};
    options.valider()?;
    let (tous, schemas) = {
        let conn = etat.db.lock().unwrap();
        (
            ia::ancrage::extraits_du_chapitre(&conn, &matiere, chapitre.as_deref()).map_err(|e| e.to_string())?,
            videos::schemas_du_chapitre(&conn, &matiere, chapitre.as_deref()).map_err(|e| e.to_string())?,
        )
    };
    let extraits = ia::ancrage::lots(&tous).into_iter().next().unwrap_or_default();
    if extraits.is_empty() {
        return Err("Pas assez de contenu dans ces notes pour faire une vidéo.".into());
    }
    let (modele, modele_tts, voix) = {
        let r = etat.reglages.lock().unwrap();
        (r.modele_ia.clone(), r.modele_tts.clone(), r.voix_video.clone())
    };
    let nom = chapitre.clone().unwrap_or_else(|| format!("{matiere} (notes générales)"));
    let requete = Requete {
        modele: modele.clone(),
        systeme: videos::CONSIGNE_SYSTEME.into(),
        parties: vec![Partie::Texte(format!(
            "{}\n\nExtraits des notes :\n\n{}",
            videos::consigne(&nom, &options, &schemas),
            ia::ancrage::texte_des_extraits(&extraits)
        ))],
        schema: Some(videos::schema_json()),
        temperature: Some(0.6),
    };
    let script = etat
        .ia
        .appeler_json(&etat.db, "video_script", etat.limites_ia(), &requete, |s| {
            videos::valider(s, &options, &extraits, &schemas)
        })
        .await?;
    let (titre, diapos) = videos::preparer(script, &extraits, &schemas);
    let conn = etat.db.lock().unwrap();
    let id = videos::creer(
        &conn,
        &matiere,
        chapitre.as_deref(),
        &titre,
        &options,
        &modele,
        &modele_tts,
        &voix,
        &diapos,
        &extraits,
        maintenant_ms(),
    )
    .map_err(|e| e.to_string())?;
    videos::lire_video(&conn, id).map_err(|e| e.to_string())?.ok_or_else(|| "Vidéo introuvable.".into())
}

/// Étape 2 : enregistre la narration de la diapositive suivante (1 appel) et la sauvegarde aussitôt.
#[tauri::command]
pub async fn enregistrer_narration_video(
    etat: State<'_, Etat>,
    id: i64,
    tache: Option<String>,
) -> Result<videos::Video, String> {
    etat.taches.annulable(tache, enregistrer_narration_video_(&etat, id)).await
}

async fn enregistrer_narration_video_(etat: &Etat, id: i64) -> Result<videos::Video, String> {
    let mut v = video(&etat, id)?;
    let k = v.faites;
    let Some(diapo) = v.diapos.get(k) else { return Ok(v) };
    let requete = ia::gemini::RequeteVoix {
        modele: v.modele_tts.clone(),
        orateurs: vec![("Narration".into(), v.voix.clone())],
        repliques: vec![(
            "Narration".into(),
            diapo.narration.clone(),
            "voix claire et chaleureuse de professeur, rythme posé".into(),
        )],
    };
    let voix = etat.ia.appeler_voix(&etat.db, "video_voix", etat.limites_ia(), &requete).await?;
    let dossier = podcasts::dossier_audio(&etat.dossier)?;
    std::fs::write(dossier.join(v.fichier_narration(k)), voix.pcm.en_wav())
        .map_err(|e| format!("Enregistrement de la narration : {e}"))?;
    videos::narration_enregistree(&etat.db.lock().unwrap(), &mut v, k, voix.pcm.duree_ms()).map_err(|e| e.to_string())?;
    Ok(v)
}

/// Étape 3 : toute la narration (WAV, silences compris), que l'interface met sous les images.
#[tauri::command]
pub fn narration_video(etat: State<Etat>, id: i64) -> Result<Response, String> {
    let v = video(&etat, id)?;
    if v.faites < v.diapos.len() {
        return Err("Toutes les narrations ne sont pas encore enregistrées.".into());
    }
    let dossier = podcasts::dossier_audio(&etat.dossier)?;
    videos::narration_complete(&dossier, &v).map(Response::new)
}

/// Étape 4 : la vidéo assemblée par l'interface (corps brut ; identifiant et durée dans les en-têtes).
#[tauri::command]
pub fn enregistrer_fichier_video(etat: State<Etat>, request: tauri::ipc::Request) -> Result<videos::Video, String> {
    let tauri::ipc::InvokeBody::Raw(octets) = request.body() else {
        return Err("Données vidéo attendues.".into());
    };
    let entete = |nom: &str| -> Option<i64> { request.headers().get(nom)?.to_str().ok()?.parse().ok() };
    let id = entete("x-video").ok_or("Vidéo non précisée.")?;
    let duree = entete("x-duree").unwrap_or(0);
    // MP4 : bloc « ftyp » au début ; WebM : signature EBML 1A 45 DF A3.
    let extension = if octets.len() > 1000 && octets.get(4..8) == Some(b"ftyp") {
        "mp4"
    } else if octets.len() > 1000 && octets.starts_with(&[0x1A, 0x45, 0xDF, 0xA3]) {
        "webm"
    } else {
        return Err("Fichier vidéo invalide.".into());
    };
    let mut v = video(&etat, id)?;
    let dossier = videos::dossier_videos(&etat.dossier)?;
    let fichier = format!("video-{id}.{extension}");
    std::fs::write(dossier.join(&fichier), octets).map_err(|e| format!("Enregistrement de la vidéo : {e}"))?;
    if let Some(ancien) = v.fichier.as_ref().filter(|a| **a != fichier) {
        let _ = std::fs::remove_file(dossier.join(ancien));
    }
    videos::definir_fichier(&etat.db.lock().unwrap(), id, &fichier, duree).map_err(|e| e.to_string())?;
    // La narration est dans la vidéo : les WAV (≈ 3 Mo par minute) ne servent plus.
    let audio = podcasts::dossier_audio(&etat.dossier)?;
    for k in 0..v.diapos.len() {
        let _ = std::fs::remove_file(audio.join(v.fichier_narration(k)));
    }
    v.fichier = Some(fichier);
    v.duree_ms = Some(duree);
    Ok(v)
}

#[tauri::command]
pub fn lire_fichier_video(etat: State<Etat>, id: i64) -> Result<Response, String> {
    let v = video(&etat, id)?;
    let fichier = v.fichier.ok_or("Cette vidéo n'est pas encore prête.")?;
    let dossier = videos::dossier_videos(&etat.dossier)?;
    std::fs::read(dossier.join(fichier)).map(Response::new).map_err(|e| format!("Lecture de la vidéo : {e}"))
}

/// Chemin complet du fichier vidéo (pour le montrer dans l'Explorateur et le partager).
#[tauri::command]
pub fn chemin_video(etat: State<Etat>, id: i64) -> Result<String, String> {
    let v = video(&etat, id)?;
    let fichier = v.fichier.ok_or("Cette vidéo n'est pas encore prête.")?;
    Ok(videos::dossier_videos(&etat.dossier)?.join(fichier).to_string_lossy().into())
}

#[tauri::command]
pub fn supprimer_video(etat: State<Etat>, id: i64) -> Result<(), String> {
    let audio = podcasts::dossier_audio(&etat.dossier)?;
    let dossier = videos::dossier_videos(&etat.dossier)?;
    videos::supprimer(&etat.db.lock().unwrap(), &audio, &dossier, id)
}

// ---------- Exercices type bac et mode examen ----------

#[tauri::command]
pub fn types_exercices(matiere: String) -> Vec<ia::exercices::TypeExercice> {
    ia::exercices::types_pour(&matiere)
}

/// Un sujet rédigé par Gemini à partir des notes (un appel), enregistré pour être refait hors ligne.
#[tauri::command]
pub async fn generer_sujet(
    etat: State<'_, Etat>,
    type_id: String,
    chapitres: Vec<ia::copies::ChapitreChoisi>,
    duree_minutes: Option<u32>,
    tache: Option<String>,
) -> Result<ia::copies::SujetEnregistre, String> {
    etat.taches.annulable(tache, generer_sujet_(&etat, type_id, chapitres, duree_minutes)).await
}

async fn generer_sujet_(
    etat: &Etat,
    type_id: String,
    chapitres: Vec<ia::copies::ChapitreChoisi>,
    duree_minutes: Option<u32>,
) -> Result<ia::copies::SujetEnregistre, String> {
    use ia::ancrage::{self, CARACTERES_PAR_APPEL};
    use ia::exercices;
    use ia::gemini::{Partie, Requete};

    let premier = chapitres.first().ok_or("Choisis au moins un chapitre.")?;
    let t = exercices::type_par_id(&premier.matiere, &type_id).ok_or("Type d'exercice inconnu pour cette matière.")?;
    if t.id != exercices::EXAMEN.id && chapitres.len() > 1 {
        return Err("Un exercice porte sur un seul chapitre ; le mode examen peut en réunir plusieurs.".into());
    }
    if chapitres.len() > 8 {
        return Err("8 chapitres au plus pour un même sujet.".into());
    }
    let duree = duree_minutes.unwrap_or(t.duree_minutes).clamp(5, 180);
    let mut matieres: Vec<&str> = Vec::new();
    for c in &chapitres {
        if !matieres.contains(&c.matiere.as_str()) {
            matieres.push(&c.matiere);
        }
    }
    let matiere = matieres.join(" + ");

    let (extraits, deja) = {
        let conn = etat.db.lock().unwrap();
        // La méthodologie d'abord (au plus 12 000 caractères), le cours remplit le reste.
        let mut methodo = Vec::new();
        let mut taille = 0;
        for e in ancrage::extraits_methodologie(&conn, t.methodologie).map_err(|e| e.to_string())? {
            if taille + e.texte.len() <= 12_000 {
                taille += e.texte.len();
                methodo.push(e);
            }
        }
        let mut listes = Vec::new();
        for c in &chapitres {
            listes.push(ancrage::extraits_du_chapitre(&conn, &c.matiere, c.chapitre.as_deref()).map_err(|e| e.to_string())?);
        }
        let cours = ancrage::melanger(listes, CARACTERES_PAR_APPEL - taille);
        if cours.is_empty() {
            return Err("Pas assez de contenu dans ces notes pour créer un sujet.".into());
        }
        let debut = cours.len() + 1;
        let mut extraits = cours;
        extraits.extend(ancrage::renumeroter(methodo, debut));
        let deja = ia::copies::titres_deja_proposes(&conn, t.id, &chapitres).map_err(|e| e.to_string())?;
        (extraits, deja)
    };

    let noms_chapitres: Vec<String> = chapitres
        .iter()
        .map(|c| format!("{} › {}", c.matiere, c.chapitre.as_deref().unwrap_or("notes générales")))
        .collect();
    let mut consigne = exercices::consigne_generation(&t, duree, &matiere);
    if !deja.is_empty() {
        consigne += &format!("\n\nSujets déjà proposés sur ces notes (propose autre chose) : {}.", deja.join(" ; "));
    }
    let modele = etat.reglages.lock().unwrap().modele_ia.clone();
    let requete = Requete {
        modele: modele.clone(),
        systeme: exercices::CONSIGNE_EXERCICES.into(),
        parties: vec![Partie::Texte(format!(
            "{consigne}\n\nExtraits des notes ({}) :\n\n{}",
            noms_chapitres.join(" ; "),
            ancrage::texte_des_extraits(&extraits)
        ))],
        schema: Some(exercices::schema_sujet()),
        temperature: Some(0.8),
    };
    let ids: Vec<String> = extraits.iter().map(|e| e.id.clone()).collect();
    let mut sujet = etat
        .ia
        .appeler_json(&etat.db, "exercice", etat.limites_ia(), &requete, |s| exercices::valider_sujet(s, &ids))
        .await?;
    exercices::verifier_calculs(&mut sujet);
    for q in &mut sujet.questions {
        q.source = q.source.trim().trim_start_matches('[').trim_end_matches(']').to_uppercase();
    }
    let conn = etat.db.lock().unwrap();
    let id = ia::copies::enregistrer_sujet(&conn, t.id, &matiere, &chapitres, &modele, duree, &sujet, &extraits, maintenant_ms())
        .map_err(|e| e.to_string())?;
    ia::copies::lire_sujet(&conn, id).map_err(|e| e.to_string())?.ok_or_else(|| "Sujet introuvable.".into())
}

#[tauri::command]
pub fn dernieres_notes(etat: State<Etat>) -> Result<Vec<ia::copies::NoteCopie>, String> {
    ia::copies::dernieres_notes(&etat.db.lock().unwrap(), 20).map_err(|e| e.to_string())
}

/// Enregistre un export (Markdown) dans `%APPDATA%\Revisio\exports` ou dans un dossier choisi,
/// jamais dans le vault. Renvoie le chemin du fichier.
#[tauri::command]
pub fn enregistrer_export(etat: State<Etat>, nom: String, contenu: String, dossier: Option<String>) -> Result<String, String> {
    let nom_valide = !nom.is_empty()
        && nom.ends_with(".md")
        && nom.chars().all(|c| c.is_ascii_alphanumeric() || "-_.".contains(c))
        && !nom.starts_with('.');
    if !nom_valide {
        return Err("Nom de fichier invalide.".into());
    }
    let dossier = match dossier {
        Some(d) => {
            let d = std::path::PathBuf::from(d);
            if !d.is_dir() {
                return Err("Ce dossier n'existe pas.".into());
            }
            d
        }
        None => etat.dossier.join("exports"),
    };
    let vault = etat.reglages.lock().unwrap().vault();
    crate::paths::verifier_hors_vault(&dossier, vault.as_deref())?;
    std::fs::create_dir_all(&dossier).map_err(|e| format!("Dossier d'export : {e}"))?;
    let chemin = dossier.join(nom);
    std::fs::write(&chemin, contenu).map_err(|e| format!("Enregistrement de l'export : {e}"))?;
    Ok(chemin.to_string_lossy().into())
}

/// Montre le fichier exporté dans l'Explorateur Windows.
#[tauri::command]
pub fn montrer_fichier(app: AppHandle, chemin: String) -> Result<(), String> {
    app.opener().reveal_item_in_dir(chemin).map_err(|e| format!("Impossible d'ouvrir l'Explorateur : {e}"))
}

#[tauri::command]
pub fn sujets_enregistres(etat: State<Etat>) -> Result<Vec<ia::copies::ResumeSujet>, String> {
    ia::copies::lister_sujets(&etat.db.lock().unwrap()).map_err(|e| e.to_string())
}

#[tauri::command]
pub fn sujet_enregistre(etat: State<Etat>, id: i64) -> Result<ia::copies::SujetEnregistre, String> {
    ia::copies::lire_sujet(&etat.db.lock().unwrap(), id)
        .map_err(|e| e.to_string())?
        .ok_or_else(|| "Sujet introuvable.".into())
}

#[tauri::command]
pub fn supprimer_sujet(etat: State<Etat>, id: i64) -> Result<(), String> {
    ia::copies::supprimer_sujet(&etat.db.lock().unwrap(), id).map(|_| ()).map_err(|e| e.to_string())
}

/// Rend la copie : tout ce qui n'est pas une rédaction est corrigé tout de suite, hors ligne.
#[tauri::command]
pub fn rendre_copie(
    etat: State<Etat>,
    sujet_id: i64,
    reponses: Vec<String>,
    duree_ms: i64,
) -> Result<ia::copies::Copie, String> {
    let conn = etat.db.lock().unwrap();
    let sujet = ia::copies::lire_sujet(&conn, sujet_id).map_err(|e| e.to_string())?.ok_or("Sujet introuvable.")?;
    let resultats =
        ia::exercices::corriger_automatiquement(&sujet.sujet, &reponses, ia::exercices::est_langue(&sujet.matiere));
    let copie = ia::copies::enregistrer_copie(&conn, sujet_id, maintenant_ms(), duree_ms.max(0), &reponses, &resultats)
        .map_err(|e| e.to_string())?;
    // Les erreurs corrigées tout de suite rejoignent le carnet d'erreurs.
    let numeros: Vec<u32> = resultats.iter().map(|r| r.numero).collect();
    ia::copies::noter_erreurs(&conn, &sujet, &reponses, &resultats, &numeros, maintenant_ms()).map_err(|e| e.to_string())?;
    Ok(copie)
}

/// Les rédactions de la copie corrigées par Gemini (un appel), d'après les extraits enregistrés avec le sujet.
#[tauri::command]
pub async fn corriger_redactions(
    etat: State<'_, Etat>,
    copie_id: i64,
    tache: Option<String>,
) -> Result<ia::copies::Copie, String> {
    etat.taches.annulable(tache, corriger_redactions_(&etat, copie_id)).await
}

async fn corriger_redactions_(etat: &Etat, copie_id: i64) -> Result<ia::copies::Copie, String> {
    use ia::exercices;
    use ia::gemini::{Partie, Requete};

    let (mut copie, sujet) = {
        let conn = etat.db.lock().unwrap();
        let copie = ia::copies::lire_copie(&conn, copie_id).map_err(|e| e.to_string())?.ok_or("Copie introuvable.")?;
        let sujet = ia::copies::lire_sujet(&conn, copie.sujet_id).map_err(|e| e.to_string())?.ok_or("Sujet introuvable.")?;
        (copie, sujet)
    };
    let a_corriger = copie.a_corriger();
    if a_corriger.is_empty() {
        return Ok(copie);
    }
    let requete = Requete {
        modele: etat.reglages.lock().unwrap().modele_ia.clone(),
        systeme: exercices::CONSIGNE_CORRECTION.into(),
        parties: vec![Partie::Texte(format!(
            "{}\n\nExtraits des notes :\n\n{}",
            exercices::consigne_correction(&sujet.sujet, &copie.reponses, &copie.resultats),
            ia::ancrage::texte_des_extraits(&sujet.extraits)
        ))],
        schema: Some(exercices::schema_correction()),
        temperature: Some(0.2),
    };
    let correction = etat
        .ia
        .appeler_json(&etat.db, "correction_copie", etat.limites_ia(), &requete, |c| {
            exercices::valider_correction(c, &a_corriger)
        })
        .await?;
    exercices::appliquer_correction(&mut copie.resultats, &correction);
    copie.appreciation = correction.appreciation;
    let conn = etat.db.lock().unwrap();
    ia::copies::mettre_a_jour_copie(&conn, &mut copie).map_err(|e| e.to_string())?;
    // Les rédactions jugées fausses rejoignent elles aussi le carnet d'erreurs.
    let numeros: Vec<u32> = a_corriger.iter().map(|&i| i as u32 + 1).collect();
    ia::copies::noter_erreurs(&conn, &sujet, &copie.reponses, &copie.resultats, &numeros, maintenant_ms())
        .map_err(|e| e.to_string())?;
    Ok(copie)
}

/// « Question mal posée » : la question ne compte plus dans la note (ou compte de nouveau).
#[tauri::command]
pub fn retirer_question(etat: State<Etat>, copie_id: i64, numero: u32, retiree: bool) -> Result<ia::copies::Copie, String> {
    let conn = etat.db.lock().unwrap();
    let mut copie = ia::copies::lire_copie(&conn, copie_id).map_err(|e| e.to_string())?.ok_or("Copie introuvable.")?;
    let r = copie.resultats.iter_mut().find(|r| r.numero == numero).ok_or("Question introuvable.")?;
    r.retiree = retiree;
    ia::copies::mettre_a_jour_copie(&conn, &mut copie).map_err(|e| e.to_string())?;
    Ok(copie)
}

fn modifier_reglages(etat: &Etat, modif: impl FnOnce(&mut Reglages)) -> Result<Reglages, String> {
    let mut reglages = etat.reglages.lock().unwrap();
    let mut nouveaux = reglages.clone();
    modif(&mut nouveaux);
    settings::enregistrer(&etat.dossier, &nouveaux)?;
    *reglages = nouveaux.clone();
    Ok(nouveaux)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn entete_decode() {
        assert_eq!(decoder_entete("Sp%C3%A9%20SVT%2FNote.md"), "Spé SVT/Note.md");
        assert_eq!(decoder_entete("100%"), "100%");
    }

    #[test]
    fn dossier_des_images_comme_obsidian() {
        let racine = std::env::temp_dir().join(format!("revisio-test-pj-{}", std::process::id()));
        std::fs::create_dir_all(&racine).unwrap();
        // Dossier qui n'est pas un vault Obsidian : _Assets.
        assert_eq!(dossier_pieces_jointes(&racine, "SVT/Note.md"), "_Assets");
        std::fs::create_dir_all(racine.join(".obsidian")).unwrap();
        // Vault Obsidian sans réglage : racine (comportement d'Obsidian).
        assert_eq!(dossier_pieces_jointes(&racine, "SVT/Note.md"), "");
        let regler = |v: &str| std::fs::write(racine.join(".obsidian").join("app.json"), format!("{{\"attachmentFolderPath\":\"{v}\"}}")).unwrap();
        regler("./");
        assert_eq!(dossier_pieces_jointes(&racine, "SVT/Chap/Note.md"), "SVT/Chap");
        regler("./images");
        assert_eq!(dossier_pieces_jointes(&racine, "SVT/Chap/Note.md"), "SVT/Chap/images");
        assert_eq!(dossier_pieces_jointes(&racine, "Note.md"), "images");
        regler("_Assets");
        assert_eq!(dossier_pieces_jointes(&racine, "SVT/Note.md"), "_Assets");
        std::fs::remove_dir_all(&racine).unwrap();
    }
}
