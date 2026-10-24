//! Photos de cours : lots (un cours = quelques photos), traitement par l'IA, écriture des
//! nouvelles notes, modifications proposées à l'élève, et surveillance d'un dossier
//! (par exemple un dossier Google Drive synchronisé sur le PC).
//!
//! Rien n'est écrit dans le vault sans l'autorisation de l'élève (`ecriture_autorisee`) ;
//! les écritures passent toutes par `ecriture.rs` (historique et annulation).

use std::collections::HashMap;
use std::path::{Path, PathBuf};
use std::sync::Mutex;
use std::time::{Duration, Instant, UNIX_EPOCH};

use rusqlite::{params, Connection, OptionalExtension};
use serde::Serialize;
use tauri::{AppHandle, Emitter, Manager};

use crate::ia::gemini::{Partie, Requete};
use crate::ia::photos::{self as ia_photos, ContexteRedaction, Redaction, Reperage};
use crate::{ecriture, maintenant_ms, vault, Etat};

/// Un lot de photos et ce qu'il a donné.
#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct LotPhotos {
    pub id: i64,
    pub date: i64,
    pub source: String,
    /// Noms des fichiers (sans le dossier).
    pub photos: Vec<String>,
    pub statut: String,
    pub matiere: Option<String>,
    pub chapitre: Option<String>,
    pub dossier: Option<String>,
    pub message: Option<String>,
}

/// Modification d'une note existante proposée par l'IA, en attente de l'accord de l'élève.
#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Proposition {
    pub id: i64,
    pub date: i64,
    pub lot_id: Option<i64>,
    pub chemin: String,
    pub avant: String,
    pub apres: String,
    pub raison: String,
}

/// État de la surveillance du dossier de photos (affiché dans l'interface).
#[derive(Debug, Clone, Default, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct EtatSurveillance {
    pub dossier: Option<String>,
    /// Nouvelles photos repérées, pas encore traitées.
    pub en_attente: usize,
    pub derniere_verification: Option<i64>,
    /// Pourquoi rien ne se passe (écriture désactivée, pas d'Internet…), ou ce qui est en cours.
    pub message: Option<String>,
    pub en_cours: bool,
}

/// Suivi en mémoire : photos vues mais pas encore « stables » (encore en cours de synchronisation).
#[derive(Default)]
pub struct SuiviPhotos {
    pub etat: EtatSurveillance,
    reperees: HashMap<PathBuf, (u64, Instant)>,
}

/// Intervalle entre deux coups d'œil au dossier.
const INTERVALLE: Duration = Duration::from_secs(15);
/// On attend qu'aucune nouvelle photo n'arrive pendant ce temps : les photos d'un même cours
/// partent ensemble.
const CALME: Duration = Duration::from_secs(60);
/// Deux photos prises à plus de 10 minutes d'écart appartiennent à deux lots différents.
const ECART_LOT_MS: i64 = 10 * 60 * 1000;

pub fn creer_lot(conn: &Connection, source: &str, photos: &[String], maintenant: i64) -> rusqlite::Result<i64> {
    conn.execute(
        "INSERT INTO lots_photos(date, source, photos, statut) VALUES (?1, ?2, ?3, 'attente')",
        params![maintenant, source, serde_json::to_string(photos).unwrap()],
    )?;
    Ok(conn.last_insert_rowid())
}

fn lire_lot(r: &rusqlite::Row) -> rusqlite::Result<LotPhotos> {
    let photos: String = r.get(3)?;
    let photos: Vec<String> = serde_json::from_str(&photos).unwrap_or_default();
    Ok(LotPhotos {
        id: r.get(0)?,
        date: r.get(1)?,
        source: r.get(2)?,
        photos: photos
            .iter()
            .map(|p| Path::new(p).file_name().map(|n| n.to_string_lossy().into_owned()).unwrap_or_default())
            .collect(),
        statut: r.get(4)?,
        matiere: r.get(5)?,
        chapitre: r.get(6)?,
        dossier: r.get(7)?,
        message: r.get(8)?,
    })
}

const COLONNES_LOT: &str = "id, date, source, photos, statut, matiere, chapitre, dossier, message";

pub fn lots(conn: &Connection, limite: usize) -> rusqlite::Result<Vec<LotPhotos>> {
    let mut req = conn.prepare(&format!("SELECT {COLONNES_LOT} FROM lots_photos ORDER BY id DESC LIMIT ?1"))?;
    let lignes = req.query_map([limite as i64], lire_lot)?;
    lignes.collect()
}

pub fn lot(conn: &Connection, id: i64) -> rusqlite::Result<Option<LotPhotos>> {
    conn.query_row(&format!("SELECT {COLONNES_LOT} FROM lots_photos WHERE id = ?1"), [id], lire_lot)
        .optional()
}

fn changer_statut(conn: &Connection, id: i64, statut: &str, message: Option<&str>) {
    let _ = conn.execute(
        "UPDATE lots_photos SET statut = ?1, message = ?2 WHERE id = ?3",
        params![statut, message, id],
    );
}

/// Propositions en attente, de la plus ancienne à la plus récente.
pub fn propositions(conn: &Connection) -> rusqlite::Result<Vec<Proposition>> {
    let mut req = conn.prepare(
        "SELECT id, date, lot_id, chemin, avant, apres, raison FROM propositions_vault
         WHERE statut = 'attente' ORDER BY id",
    )?;
    let lignes = req.query_map([], |r| {
        let avant: Vec<u8> = r.get(4)?;
        let avant = String::from_utf8_lossy(&avant);
        Ok(Proposition {
            id: r.get(0)?,
            date: r.get(1)?,
            lot_id: r.get(2)?,
            chemin: r.get(3)?,
            avant: avant.strip_prefix('\u{feff}').unwrap_or(&avant).to_string(),
            apres: r.get(5)?,
            raison: r.get(6)?,
        })
    })?;
    lignes.collect()
}

/// L'élève accepte une modification : elle est écrite (et peut être annulée ensuite).
/// Si la note a changé depuis la proposition, celle-ci est retirée.
pub fn accepter(etat: &Etat, id: i64) -> Result<String, String> {
    let reglages = etat.reglages.lock().unwrap().clone();
    let racine = reglages.vault().ok_or("Aucun vault choisi.")?;
    let conn = etat.db.lock().unwrap();
    let (lot_id, chemin, avant, apres, raison): (Option<i64>, String, Vec<u8>, String, String) = conn
        .query_row(
            "SELECT lot_id, chemin, avant, apres, raison FROM propositions_vault WHERE id = ?1 AND statut = 'attente'",
            [id],
            |r| Ok((r.get(0)?, r.get(1)?, r.get(2)?, r.get(3)?, r.get(4)?)),
        )
        .optional()
        .map_err(|e| e.to_string())?
        .ok_or("Proposition introuvable ou déjà traitée.")?;
    if vault::lire_octets(&racine, &chemin).ok().as_deref() != Some(avant.as_slice()) {
        conn.execute("UPDATE propositions_vault SET statut = 'perimee' WHERE id = ?1", [id])
            .map_err(|e| e.to_string())?;
        return Err("La note a changé depuis cette proposition (dans un autre programme ?) : proposition retirée.".into());
    }
    let ctx = ecriture::Contexte {
        racine: &racine,
        autorisee: reglages.ecriture_autorisee,
        origine: "photos",
        lot_id,
        raison: Some(&raison),
        maintenant: maintenant_ms(),
    };
    let historique = ecriture::modifier(&conn, &ctx, &chemin, &apres, &|a| a == avant.as_slice())?;
    conn.execute(
        "UPDATE propositions_vault SET statut = 'acceptee', historique_id = ?1 WHERE id = ?2",
        params![historique, id],
    )
    .map_err(|e| e.to_string())?;
    Ok(format!("Modification enregistrée : {chemin}"))
}

pub fn refuser(conn: &Connection, id: i64) -> Result<(), String> {
    conn.execute("UPDATE propositions_vault SET statut = 'refusee' WHERE id = ?1 AND statut = 'attente'", [id])
        .map_err(|e| e.to_string())?;
    Ok(())
}

/// Traite un lot : photos → repérage → rédaction → notes créées et modifications proposées.
/// Le statut du lot est mis à jour quoi qu'il arrive.
pub async fn traiter_lot(etat: &Etat, id: i64) -> Result<LotPhotos, String> {
    if !etat.reglages.lock().unwrap().ecriture_autorisee {
        return Err(ecriture::DESACTIVEE.into());
    }
    let photos: Vec<String> = {
        let conn = etat.db.lock().unwrap();
        changer_statut(&conn, id, "en_cours", None);
        let texte: String = conn
            .query_row("SELECT photos FROM lots_photos WHERE id = ?1", [id], |r| r.get(0))
            .map_err(|e| e.to_string())?;
        serde_json::from_str(&texte).unwrap_or_default()
    };
    let resultat = traiter(etat, id, photos).await;
    let conn = etat.db.lock().unwrap();
    match resultat {
        Ok((statut, message)) => changer_statut(&conn, id, statut, Some(&message)),
        Err(e) => changer_statut(&conn, id, "erreur", Some(&e)),
    }
    lot(&conn, id).map_err(|e| e.to_string())?.ok_or_else(|| "Lot introuvable.".into())
}

async fn traiter(etat: &Etat, id: i64, photos: Vec<String>) -> Result<(&'static str, String), String> {
    let reglages = etat.reglages.lock().unwrap().clone();
    let racine = reglages.vault().ok_or("Aucun vault choisi.")?;
    if photos.is_empty() {
        return Err("Aucune photo dans ce lot.".into());
    }
    // Préparation des photos (réduction, rotation) hors du fil principal : c'est un peu long.
    let chemins = photos.clone();
    let images = tauri::async_runtime::spawn_blocking(move || {
        chemins.iter().map(|c| ia_photos::preparer_photo(Path::new(c))).collect::<Result<Vec<_>, String>>()
    })
    .await
    .map_err(|e| e.to_string())??;
    let parties_images = || images.iter().map(|(mime, b64)| Partie::Image { mime: mime.clone(), base64: b64.clone() });

    // 1. Repérage
    let arbre = ia_photos::arbre(&etat.db.lock().unwrap()).map_err(|e| e.to_string())?;
    let mut parties = vec![Partie::Texte(ia_photos::consigne_reperage(images.len(), &arbre))];
    parties.extend(parties_images());
    let requete = Requete {
        modele: reglages.modele_ia.clone(),
        systeme: ia_photos::CONSIGNE_SYSTEME.into(),
        parties,
        schema: Some(ia_photos::schema_reperage()),
        temperature: Some(0.2),
    };
    let reperage: Reperage = etat
        .ia
        .appeler_json(&etat.db, "photos", etat.limites_ia(), &requete, |r| ia_photos::valider_reperage(r, &arbre))
        .await?;
    if !reperage.lisible {
        let raison = if reperage.probleme.trim().is_empty() { "photos illisibles" } else { reperage.probleme.trim() };
        return Ok(("illisible", format!("Rien n'a été écrit : {raison}")));
    }
    let dossier = reperage.dossier.trim_matches('/').to_string();
    {
        let conn = etat.db.lock().unwrap();
        let _ = conn.execute(
            "UPDATE lots_photos SET matiere = ?1, chapitre = ?2, dossier = ?3, modele = ?4 WHERE id = ?5",
            params![reperage.matiere, reperage.chapitre, dossier, reglages.modele_ia, id],
        );
    }

    // 2. Rédaction, avec le modèle de note, deux exemples et les notes existantes concernées.
    let contexte = contexte_redaction(etat, &racine, &reperage, &dossier, &arbre)?;
    let mut parties = vec![Partie::Texte(ia_photos::consigne_redaction(&contexte))];
    parties.extend(parties_images());
    let requete = Requete { parties, schema: Some(ia_photos::schema_redaction()), temperature: Some(0.3), ..requete };
    let existantes = contexte.existantes.clone();
    let redaction: Redaction = etat
        .ia
        .appeler_json(&etat.db, "photos", etat.limites_ia(), &requete, |r| {
            ia_photos::valider_redaction(r, &existantes)
        })
        .await?;

    // 3. Écriture des nouvelles notes, propositions pour les notes existantes.
    let cree_le = chrono::Local::now().format("%Y-%m-%dT%H:%M").to_string();
    let conn = etat.db.lock().unwrap();
    let ctx = ecriture::Contexte {
        racine: &racine,
        autorisee: etat.reglages.lock().unwrap().ecriture_autorisee,
        origine: "photos",
        lot_id: Some(id),
        raison: None,
        maintenant: maintenant_ms(),
    };
    let mut creees = 0;
    let mut deja = Vec::new();
    for note in &redaction.notes {
        let chemin = format!("{dossier}/{}.md", ecriture::nom_de_fichier(&note.titre));
        if racine.join(&chemin).exists() {
            deja.push(note.titre.clone());
            continue;
        }
        ecriture::creer(&conn, &ctx, &chemin, &ia_photos::note_complete(note, &cree_le))?;
        creees += 1;
    }
    let mut proposees = 0;
    for m in &redaction.modifications {
        let Ok(avant) = vault::lire_octets(&racine, &m.chemin) else { continue };
        conn.execute(
            "INSERT INTO propositions_vault(date, lot_id, chemin, avant, apres, raison) VALUES (?1, ?2, ?3, ?4, ?5, ?6)",
            params![maintenant_ms(), id, m.chemin, avant, ia_photos::retablir_sauts_de_ligne(&m.contenu), m.raison],
        )
        .map_err(|e| e.to_string())?;
        proposees += 1;
    }
    if creees == 0 && proposees == 0 && deja.is_empty() {
        return Ok(("termine", "Rien de nouveau : ces notions sont déjà dans ton vault.".into()));
    }
    let mut message = format!("{creees} note{} créée{} dans {dossier}", pluriel(creees), pluriel(creees));
    if proposees > 0 {
        message.push_str(&format!(", {proposees} modification{} à valider", pluriel(proposees)));
    }
    if !deja.is_empty() {
        message.push_str(&format!(" ; déjà présentes, non remplacées : {}", deja.join(", ")));
    }
    Ok(("termine", message))
}

fn pluriel(n: usize) -> &'static str {
    if n > 1 { "s" } else { "" }
}

fn contexte_redaction<'a>(
    etat: &Etat,
    racine: &Path,
    reperage: &'a Reperage,
    dossier: &str,
    arbre: &std::collections::BTreeMap<String, Vec<String>>,
) -> Result<ContexteRedaction<'a>, String> {
    let couper = |t: String, max: usize| if t.chars().count() > max { t.chars().take(max).collect::<String>() + "\n[…]" } else { t };
    let modeles = vault::lister_modeles(racine);
    let modele = modeles
        .iter()
        .find(|m| m.to_lowercase().contains("cours"))
        .or(modeles.first())
        .and_then(|m| vault::lire_texte(racine, m).ok())
        .map(|t| couper(t, 4000));
    // Toutes les notes du dossier choisi (pour imiter le style ET éviter les doublons) ;
    // dossier vide ou nouveau : deux notes de la même matière.
    let parent = dossier.rsplit_once('/').map(|(p, _)| p).unwrap_or(dossier);
    let chercher = |sql: &str, prefixe: &str| -> Result<Vec<String>, String> {
        let conn = etat.db.lock().unwrap();
        let mut req = conn.prepare(sql).map_err(|e| e.to_string())?;
        let lignes = req.query_map([prefixe], |r| r.get::<_, String>(0)).map_err(|e| e.to_string())?;
        Ok(lignes.filter_map(Result::ok).collect())
    };
    let du_dossier = chercher(
        "SELECT chemin FROM notes WHERE genre NOT IN ('schema', 'accueil') AND chemin LIKE ?1 || '/%'
         AND instr(substr(chemin, length(?1) + 2), '/') = 0 ORDER BY chemin LIMIT 25",
        dossier,
    )?;
    let exemples_du_dossier = !du_dossier.is_empty();
    let exemples_chemins = if exemples_du_dossier {
        du_dossier
    } else {
        chercher(
            "SELECT chemin FROM notes WHERE genre = 'cours' AND chemin LIKE ?1 || '/%' ORDER BY modifie DESC LIMIT 2",
            parent,
        )?
    };
    let mut total = 0;
    let mut exemples = Vec::new();
    for c in &exemples_chemins {
        let Ok(t) = vault::lire_texte(racine, c) else { continue };
        let t = couper(t, 3500);
        total += t.len();
        if total > 40_000 {
            break;
        }
        exemples.push((c.rsplit('/').next().unwrap_or(c).trim_end_matches(".md").to_string(), t));
    }
    let mut existantes: Vec<(String, String)> = Vec::new();
    for n in &reperage.notions {
        if n.note_existante.is_empty() || existantes.iter().any(|(c, _)| *c == n.note_existante) || existantes.len() >= 5 {
            continue;
        }
        if let Ok(t) = vault::lire_texte(racine, &n.note_existante) {
            if t.chars().count() <= 12_000 && !n.note_existante.to_lowercase().ends_with(".excalidraw.md") {
                existantes.push((n.note_existante.clone(), t));
            }
        }
    }
    Ok(ContexteRedaction {
        reperage,
        modele,
        exemples,
        exemples_du_dossier,
        existantes,
        voisines: arbre.get(dossier).cloned().unwrap_or_default(),
    })
}

// ---------- Dossier surveillé ----------

/// Photos (racine du dossier seulement) avec leur taille et leur date de modification.
pub fn photos_du_dossier(dossier: &Path) -> Vec<(PathBuf, u64, i64)> {
    let Ok(entrees) = std::fs::read_dir(dossier) else { return Vec::new() };
    let mut photos: Vec<(PathBuf, u64, i64)> = entrees
        .filter_map(Result::ok)
        .filter(|e| e.file_type().map(|t| t.is_file()).unwrap_or(false))
        .filter(|e| ia_photos::est_une_photo(&e.path()))
        .filter_map(|e| {
            let meta = e.metadata().ok()?;
            let modifie = meta.modified().ok()?.duration_since(UNIX_EPOCH).ok()?.as_millis() as i64;
            Some((e.path(), meta.len(), modifie))
        })
        .collect();
    photos.sort_by(|a, b| (a.2, &a.0).cmp(&(b.2, &b.0)));
    photos
}

/// Photos du dossier jamais vues (ni traitées, ni ignorées).
fn nouvelles(conn: &Connection, dossier: &Path) -> Vec<(PathBuf, u64, i64)> {
    photos_du_dossier(dossier)
        .into_iter()
        .filter(|(p, _, _)| {
            conn.query_row("SELECT 1 FROM photos_vues WHERE chemin = ?1", [p.to_string_lossy()], |_| Ok(()))
                .optional()
                .ok()
                .flatten()
                .is_none()
        })
        .collect()
}

/// Les photos déjà présentes quand la surveillance commence sont ignorées (sauf demande contraire).
pub fn ignorer_photos_presentes(conn: &Connection, dossier: &Path) -> rusqlite::Result<usize> {
    let photos = nouvelles(conn, dossier);
    for (p, taille, _) in &photos {
        conn.execute(
            "INSERT OR IGNORE INTO photos_vues(chemin, taille, date, lot_id) VALUES (?1, ?2, ?3, NULL)",
            params![p.to_string_lossy(), *taille as i64, maintenant_ms()],
        )?;
    }
    Ok(photos.len())
}

pub fn nombre_nouvelles(conn: &Connection, dossier: &Path) -> usize {
    nouvelles(conn, dossier).len()
}

/// Regroupe des photos (triées par date) en lots : un écart de plus de 10 minutes ou plus de
/// 10 photos commence un nouveau lot.
pub fn regrouper(photos: &[(PathBuf, u64, i64)]) -> Vec<Vec<PathBuf>> {
    let mut lots: Vec<Vec<PathBuf>> = Vec::new();
    let mut precedente: Option<i64> = None;
    for (p, _, date) in photos {
        let nouveau = match (lots.last(), precedente) {
            (Some(l), Some(d)) => l.len() >= ia_photos::PHOTOS_PAR_LOT || date - d > ECART_LOT_MS,
            _ => true,
        };
        if nouveau {
            lots.push(Vec::new());
        }
        lots.last_mut().unwrap().push(p.clone());
        precedente = Some(*date);
    }
    lots
}

/// Lance la surveillance (un coup d'œil toutes les 15 s, tant que l'app est ouverte).
/// Un simple coup d'œil plutôt qu'un « watcher » : les lecteurs Google Drive ne signalent
/// pas toujours les changements à Windows.
pub fn demarrer_surveillance(app: AppHandle) {
    tauri::async_runtime::spawn(async move {
        loop {
            tokio::time::sleep(INTERVALLE).await;
            verifier_dossier(&app).await;
        }
    });
}

fn noter(app: &AppHandle, modif: impl FnOnce(&mut EtatSurveillance)) {
    let etat = app.state::<Etat>();
    let copie = {
        let mut suivi = etat.photos.lock().unwrap();
        modif(&mut suivi.etat);
        suivi.etat.clone()
    };
    let _ = app.emit("photos-surveillance", copie);
}

async fn verifier_dossier(app: &AppHandle) {
    let etat = app.state::<Etat>();
    let (dossier, autorisee, vault_choisi) = {
        let r = etat.reglages.lock().unwrap();
        (r.dossier_photos.clone(), r.ecriture_autorisee, r.chemin_vault.is_some())
    };
    let Some(dossier) = dossier else { return };
    if etat.photos.lock().unwrap().etat.en_cours {
        return;
    }
    let dossier = PathBuf::from(dossier);
    let nouvelles = nouvelles(&etat.db.lock().unwrap(), &dossier);
    let maintenant = Instant::now();
    // Une photo est prête si sa taille n'a pas bougé depuis le coup d'œil précédent.
    let pretes: Vec<(PathBuf, u64, i64)> = {
        let mut suivi = etat.photos.lock().unwrap();
        suivi.reperees.retain(|p, _| nouvelles.iter().any(|(n, _, _)| n == p));
        let mut pretes = Vec::new();
        for (p, taille, date) in &nouvelles {
            match suivi.reperees.get(p) {
                Some((t, _)) if t == taille => pretes.push((p.clone(), *taille, *date)),
                _ => {
                    suivi.reperees.insert(p.clone(), (*taille, maintenant));
                }
            }
        }
        pretes
    };
    let dernier_arrivage = etat.photos.lock().unwrap().reperees.values().map(|(_, vu)| *vu).max();
    let calme = dernier_arrivage.map(|d| maintenant.duration_since(d) >= CALME).unwrap_or(true);
    let empechement = if !dossier.is_dir() {
        Some("Dossier introuvable (Google Drive est-il lancé ?).".to_string())
    } else if !vault_choisi {
        Some("Aucun vault choisi.".into())
    } else if !autorisee {
        Some("L'écriture dans le vault est désactivée : les photos attendent.".into())
    } else if etat.ia.gemini_coupe() {
        Some("Mode local : les photos seront traitées en repassant en ligne.".into())
    } else if !etat.ia.cle_presente() {
        Some("Aucune clé Gemini : les photos attendent.".into())
    } else {
        None
    };
    noter(app, |e| {
        e.dossier = Some(dossier.to_string_lossy().into_owned());
        e.en_attente = nouvelles.len();
        e.derniere_verification = Some(maintenant_ms());
        e.message = empechement.clone();
    });
    if empechement.is_some() || pretes.is_empty() || pretes.len() < nouvelles.len() || !calme {
        return;
    }
    if !etat.ia.internet_joignable().await {
        noter(app, |e| e.message = Some("Pas de connexion Internet : les photos attendent.".into()));
        return;
    }
    noter(app, |e| {
        e.en_cours = true;
        e.message = Some(format!("Traitement de {} photo{}…", pretes.len(), pluriel(pretes.len())));
    });
    for groupe in regrouper(&pretes) {
        let chemins: Vec<String> = groupe.iter().map(|p| p.to_string_lossy().into_owned()).collect();
        let id = {
            let conn = etat.db.lock().unwrap();
            let Ok(id) = creer_lot(&conn, "dossier", &chemins, maintenant_ms()) else { continue };
            // Marquées tout de suite : même en cas d'erreur, la photo n'est pas retraitée en boucle
            // (le lot peut être relancé depuis l'écran Photos de cours).
            for (p, taille, _) in pretes.iter().filter(|(p, _, _)| groupe.contains(p)) {
                let _ = conn.execute(
                    "INSERT OR IGNORE INTO photos_vues(chemin, taille, date, lot_id) VALUES (?1, ?2, ?3, ?4)",
                    params![p.to_string_lossy(), *taille as i64, maintenant_ms(), id],
                );
            }
            id
        };
        let resultat = traiter_lot(etat.inner(), id).await;
        if let Ok(lot) = resultat {
            let _ = app.emit("photos-lot", lot);
        }
    }
    etat.photos.lock().unwrap().reperees.clear();
    noter(app, |e| {
        e.en_cours = false;
        e.en_attente = 0;
        e.message = None;
    });
}

/// État courant de la surveillance (pour l'écran Photos de cours).
pub fn etat_surveillance(etat: &Etat) -> EtatSurveillance {
    let mut e = etat.photos.lock().unwrap().etat.clone();
    let dossier = etat.reglages.lock().unwrap().dossier_photos.clone();
    if e.dossier != dossier {
        e = EtatSurveillance { dossier, ..Default::default() };
    }
    e
}

pub type Suivi = Mutex<SuiviPhotos>;

#[cfg(test)]
mod tests {
    use super::*;
    use crate::db;

    #[test]
    fn photos_regroupees_par_cours() {
        let p = |n: &str, minute: i64| (PathBuf::from(n), 1, minute * 60_000);
        let photos = vec![p("a", 0), p("b", 2), p("c", 5), p("d", 40), p("e", 41)];
        let lots = regrouper(&photos);
        assert_eq!(lots.len(), 2);
        assert_eq!(lots[0].len(), 3);
        let beaucoup: Vec<_> = (0..23).map(|i| p(&format!("p{i}"), i)).collect();
        assert_eq!(regrouper(&beaucoup).iter().map(Vec::len).collect::<Vec<_>>(), vec![10, 10, 3]);
    }

    #[test]
    fn photos_presentes_ignorees_puis_nouvelles_reperees() {
        let dossier = std::env::temp_dir().join(format!("revisio-dossier-photos-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&dossier);
        std::fs::create_dir_all(dossier.join("sous-dossier")).unwrap();
        std::fs::write(dossier.join("IMG_1.jpg"), b"x").unwrap();
        std::fs::write(dossier.join("notes.txt"), b"x").unwrap();
        std::fs::write(dossier.join("sous-dossier").join("IMG_9.jpg"), b"x").unwrap();
        let conn = db::ouvrir_en_memoire().unwrap();
        assert_eq!(nombre_nouvelles(&conn, &dossier), 1);
        assert_eq!(ignorer_photos_presentes(&conn, &dossier).unwrap(), 1);
        assert_eq!(nombre_nouvelles(&conn, &dossier), 0);
        std::fs::write(dossier.join("IMG_2.HEIC"), b"x").unwrap();
        assert_eq!(nombre_nouvelles(&conn, &dossier), 1);
        std::fs::remove_dir_all(&dossier).unwrap();
    }

    #[test]
    fn propositions_et_refus() {
        let conn = db::ouvrir_en_memoire().unwrap();
        conn.execute(
            "INSERT INTO propositions_vault(date, lot_id, chemin, avant, apres, raison) VALUES (1, 1, 'a.md', ?1, 'B', 'ajout')",
            [b"\xef\xbb\xbfA".to_vec()],
        )
        .unwrap();
        let p = propositions(&conn).unwrap();
        assert_eq!((p.len(), p[0].avant.as_str(), p[0].apres.as_str()), (1, "A", "B"));
        refuser(&conn, p[0].id).unwrap();
        assert!(propositions(&conn).unwrap().is_empty());
    }
}
