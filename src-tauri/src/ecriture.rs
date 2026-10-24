//! SEUL module de Révisio qui écrit dans le vault, et seulement quand l'élève l'a permis
//! (Réglages → Écriture dans le vault ; désactivé par défaut).
//!
//! Garde-fous :
//! - uniquement des notes `.md`, jamais dans un dossier caché (`.obsidian`, `.git`…) ni dans
//!   `_Templates`, jamais un dessin `.excalidraw.md`, jamais en dehors du vault ;
//! - une création ne remplace jamais un fichier existant ;
//! - une modification vérifie que la note n'a pas changé depuis qu'on l'a lue ;
//! - chaque écriture est notée dans `historique_vault` avec le contenu d'avant : elle peut
//!   toujours être annulée (si la note n'a pas été retouchée depuis).
//!
//! La lecture du vault reste l'affaire de `vault.rs` (lecture seule).

use std::fs::OpenOptions;
use std::io::Write;
use std::path::{Component, Path, PathBuf};

use rusqlite::{params, Connection, OptionalExtension};
use serde::Serialize;

use crate::paths::est_dans;
use crate::vault;

pub const DESACTIVEE: &str =
    "L'écriture dans le vault est désactivée. Tu peux l'autoriser dans Réglages, Écriture dans le vault.";

/// Caractères interdits dans un nom de fichier Windows (plus « # ^ [ ] » qui gênent les liens Obsidian).
const INTERDITS: &[char] = &['<', '>', ':', '"', '/', '\\', '|', '?', '*', '#', '^', '[', ']'];

/// Transforme un titre de note en nom de fichier valide (sans « .md »).
pub fn nom_de_fichier(titre: &str) -> String {
    let propre: String = titre
        .chars()
        .map(|c| if INTERDITS.contains(&c) || c.is_control() { ' ' } else { c })
        .collect();
    let mut nom = propre.split_whitespace().collect::<Vec<_>>().join(" ");
    nom = nom.trim_matches(|c: char| c == '.' || c == ' ').to_string();
    if nom.chars().count() > 120 {
        nom = nom.chars().take(120).collect::<String>().trim_end().to_string();
    }
    nom
}

/// Images acceptées comme pièces jointes (collées ou déposées dans une note).
pub const EXTENSIONS_IMAGES: &[&str] = &["png", "jpg", "jpeg", "gif", "webp", "svg"];

/// Vérifications communes à toute écriture : chemin relatif propre, à l'intérieur du dossier de notes,
/// hors dossiers réservés (`.obsidian`, `.git`, `_Templates`…), sans caractère interdit.
fn chemin_sur(racine: &Path, relatif: &str) -> Result<PathBuf, String> {
    let refus = |raison: &str| Err(format!("Écriture refusée ({raison}) : {relatif}"));
    let chemin = Path::new(relatif);
    if relatif.is_empty() || chemin.components().any(|c| !matches!(c, Component::Normal(_))) {
        return refus("chemin invalide");
    }
    let morceaux: Vec<String> = chemin.components().map(|c| c.as_os_str().to_string_lossy().into_owned()).collect();
    if morceaux.iter().any(|m| vault::dossier_ignore(m)) {
        return refus("dossier réservé à Obsidian ou aux modèles");
    }
    if morceaux.iter().any(|m| m.chars().any(|c| INTERDITS.contains(&c) || c.is_control()) || m.ends_with(' ')) {
        return refus("caractère interdit dans le nom");
    }
    let complet = racine.join(chemin);
    if !est_dans(&complet, racine) || complet == racine {
        return refus("hors du dossier de notes");
    }
    Ok(complet)
}

/// Chemin absolu d'une note où Révisio a le droit d'écrire, ou la raison du refus.
pub fn chemin_ecrivable(racine: &Path, relatif: &str) -> Result<PathBuf, String> {
    let complet = chemin_sur(racine, relatif)?;
    let minuscule = relatif.to_lowercase();
    if !minuscule.ends_with(".md") {
        return Err(format!("Écriture refusée (seules les notes .md sont modifiables) : {relatif}"));
    }
    if minuscule.ends_with(".excalidraw.md") {
        return Err(format!("Écriture refusée (les dessins Excalidraw ne sont jamais modifiés) : {relatif}"));
    }
    Ok(complet)
}

/// Chemin absolu d'une image jointe (création seulement).
pub fn piece_jointe_ecrivable(racine: &Path, relatif: &str) -> Result<PathBuf, String> {
    let complet = chemin_sur(racine, relatif)?;
    let extension = Path::new(relatif).extension().map(|e| e.to_string_lossy().to_lowercase()).unwrap_or_default();
    if !EXTENSIONS_IMAGES.contains(&extension.as_str()) {
        return Err(format!("Écriture refusée (seules les images sont acceptées en pièce jointe) : {relatif}"));
    }
    Ok(complet)
}

/// Une écriture faite par Révisio (pour le journal et l'annulation).
#[derive(Debug, Clone, Serialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct EntreeHistorique {
    pub id: i64,
    pub date: i64,
    pub chemin: String,
    pub genre: String,
    pub origine: String,
    pub lot_id: Option<i64>,
    pub raison: Option<String>,
    pub annulee_le: Option<i64>,
    pub chemin_nouveau: Option<String>,
    pub groupe: Option<String>,
}

/// Infos communes à une écriture.
pub struct Contexte<'a> {
    pub racine: &'a Path,
    pub autorisee: bool,
    pub origine: &'a str,
    pub lot_id: Option<i64>,
    pub raison: Option<&'a str>,
    pub maintenant: i64,
    /// Opérations liées (un renommage et les liens mis à jour, une suppression de dossier…),
    /// annulées ensemble.
    pub groupe: Option<&'a str>,
}

/// Crée une nouvelle note (et les dossiers manquants). Ne remplace jamais un fichier existant.
pub fn creer(conn: &Connection, ctx: &Contexte, relatif: &str, contenu: &str) -> Result<i64, String> {
    let complet = chemin_ecrivable(ctx.racine, relatif)?;
    creer_fichier(conn, ctx, relatif, &complet, contenu.as_bytes())
}

/// Ajoute une image jointe (collée dans une note). Ne remplace jamais un fichier existant.
pub fn creer_piece_jointe(conn: &Connection, ctx: &Contexte, relatif: &str, octets: &[u8]) -> Result<i64, String> {
    let complet = piece_jointe_ecrivable(ctx.racine, relatif)?;
    creer_fichier(conn, ctx, relatif, &complet, octets)
}

fn creer_fichier(conn: &Connection, ctx: &Contexte, relatif: &str, complet: &Path, contenu: &[u8]) -> Result<i64, String> {
    if !ctx.autorisee {
        return Err(DESACTIVEE.into());
    }
    if complet.exists() {
        return Err(format!("Un fichier porte déjà ce nom : {relatif}"));
    }
    // Dossiers manquants, créés un par un pour pouvoir les retirer à l'annulation.
    let mut dossiers_crees: Vec<String> = Vec::new();
    let morceaux: Vec<&str> = relatif.split('/').collect();
    for i in 1..morceaux.len() {
        let dossier = morceaux[..i].join("/");
        let absolu = ctx.racine.join(&dossier);
        if !absolu.is_dir() {
            std::fs::create_dir(&absolu).map_err(|e| format!("Création du dossier {dossier} : {e}"))?;
            dossiers_crees.push(dossier);
        }
    }
    let mut fichier = OpenOptions::new()
        .write(true)
        .create_new(true)
        .open(complet)
        .map_err(|e| format!("Création de {relatif} : {e}"))?;
    fichier.write_all(contenu).map_err(|e| format!("Écriture de {relatif} : {e}"))?;
    drop(fichier);
    conn.execute(
        "INSERT INTO historique_vault(date, chemin, genre, avant, apres, origine, lot_id, raison, dossiers_crees, groupe)
         VALUES (?1, ?2, 'creation', NULL, ?3, ?4, ?5, ?6, ?7, ?8)",
        params![
            ctx.maintenant,
            relatif,
            contenu,
            ctx.origine,
            ctx.lot_id,
            ctx.raison,
            serde_json::to_string(&dossiers_crees).unwrap(),
            ctx.groupe
        ],
    )
    .map_err(|e| e.to_string())?;
    Ok(conn.last_insert_rowid())
}

/// Crée un dossier (matière, chapitre…) et ses parents manquants.
pub fn creer_dossier(conn: &Connection, ctx: &Contexte, relatif: &str) -> Result<i64, String> {
    if !ctx.autorisee {
        return Err(DESACTIVEE.into());
    }
    let complet = chemin_sur(ctx.racine, relatif)?;
    if complet.exists() {
        return Err(format!("Ce dossier existe déjà : {relatif}"));
    }
    let mut dossiers_crees: Vec<String> = Vec::new();
    let morceaux: Vec<&str> = relatif.split('/').collect();
    for i in 1..=morceaux.len() {
        let dossier = morceaux[..i].join("/");
        let absolu = ctx.racine.join(&dossier);
        if !absolu.is_dir() {
            std::fs::create_dir(&absolu).map_err(|e| format!("Création du dossier {dossier} : {e}"))?;
            dossiers_crees.push(dossier);
        }
    }
    conn.execute(
        "INSERT INTO historique_vault(date, chemin, genre, avant, apres, origine, raison, dossiers_crees, groupe)
         VALUES (?1, ?2, 'dossier', NULL, x'', ?3, ?4, ?5, ?6)",
        params![ctx.maintenant, relatif, ctx.origine, ctx.raison, serde_json::to_string(&dossiers_crees).unwrap(), ctx.groupe],
    )
    .map_err(|e| e.to_string())?;
    Ok(conn.last_insert_rowid())
}

/// Renomme ou déplace une note, une image ou un dossier. Ne remplace jamais rien : la nouvelle
/// place doit être libre, et son dossier doit exister.
pub fn deplacer(conn: &Connection, ctx: &Contexte, ancien: &str, nouveau: &str) -> Result<i64, String> {
    if !ctx.autorisee {
        return Err(DESACTIVEE.into());
    }
    let source = chemin_sur(ctx.racine, ancien)?;
    let cible = chemin_sur(ctx.racine, nouveau)?;
    if !source.exists() {
        return Err(format!("Introuvable : {ancien}"));
    }
    if source.is_file() && ancien.to_lowercase().ends_with(".md") != nouveau.to_lowercase().ends_with(".md") {
        return Err("Une note doit rester une note (.md).".into());
    }
    // Sous Windows, « Note » et « note » désignent le même fichier : on autorise ce changement de casse.
    let meme_fichier = ancien.to_lowercase() == nouveau.to_lowercase();
    if cible.exists() && !meme_fichier {
        return Err(format!("Cette place est déjà prise : {nouveau}"));
    }
    if nouveau.to_lowercase().starts_with(&format!("{}/", ancien.to_lowercase())) {
        return Err("Un dossier ne peut pas être déplacé dans lui-même.".into());
    }
    if !cible.parent().is_some_and(Path::is_dir) {
        return Err(format!("Le dossier de destination n'existe pas : {nouveau}"));
    }
    std::fs::rename(&source, &cible).map_err(|e| format!("Déplacement de {ancien} : {e}"))?;
    conn.execute(
        "INSERT INTO historique_vault(date, chemin, chemin_nouveau, genre, avant, apres, origine, raison, groupe)
         VALUES (?1, ?2, ?3, 'deplacement', NULL, x'', ?4, ?5, ?6)",
        params![ctx.maintenant, ancien, nouveau, ctx.origine, ctx.raison, ctx.groupe],
    )
    .map_err(|e| e.to_string())?;
    Ok(conn.last_insert_rowid())
}

/// Taille maximale d'un dossier supprimé (son contenu est gardé dans la corbeille de Révisio).
const TAILLE_MAX_SUPPRESSION: u64 = 200 * 1024 * 1024;

/// Supprime une note, une image ou un dossier entier : chaque fichier est d'abord gardé dans
/// l'historique (la corbeille de Révisio), d'où il peut être restauré. `ctx.groupe` est obligatoire.
pub fn supprimer(conn: &Connection, ctx: &Contexte, relatif: &str) -> Result<usize, String> {
    if !ctx.autorisee {
        return Err(DESACTIVEE.into());
    }
    let groupe = ctx.groupe.ok_or("Suppression sans groupe.")?;
    let complet = chemin_sur(ctx.racine, relatif)?;
    if complet.is_file() {
        let octets = vault::lire_octets(ctx.racine, relatif)?;
        noter_suppression(conn, ctx, groupe, relatif, &octets)?;
        std::fs::remove_file(&complet).map_err(|e| format!("Suppression de {relatif} : {e}"))?;
        return Ok(1);
    }
    if !complet.is_dir() {
        return Err(format!("Introuvable : {relatif}"));
    }
    // Inventaire d'abord (rien n'est supprimé si un élément pose problème).
    let mut fichiers: Vec<String> = Vec::new();
    let mut dossiers: Vec<String> = vec![relatif.to_string()];
    let mut total = 0u64;
    for entree in walkdir::WalkDir::new(&complet).min_depth(1).follow_links(false) {
        let entree = entree.map_err(|e| format!("Lecture du dossier {relatif} : {e}"))?;
        let rel = entree
            .path()
            .strip_prefix(ctx.racine)
            .map_err(|_| "Chemin inattendu.".to_string())?
            .components()
            .map(|c| c.as_os_str().to_string_lossy().into_owned())
            .collect::<Vec<_>>()
            .join("/");
        if entree.file_name().to_string_lossy().starts_with('.') {
            return Err(format!("Ce dossier contient un élément caché ({rel}) : supprime-le depuis l'explorateur de fichiers."));
        }
        if entree.file_type().is_dir() {
            dossiers.push(rel);
        } else if entree.file_type().is_file() {
            total += entree.metadata().map(|m| m.len()).unwrap_or(0);
            fichiers.push(rel);
        } else {
            return Err(format!("Élément inattendu dans le dossier : {rel}"));
        }
    }
    if total > TAILLE_MAX_SUPPRESSION {
        return Err("Ce dossier est trop lourd pour la corbeille de Révisio (plus de 200 Mo).".into());
    }
    for f in &fichiers {
        let octets = vault::lire_octets(ctx.racine, f)?;
        noter_suppression(conn, ctx, groupe, f, &octets)?;
    }
    conn.execute(
        "INSERT INTO historique_vault(date, chemin, genre, avant, apres, origine, raison, dossiers_crees, groupe)
         VALUES (?1, ?2, 'suppression_dossier', NULL, x'', ?3, ?4, ?5, ?6)",
        params![ctx.maintenant, relatif, ctx.origine, ctx.raison, serde_json::to_string(&dossiers).unwrap(), groupe],
    )
    .map_err(|e| e.to_string())?;
    for f in &fichiers {
        std::fs::remove_file(ctx.racine.join(f)).map_err(|e| format!("Suppression de {f} : {e}"))?;
    }
    for d in dossiers.iter().rev() {
        std::fs::remove_dir(ctx.racine.join(d)).map_err(|e| format!("Suppression du dossier {d} : {e}"))?;
    }
    Ok(fichiers.len())
}

fn noter_suppression(conn: &Connection, ctx: &Contexte, groupe: &str, relatif: &str, octets: &[u8]) -> Result<(), String> {
    conn.execute(
        "INSERT INTO historique_vault(date, chemin, genre, avant, apres, origine, raison, groupe)
         VALUES (?1, ?2, 'suppression', ?3, x'', ?4, ?5, ?6)",
        params![ctx.maintenant, relatif, octets, ctx.origine, ctx.raison, groupe],
    )
    .map_err(|e| e.to_string())?;
    Ok(())
}

/// Remplace le contenu d'une note existante. `encore_pareille` reçoit le contenu actuel
/// (octets exacts) et dit s'il correspond à ce que l'élève ou l'IA avait lu : sinon, refus.
pub fn modifier(
    conn: &Connection,
    ctx: &Contexte,
    relatif: &str,
    nouveau: &str,
    encore_pareille: &dyn Fn(&[u8]) -> bool,
) -> Result<i64, String> {
    if !ctx.autorisee {
        return Err(DESACTIVEE.into());
    }
    let complet = chemin_ecrivable(ctx.racine, relatif)?;
    if !complet.is_file() {
        return Err(format!("Note introuvable : {relatif}"));
    }
    let avant = vault::lire_octets(ctx.racine, relatif)?;
    if !encore_pareille(&avant) {
        return Err("La note a été modifiée entre-temps (dans un autre programme ?). Recharge-la avant de l'enregistrer.".into());
    }
    if avant == nouveau.as_bytes() {
        return Err("Aucun changement à enregistrer.".into());
    }
    // Sauvegarde d'abord, écriture ensuite : en cas de coupure, l'ancien contenu est déjà à l'abri.
    conn.execute(
        "INSERT INTO historique_vault(date, chemin, genre, avant, apres, origine, lot_id, raison, groupe)
         VALUES (?1, ?2, 'modification', ?3, ?4, ?5, ?6, ?7, ?8)",
        params![ctx.maintenant, relatif, avant, nouveau.as_bytes(), ctx.origine, ctx.lot_id, ctx.raison, ctx.groupe],
    )
    .map_err(|e| e.to_string())?;
    let id = conn.last_insert_rowid();
    if let Err(e) = std::fs::write(&complet, nouveau.as_bytes()) {
        conn.execute("DELETE FROM historique_vault WHERE id = ?1", [id]).ok();
        return Err(format!("Écriture de {relatif} : {e}"));
    }
    Ok(id)
}

/// Résultat d'une annulation : un message, et les déplacements défaits (de → vers) pour que
/// l'appelant remette aussi la base (cartes, carnet…) dans l'ancien état.
#[derive(Debug, Default)]
pub struct Annulation {
    pub message: String,
    pub deplacements: Vec<(String, String)>,
}

/// Durée d'une « séance d'écriture » : les enregistrements automatiques d'une note pendant ce temps
/// forment une seule entrée de l'historique (annuler revient à l'état d'avant la séance).
pub const DUREE_SEANCE_ECRITURE_MS: i64 = 10 * 60 * 1000;

/// Comme `modifier`, mais prolonge la dernière modification manuelle de la même note si elle date
/// de moins de 10 minutes et que la note n'a pas changé depuis (enregistrement automatique de l'éditeur).
/// Renvoie l'identifiant de l'entrée d'historique (créée ou prolongée).
pub fn modifier_en_seance(
    conn: &Connection,
    ctx: &Contexte,
    relatif: &str,
    nouveau: &str,
    encore_pareille: &dyn Fn(&[u8]) -> bool,
) -> Result<i64, String> {
    if !ctx.autorisee {
        return Err(DESACTIVEE.into());
    }
    let complet = chemin_ecrivable(ctx.racine, relatif)?;
    if !complet.is_file() {
        return Err(format!("Note introuvable : {relatif}"));
    }
    let actuel = vault::lire_octets(ctx.racine, relatif)?;
    if !encore_pareille(&actuel) {
        return Err("La note a été modifiée entre-temps (dans un autre programme ?). Recharge-la avant de l'enregistrer.".into());
    }
    let derniere: Option<(i64, Vec<u8>, i64, String, Option<String>, Option<i64>)> = conn
        .query_row(
            "SELECT id, apres, date, origine, groupe, annulee_le FROM historique_vault
             WHERE id = (SELECT max(id) FROM historique_vault WHERE chemin = ?1) AND genre = 'modification'",
            [relatif],
            |r| Ok((r.get(0)?, r.get(1)?, r.get(2)?, r.get(3)?, r.get(4)?, r.get(5)?)),
        )
        .optional()
        .map_err(|e| e.to_string())?;
    match derniere {
        Some((id, apres, date, origine, None, None))
            if origine == "manuel" && apres == actuel && ctx.maintenant - date < DUREE_SEANCE_ECRITURE_MS =>
        {
            if actuel == nouveau.as_bytes() {
                return Ok(id);
            }
            conn.execute(
                "UPDATE historique_vault SET apres = ?2, date = ?3 WHERE id = ?1",
                params![id, nouveau.as_bytes(), ctx.maintenant],
            )
            .map_err(|e| e.to_string())?;
            if let Err(e) = std::fs::write(&complet, nouveau.as_bytes()) {
                conn.execute("UPDATE historique_vault SET apres = ?2 WHERE id = ?1", params![id, actuel]).ok();
                return Err(format!("Écriture de {relatif} : {e}"));
            }
            Ok(id)
        }
        _ => modifier(conn, ctx, relatif, nouveau, encore_pareille),
    }
}

/// Annule une écriture (ou tout son groupe : un renommage avec ses liens, une suppression de dossier…).
/// Refusé si une note a été retouchée depuis (pour ne jamais perdre ces changements).
/// Possible même si l'écriture a été désactivée ensuite : on ne fait que revenir en arrière.
pub fn annuler(conn: &Connection, racine: &Path, id: i64, maintenant: i64) -> Result<Annulation, String> {
    let groupe: Option<String> = conn
        .query_row("SELECT groupe FROM historique_vault WHERE id = ?1", [id], |r| r.get(0))
        .optional()
        .map_err(|e| e.to_string())?
        .ok_or("Modification introuvable.")?;
    match groupe {
        Some(g) => annuler_groupe(conn, racine, &g, maintenant),
        None => {
            let mut a = Annulation::default();
            annuler_une(conn, racine, id, maintenant, &mut a)?;
            Ok(a)
        }
    }
}

/// Annule toutes les opérations d'un groupe, de la plus récente à la plus ancienne.
pub fn annuler_groupe(conn: &Connection, racine: &Path, groupe: &str, maintenant: i64) -> Result<Annulation, String> {
    let ids: Vec<i64> = {
        let mut req = conn
            .prepare("SELECT id FROM historique_vault WHERE groupe = ?1 AND annulee_le IS NULL ORDER BY id DESC")
            .map_err(|e| e.to_string())?;
        let lignes = req.query_map([groupe], |r| r.get(0)).map_err(|e| e.to_string())?;
        lignes.collect::<rusqlite::Result<_>>().map_err(|e| e.to_string())?
    };
    if ids.is_empty() {
        return Err("Rien à annuler : c'est déjà fait.".into());
    }
    let mut a = Annulation::default();
    for id in ids {
        annuler_une(conn, racine, id, maintenant, &mut a)?;
    }
    Ok(a)
}

fn annuler_une(conn: &Connection, racine: &Path, id: i64, maintenant: i64, a: &mut Annulation) -> Result<(), String> {
    type Ligne = (String, Option<String>, String, Option<Vec<u8>>, Vec<u8>, String, Option<i64>);
    let ligne: Option<Ligne> = conn
        .query_row(
            "SELECT chemin, chemin_nouveau, genre, avant, apres, dossiers_crees, annulee_le FROM historique_vault WHERE id = ?1",
            [id],
            |r| Ok((r.get(0)?, r.get(1)?, r.get(2)?, r.get(3)?, r.get(4)?, r.get(5)?, r.get(6)?)),
        )
        .optional()
        .map_err(|e| e.to_string())?;
    let (chemin, chemin_nouveau, genre, avant, apres, dossiers, annulee) = ligne.ok_or("Modification introuvable.")?;
    if annulee.is_some() {
        return Err("Cette modification est déjà annulée.".into());
    }
    let marquer = || {
        conn.execute("UPDATE historique_vault SET annulee_le = ?1 WHERE id = ?2", params![maintenant, id])
            .map_err(|e| e.to_string())
    };
    let dossiers: Vec<String> = serde_json::from_str(&dossiers).unwrap_or_default();
    let complet = chemin_sur(racine, &chemin)?;
    match genre.as_str() {
        "creation" => {
            if !complet.exists() {
                a.message = format!("{chemin} n'existait déjà plus.");
            } else {
                if vault::lire_octets(racine, &chemin)? != apres {
                    return Err(retouchee(&chemin));
                }
                std::fs::remove_file(&complet).map_err(|e| format!("Suppression de {chemin} : {e}"))?;
                retirer_dossiers_vides(racine, &dossiers);
                a.message = format!("Supprimé : {chemin}");
            }
        }
        "modification" => {
            if vault::lire_octets(racine, &chemin)? != apres {
                return Err(retouchee(&chemin));
            }
            let avant = avant.ok_or("Contenu d'origine introuvable.")?;
            std::fs::write(&complet, &avant).map_err(|e| format!("Écriture de {chemin} : {e}"))?;
            a.message = format!("Note remise comme avant : {chemin}");
        }
        "dossier" => {
            retirer_dossiers_vides(racine, &dossiers);
            if complet.exists() {
                return Err(format!("Le dossier {chemin} n'est plus vide : vide-le d'abord."));
            }
            a.message = format!("Dossier retiré : {chemin}");
        }
        "deplacement" => {
            let nouveau = chemin_nouveau.ok_or("Nouvelle place inconnue.")?;
            let actuel = chemin_sur(racine, &nouveau)?;
            if !actuel.exists() {
                return Err(format!("{nouveau} n'existe plus : impossible de le remettre à sa place."));
            }
            if complet.exists() && chemin.to_lowercase() != nouveau.to_lowercase() {
                return Err(format!("La place d'origine ({chemin}) est reprise : impossible d'annuler."));
            }
            if !complet.parent().is_some_and(Path::is_dir) {
                return Err(format!("Le dossier d'origine de {chemin} n'existe plus."));
            }
            std::fs::rename(&actuel, &complet).map_err(|e| format!("Déplacement de {nouveau} : {e}"))?;
            a.deplacements.push((nouveau.clone(), chemin.clone()));
            a.message = format!("Remis à sa place : {chemin}");
        }
        "suppression" => {
            if complet.exists() {
                return Err(format!("Un fichier occupe déjà la place de {chemin} : renomme-le d'abord."));
            }
            if let Some(parent) = complet.parent() {
                std::fs::create_dir_all(parent).map_err(|e| format!("Création du dossier de {chemin} : {e}"))?;
            }
            let avant = avant.ok_or("Contenu d'origine introuvable.")?;
            let mut f = OpenOptions::new()
                .write(true)
                .create_new(true)
                .open(&complet)
                .map_err(|e| format!("Restauration de {chemin} : {e}"))?;
            f.write_all(&avant).map_err(|e| format!("Restauration de {chemin} : {e}"))?;
            if a.message.is_empty() {
                a.message = format!("Restauré : {chemin}");
            }
        }
        "suppression_dossier" => {
            for d in &dossiers {
                let p = chemin_sur(racine, d)?;
                std::fs::create_dir_all(&p).map_err(|e| format!("Création du dossier {d} : {e}"))?;
            }
            a.message = format!("Dossier restauré : {chemin}");
        }
        autre => return Err(format!("Opération inconnue : {autre}")),
    }
    marquer().map(|_| ())
}

fn retouchee(chemin: &str) -> String {
    format!("{chemin} a été modifiée depuis : l'annulation effacerait ces changements. Corrige-la plutôt avec le bouton Modifier.")
}

/// Retire les dossiers créés pour une opération, seulement s'ils sont restés vides.
fn retirer_dossiers_vides(racine: &Path, dossiers: &[String]) {
    for dossier in dossiers.iter().rev() {
        if chemin_dossier_sur(racine, dossier) {
            let _ = std::fs::remove_dir(racine.join(dossier));
        }
    }
}

/// Un dossier relatif à l'intérieur du vault, hors dossiers réservés.
fn chemin_dossier_sur(racine: &Path, relatif: &str) -> bool {
    let p = Path::new(relatif);
    !relatif.is_empty()
        && p.components().all(|c| matches!(c, Component::Normal(_)))
        && !p.components().any(|c| vault::dossier_ignore(&c.as_os_str().to_string_lossy()))
        && est_dans(&racine.join(p), racine)
}

/// Dernières écritures, de la plus récente à la plus ancienne.
pub fn historique(conn: &Connection, limite: usize) -> rusqlite::Result<Vec<EntreeHistorique>> {
    let mut req = conn.prepare(
        "SELECT id, date, chemin, genre, origine, lot_id, raison, annulee_le, chemin_nouveau, groupe FROM historique_vault
         ORDER BY id DESC LIMIT ?1",
    )?;
    let lignes = req.query_map([limite as i64], |r| {
        Ok(EntreeHistorique {
            id: r.get(0)?,
            date: r.get(1)?,
            chemin: r.get(2)?,
            genre: r.get(3)?,
            origine: r.get(4)?,
            lot_id: r.get(5)?,
            raison: r.get(6)?,
            annulee_le: r.get(7)?,
            chemin_nouveau: r.get(8)?,
            groupe: r.get(9)?,
        })
    })?;
    lignes.collect()
}

/// Un élément de la corbeille : une suppression (fichier ou dossier) qui peut être restaurée.
#[derive(Debug, Clone, Serialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct ElementCorbeille {
    pub groupe: String,
    pub date: i64,
    pub chemin: String,
    pub dossier: bool,
    pub fichiers: i64,
}

/// Suppressions non annulées, de la plus récente à la plus ancienne.
pub fn corbeille(conn: &Connection) -> rusqlite::Result<Vec<ElementCorbeille>> {
    let mut req = conn.prepare(
        "SELECT groupe, max(date),
                coalesce(max(CASE WHEN genre = 'suppression_dossier' THEN chemin END), min(chemin)),
                max(genre = 'suppression_dossier'),
                sum(genre = 'suppression')
         FROM historique_vault
         WHERE genre IN ('suppression', 'suppression_dossier') AND annulee_le IS NULL AND groupe IS NOT NULL
         GROUP BY groupe ORDER BY max(id) DESC",
    )?;
    let lignes = req.query_map([], |r| {
        Ok(ElementCorbeille {
            groupe: r.get(0)?,
            date: r.get(1)?,
            chemin: r.get(2)?,
            dossier: r.get::<_, i64>(3)? == 1,
            fichiers: r.get(4)?,
        })
    })?;
    lignes.collect()
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::db;

    fn vault_temporaire(nom: &str) -> PathBuf {
        let d = std::env::temp_dir().join(format!("revisio-ecriture-{nom}-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&d);
        std::fs::create_dir_all(d.join(".obsidian")).unwrap();
        std::fs::create_dir_all(d.join("Spé SVT")).unwrap();
        std::fs::write(d.join("Spé SVT").join("Méiose.md"), "Avant\r\n").unwrap();
        d
    }

    fn ctx(racine: &Path, autorisee: bool) -> Contexte<'_> {
        Contexte { racine, autorisee, origine: "manuel", lot_id: None, raison: None, maintenant: 1, groupe: None }
    }

    fn ctx_groupe<'a>(racine: &'a Path, groupe: &'a str) -> Contexte<'a> {
        Contexte { groupe: Some(groupe), ..ctx(racine, true) }
    }

    #[test]
    fn dossier_cree_puis_retire() {
        let r = vault_temporaire("dossier");
        let conn = db::ouvrir_en_memoire().unwrap();
        let id = creer_dossier(&conn, &ctx(&r, true), "Histoire/Chapitre 1").unwrap();
        assert!(r.join("Histoire").join("Chapitre 1").is_dir());
        assert!(creer_dossier(&conn, &ctx(&r, true), "Histoire/Chapitre 1").is_err(), "existe déjà");
        assert!(creer_dossier(&conn, &ctx(&r, true), ".obsidian/x").is_err());
        assert!(creer_dossier(&conn, &ctx(&r, false), "Maths").is_err());
        annuler(&conn, &r, id, 2).unwrap();
        assert!(!r.join("Histoire").exists());
        std::fs::remove_dir_all(&r).unwrap();
    }

    #[test]
    fn deplacement_puis_annulation() {
        let r = vault_temporaire("deplacement");
        let conn = db::ouvrir_en_memoire().unwrap();
        std::fs::create_dir_all(r.join("Spé SVT").join("Chap")).unwrap();
        let id = deplacer(&conn, &ctx(&r, true), "Spé SVT/Méiose.md", "Spé SVT/Chap/La méiose.md").unwrap();
        assert!(r.join("Spé SVT/Chap/La méiose.md").is_file() && !r.join("Spé SVT/Méiose.md").exists());
        // Refus : place prise, note devenue autre chose, dossier dans lui-même, destination absente.
        std::fs::write(r.join("Spé SVT").join("Autre.md"), "x").unwrap();
        assert!(deplacer(&conn, &ctx(&r, true), "Spé SVT/Autre.md", "Spé SVT/Chap/La méiose.md").is_err());
        assert!(deplacer(&conn, &ctx(&r, true), "Spé SVT/Autre.md", "Spé SVT/Autre.txt").is_err());
        assert!(deplacer(&conn, &ctx(&r, true), "Spé SVT", "Spé SVT/Chap/Spé SVT").is_err());
        assert!(deplacer(&conn, &ctx(&r, true), "Spé SVT/Autre.md", "Absent/Autre.md").is_err());
        let a = annuler(&conn, &r, id, 2).unwrap();
        assert_eq!(a.deplacements, vec![("Spé SVT/Chap/La méiose.md".to_string(), "Spé SVT/Méiose.md".to_string())]);
        assert_eq!(std::fs::read(r.join("Spé SVT").join("Méiose.md")).unwrap(), b"Avant\r\n");
        // Un dossier entier se déplace aussi.
        deplacer(&conn, &ctx(&r, true), "Spé SVT/Chap", "Chap").unwrap();
        assert!(r.join("Chap").is_dir());
        std::fs::remove_dir_all(&r).unwrap();
    }

    #[test]
    fn suppression_dans_la_corbeille_puis_restauration() {
        let r = vault_temporaire("corbeille");
        let conn = db::ouvrir_en_memoire().unwrap();
        std::fs::create_dir_all(r.join("Spé SVT").join("Chap").join("Vide")).unwrap();
        std::fs::write(r.join("Spé SVT").join("Chap").join("Note.md"), "Contenu").unwrap();
        std::fs::write(r.join("Spé SVT").join("Chap").join("image.png"), [0u8, 1, 2]).unwrap();
        // Sans groupe ou sans autorisation : refus.
        assert!(supprimer(&conn, &ctx(&r, true), "Spé SVT").is_err());
        assert_eq!(supprimer(&conn, &ctx(&r, false), "Spé SVT").unwrap_err(), DESACTIVEE);
        assert_eq!(supprimer(&conn, &ctx_groupe(&r, "g1"), "Spé SVT").unwrap(), 3);
        assert!(!r.join("Spé SVT").exists());
        let c = corbeille(&conn).unwrap();
        assert_eq!((c.len(), c[0].chemin.as_str(), c[0].dossier, c[0].fichiers), (1, "Spé SVT", true, 3));
        annuler_groupe(&conn, &r, "g1", 2).unwrap();
        assert_eq!(std::fs::read_to_string(r.join("Spé SVT/Chap/Note.md")).unwrap(), "Contenu");
        assert_eq!(std::fs::read(r.join("Spé SVT/Chap/image.png")).unwrap(), vec![0u8, 1, 2]);
        assert!(r.join("Spé SVT/Chap/Vide").is_dir());
        assert_eq!(std::fs::read(r.join("Spé SVT").join("Méiose.md")).unwrap(), b"Avant\r\n");
        assert!(corbeille(&conn).unwrap().is_empty());
        // Un seul fichier.
        supprimer(&conn, &ctx_groupe(&r, "g2"), "Spé SVT/Méiose.md").unwrap();
        assert!(!r.join("Spé SVT").join("Méiose.md").exists());
        annuler_groupe(&conn, &r, "g2", 3).unwrap();
        assert!(r.join("Spé SVT").join("Méiose.md").exists());
        // Un dossier contenant un élément caché n'est pas supprimé.
        std::fs::create_dir_all(r.join("Spé SVT").join(".cache")).unwrap();
        assert!(supprimer(&conn, &ctx_groupe(&r, "g3"), "Spé SVT").is_err());
        assert!(r.join("Spé SVT").join("Méiose.md").exists());
        std::fs::remove_dir_all(&r).unwrap();
    }

    #[test]
    fn images_jointes() {
        let r = vault_temporaire("images");
        let conn = db::ouvrir_en_memoire().unwrap();
        creer_piece_jointe(&conn, &ctx(&r, true), "_Assets/Image collée 1.png", &[1, 2, 3]).unwrap();
        assert_eq!(std::fs::read(r.join("_Assets").join("Image collée 1.png")).unwrap(), vec![1u8, 2, 3]);
        assert!(creer_piece_jointe(&conn, &ctx(&r, true), "_Assets/script.exe", &[0]).is_err());
        assert!(creer_piece_jointe(&conn, &ctx(&r, true), "_Assets/Image collée 1.png", &[9]).is_err(), "jamais d'écrasement");
        std::fs::remove_dir_all(&r).unwrap();
    }

    #[test]
    fn noms_de_fichier() {
        assert_eq!(nom_de_fichier("La méiose : étapes"), "La méiose étapes");
        assert_eq!(nom_de_fichier("  Vitesse v = d/t ?  "), "Vitesse v = d t");
        assert_eq!(nom_de_fichier("[[Lien]] #tag"), "Lien tag");
        assert_eq!(nom_de_fichier("..."), "");
    }

    #[test]
    fn chemins_refuses() {
        let r = vault_temporaire("refus");
        assert!(chemin_ecrivable(&r, "Spé SVT/Nouvelle.md").is_ok());
        for mauvais in [
            "../dehors.md",
            ".obsidian/app.md",
            "Spé SVT/.cache/x.md",
            "_Templates/Modèle.md",
            "Spé SVT/image.png",
            "Spé SVT/Dessin.excalidraw.md",
            r"C:\Windows\x.md",
            "Spé SVT/a:b.md",
            "",
        ] {
            assert!(chemin_ecrivable(&r, mauvais).is_err(), "{mauvais} devrait être refusé");
        }
        std::fs::remove_dir_all(&r).unwrap();
    }

    #[test]
    fn rien_sans_autorisation() {
        let r = vault_temporaire("interdit");
        let conn = db::ouvrir_en_memoire().unwrap();
        assert_eq!(creer(&conn, &ctx(&r, false), "Spé SVT/X.md", "x").unwrap_err(), DESACTIVEE);
        assert_eq!(modifier(&conn, &ctx(&r, false), "Spé SVT/Méiose.md", "x", &|_| true).unwrap_err(), DESACTIVEE);
        assert!(!r.join("Spé SVT").join("X.md").exists());
        assert_eq!(std::fs::read(r.join("Spé SVT").join("Méiose.md")).unwrap(), b"Avant\r\n");
        std::fs::remove_dir_all(&r).unwrap();
    }

    #[test]
    fn creation_puis_annulation_retire_note_et_dossiers() {
        let r = vault_temporaire("creation");
        let conn = db::ouvrir_en_memoire().unwrap();
        let id = creer(&conn, &ctx(&r, true), "Spé SVT/Chapitre 4/Mutations.md", "# Mutations\n").unwrap();
        assert_eq!(std::fs::read_to_string(r.join("Spé SVT/Chapitre 4/Mutations.md")).unwrap(), "# Mutations\n");
        // Jamais d'écrasement.
        assert!(creer(&conn, &ctx(&r, true), "Spé SVT/Méiose.md", "x").is_err());
        assert_eq!(std::fs::read(r.join("Spé SVT").join("Méiose.md")).unwrap(), b"Avant\r\n");
        annuler(&conn, &r, id, 2).unwrap();
        assert!(!r.join("Spé SVT/Chapitre 4").exists());
        assert!(r.join("Spé SVT").is_dir());
        assert!(annuler(&conn, &r, id, 3).is_err());
        assert_eq!(historique(&conn, 10).unwrap()[0].annulee_le, Some(2));
        std::fs::remove_dir_all(&r).unwrap();
    }

    #[test]
    fn modification_verifiee_et_annulable() {
        let r = vault_temporaire("modif");
        let conn = db::ouvrir_en_memoire().unwrap();
        let note = r.join("Spé SVT").join("Méiose.md");
        // La note a changé depuis la lecture : refus.
        assert!(modifier(&conn, &ctx(&r, true), "Spé SVT/Méiose.md", "Après", &|a| a == b"Autre").is_err());
        let id = modifier(&conn, &ctx(&r, true), "Spé SVT/Méiose.md", "Après", &|a| a == b"Avant\r\n").unwrap();
        assert_eq!(std::fs::read_to_string(&note).unwrap(), "Après");
        annuler(&conn, &r, id, 2).unwrap();
        assert_eq!(std::fs::read(&note).unwrap(), b"Avant\r\n");

        // Retouchée après l'écriture de Révisio : l'annulation est refusée, rien n'est perdu.
        let id = modifier(&conn, &ctx(&r, true), "Spé SVT/Méiose.md", "Après", &|_| true).unwrap();
        std::fs::write(&note, "Retouchée dans Obsidian").unwrap();
        assert!(annuler(&conn, &r, id, 3).is_err());
        assert_eq!(std::fs::read_to_string(&note).unwrap(), "Retouchée dans Obsidian");
        std::fs::remove_dir_all(&r).unwrap();
    }
}
