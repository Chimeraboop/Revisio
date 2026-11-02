//! Synchronisation avec un autre appareil (Révisio sur téléphone, en web-app).
//!
//! Le PC publie un **contenu** : cartes et leur état FSRS, révisions récentes, carnet d'erreurs,
//! évaluations, réglages des séances et texte des notes. Le téléphone révise hors ligne et publie
//! son **journal** : révisions faites, nouveaux états des cartes, erreurs, « j'ai compris »,
//! cartes signalées, notes modifiées. Chaque appareil n'écrit que son propre fichier ; le PC
//! fusionne les journaux des autres :
//! - révisions et erreurs ont un identifiant unique (`uid`) : on fait l'union, sans doublon ;
//! - pour l'état d'une carte, le plus récent gagne (`etat_maj`) — l'état FSRS contient une part
//!   de hasard et certaines cartes n'ont pas d'historique, on ne peut donc pas le recalculer ;
//! - une note modifiée sur le téléphone est écrite (historique, annulable) si elle n'a pas changé
//!   sur le PC entre-temps, sinon la modification est proposée à l'élève (comme celles de l'IA).

use std::collections::hash_map::RandomState;
use std::hash::{BuildHasher, Hasher};
use std::path::Path;

use rusqlite::{params, Connection, OptionalExtension};
use serde::{Deserialize, Serialize};
use serde_json::Value;

use crate::settings::Reglages;
use crate::{db, ecriture, planning, vault};

pub const VERSION_FORMAT: u32 = 1;
/// Révisions envoyées au téléphone (série, activité, durée moyenne) : celles des 400 derniers jours.
const JOURS_REVISIONS: i64 = 400;
/// Notes plus longues : pas envoyées (le téléphone n'a pas besoin de dessins énormes).
const TAILLE_MAX_NOTE: usize = 300_000;

/// Identifiant de cet appareil (créé une fois, gardé dans la base).
pub fn appareil(conn: &Connection) -> String {
    if let Some(id) = db::lire_meta(conn, "appareil") {
        return id;
    }
    let mut h = RandomState::new().build_hasher();
    h.write_i64(crate::maintenant_ms());
    let id = format!("pc-{:016x}", h.finish());
    let _ = db::ecrire_meta(conn, "appareil", &id);
    id
}

/// Donne un identifiant unique aux révisions et erreurs créées sur ce PC qui n'en ont pas encore.
pub fn assurer_uids(conn: &Connection, appareil: &str) -> rusqlite::Result<()> {
    conn.execute("UPDATE revisions SET uid = ?1 || ':r' || id WHERE uid IS NULL", [appareil])?;
    conn.execute("UPDATE erreurs SET uid = ?1 || ':e' || id WHERE uid IS NULL", [appareil])?;
    Ok(())
}

/// Empreinte d'un fichier (FNV-1a sur 64 bits + longueur) : sert à savoir si une note a changé.
pub fn empreinte(octets: &[u8]) -> String {
    let mut h: u64 = 0xcbf2_9ce4_8422_2325;
    for o in octets {
        h ^= *o as u64;
        h = h.wrapping_mul(0x0000_0100_0000_01b3);
    }
    format!("{:016x}-{}", h, octets.len())
}

// ---------- Contenu publié par le PC ----------

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct Contenu {
    pub version: u32,
    pub appareil: String,
    pub genere_le: i64,
    pub reglages: ReglagesSynchro,
    pub cartes: Vec<CarteSynchro>,
    pub revisions: Vec<RevisionSynchro>,
    pub erreurs: Vec<ErreurSynchro>,
    pub evaluations: Vec<planning::Evaluation>,
    pub notes: Vec<NoteSynchro>,
}

/// Réglages utiles aux séances sur le téléphone.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct ReglagesSynchro {
    pub nouvelles_par_jour: usize,
    pub nouvelles_par_jour_langues: usize,
    pub duree_seance: u32,
    pub duree_seance_langues: u32,
    pub entrelacement: bool,
    pub vocab_vers_francais: String,
    pub vocab_depuis_francais: String,
    pub sens_vocabulaire: String,
    pub matieres_exclues: Vec<String>,
    /// Faux : le téléphone ne propose pas de modifier les notes.
    pub ecriture_autorisee: bool,
}

impl From<&Reglages> for ReglagesSynchro {
    fn from(r: &Reglages) -> Self {
        Self {
            nouvelles_par_jour: r.nouvelles_par_jour,
            nouvelles_par_jour_langues: r.nouvelles_par_jour_langues,
            duree_seance: r.duree_seance,
            duree_seance_langues: r.duree_seance_langues,
            entrelacement: r.entrelacement,
            vocab_vers_francais: r.vocab_vers_francais.clone(),
            vocab_depuis_francais: r.vocab_depuis_francais.clone(),
            sens_vocabulaire: r.sens_vocabulaire.clone(),
            matieres_exclues: r.matieres_exclues.clone(),
            ecriture_autorisee: r.ecriture_autorisee,
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct CarteSynchro {
    pub cle: String,
    pub genre: String,
    pub note_chemin: String,
    pub matiere: Option<String>,
    pub chapitre: Option<String>,
    pub section: String,
    pub question: String,
    pub reponse: String,
    pub donnees: Value,
    pub extrait: String,
    pub suspendue: bool,
    pub creee_le: i64,
    pub etat_fsrs: Option<String>,
    pub echeance: Option<i64>,
    pub introduite_le: Option<i64>,
    /// Date du dernier changement d'état (0 : jamais révisée).
    pub etat_maj: i64,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct RevisionSynchro {
    pub uid: String,
    pub cle: String,
    pub date: i64,
    pub note: i64,
    pub confiance: String,
    pub correcte: bool,
    #[serde(default)]
    pub reponse_donnee: Option<String>,
    #[serde(default)]
    pub duree_ms: i64,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct ErreurSynchro {
    pub uid: String,
    pub cle: Option<String>,
    pub date: i64,
    pub genre: String,
    pub matiere: Option<String>,
    pub chapitre: Option<String>,
    pub note_chemin: String,
    pub section: String,
    pub question: String,
    pub reponse_attendue: String,
    pub reponse_donnee: Option<String>,
    pub extrait: String,
    pub comprise: bool,
    pub comprise_le: i64,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct NoteSynchro {
    pub chemin: String,
    pub titre: String,
    pub matiere: Option<String>,
    pub chapitre: Option<String>,
    pub genre: String,
    pub contenu: String,
    /// Empreinte du fichier : le téléphone la renvoie avec une note modifiée.
    pub empreinte: String,
}

/// Tout ce que le téléphone doit savoir. `racine` absente (aucun dossier de notes) : pas de notes.
pub fn exporter(conn: &Connection, reglages: &Reglages, racine: Option<&Path>, maintenant: i64) -> Result<Contenu, String> {
    let e = |e: rusqlite::Error| e.to_string();
    let id = appareil(conn);
    assurer_uids(conn, &id).map_err(e)?;

    let cartes = conn
        .prepare(
            "SELECT cle, genre, note_chemin, matiere, chapitre, section, question, reponse, donnees, extrait,
                    suspendue, creee_le, etat_fsrs, echeance, introduite_le, etat_maj
             FROM cartes WHERE active = 1 ORDER BY id",
        )
        .map_err(e)?
        .query_map([], |r| {
            let donnees: String = r.get(8)?;
            Ok(CarteSynchro {
                cle: r.get(0)?,
                genre: r.get(1)?,
                note_chemin: r.get(2)?,
                matiere: r.get(3)?,
                chapitre: r.get(4)?,
                section: r.get(5)?,
                question: r.get(6)?,
                reponse: r.get(7)?,
                donnees: serde_json::from_str(&donnees).unwrap_or(Value::Object(Default::default())),
                extrait: r.get(9)?,
                suspendue: r.get(10)?,
                creee_le: r.get(11)?,
                etat_fsrs: r.get(12)?,
                echeance: r.get(13)?,
                introduite_le: r.get(14)?,
                etat_maj: r.get(15)?,
            })
        })
        .map_err(e)?
        .collect::<rusqlite::Result<Vec<_>>>()
        .map_err(e)?;

    let depuis = maintenant - JOURS_REVISIONS * 24 * 3600 * 1000;
    let revisions = conn
        .prepare(
            "SELECT r.uid, c.cle, r.date, r.note, r.confiance, r.correcte, r.reponse_donnee, r.duree_ms
             FROM revisions r JOIN cartes c ON c.id = r.carte_id WHERE r.date >= ?1 ORDER BY r.date",
        )
        .map_err(e)?
        .query_map([depuis], |r| {
            Ok(RevisionSynchro {
                uid: r.get(0)?,
                cle: r.get(1)?,
                date: r.get(2)?,
                note: r.get(3)?,
                confiance: r.get(4)?,
                correcte: r.get(5)?,
                reponse_donnee: r.get(6)?,
                duree_ms: r.get(7)?,
            })
        })
        .map_err(e)?
        .collect::<rusqlite::Result<Vec<_>>>()
        .map_err(e)?;

    let erreurs = conn
        .prepare(
            "SELECT e.uid, c.cle, e.date, e.genre, e.matiere, e.chapitre, e.note_chemin, e.section, e.question,
                    e.reponse_attendue, e.reponse_donnee, e.extrait, e.comprise, e.comprise_le
             FROM erreurs e LEFT JOIN cartes c ON c.id = e.carte_id ORDER BY e.date",
        )
        .map_err(e)?
        .query_map([], |r| {
            Ok(ErreurSynchro {
                uid: r.get(0)?,
                cle: r.get(1)?,
                date: r.get(2)?,
                genre: r.get(3)?,
                matiere: r.get(4)?,
                chapitre: r.get(5)?,
                note_chemin: r.get(6)?,
                section: r.get(7)?,
                question: r.get(8)?,
                reponse_attendue: r.get(9)?,
                reponse_donnee: r.get(10)?,
                extrait: r.get(11)?,
                comprise: r.get(12)?,
                comprise_le: r.get(13)?,
            })
        })
        .map_err(e)?
        .collect::<rusqlite::Result<Vec<_>>>()
        .map_err(e)?;

    let mut notes = Vec::new();
    if let Some(racine) = racine {
        let liste: Vec<(String, String, Option<String>, Option<String>, String)> = conn
            .prepare("SELECT chemin, titre, matiere, chapitre, genre FROM notes WHERE genre != 'schema' ORDER BY chemin")
            .map_err(e)?
            .query_map([], |r| Ok((r.get(0)?, r.get(1)?, r.get(2)?, r.get(3)?, r.get(4)?)))
            .map_err(e)?
            .collect::<rusqlite::Result<_>>()
            .map_err(e)?;
        for (chemin, titre, matiere, chapitre, genre) in liste {
            let Ok(octets) = vault::lire_octets(racine, &chemin) else { continue };
            if octets.len() > TAILLE_MAX_NOTE {
                continue;
            }
            let Ok(contenu) = String::from_utf8(octets.clone()) else { continue };
            notes.push(NoteSynchro { chemin, titre, matiere, chapitre, genre, contenu, empreinte: empreinte(&octets) });
        }
    }

    Ok(Contenu {
        version: VERSION_FORMAT,
        appareil: id,
        genere_le: maintenant,
        reglages: reglages.into(),
        cartes,
        revisions,
        erreurs,
        evaluations: planning::lister(conn, "0000-01-01").map_err(e)?,
        notes,
    })
}

// ---------- Journal publié par un autre appareil ----------

#[derive(Debug, Clone, Default, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "camelCase", default)]
pub struct Journal {
    pub version: u32,
    pub appareil: String,
    /// Nom affiché (« iPhone »).
    pub nom: String,
    pub revisions: Vec<RevisionSynchro>,
    pub etats: Vec<EtatSynchro>,
    pub erreurs: Vec<ErreurNouvelle>,
    pub comprises: Vec<Comprise>,
    pub suspendues: Vec<String>,
    pub notes: Vec<NoteModifiee>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct EtatSynchro {
    pub cle: String,
    pub etat_fsrs: String,
    pub echeance: i64,
    #[serde(default)]
    pub introduite_le: Option<i64>,
    pub maj: i64,
}

/// Réponse fausse donnée sur l'autre appareil (le reste de l'erreur vient de la carte).
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct ErreurNouvelle {
    pub uid: String,
    pub cle: String,
    pub date: i64,
    #[serde(default)]
    pub reponse_donnee: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct Comprise {
    pub uid: String,
    pub comprise: bool,
    pub date: i64,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct NoteModifiee {
    pub chemin: String,
    /// Empreinte de la version à partir de laquelle la note a été modifiée.
    pub base: String,
    pub contenu: String,
    pub date: i64,
}

/// Ce que la fusion a changé.
#[derive(Debug, Clone, Default, Serialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct Bilan {
    pub revisions: usize,
    pub etats: usize,
    pub erreurs: usize,
    pub comprises: usize,
    pub suspendues: usize,
    pub notes_ecrites: usize,
    /// Notes modifiées des deux côtés : proposées à l'élève.
    pub notes_a_valider: usize,
    /// Notes modifiées qui attendent que l'écriture soit autorisée.
    pub notes_en_attente: usize,
    /// Cartes inconnues ici (note supprimée entre-temps…).
    pub ignorees: usize,
}

/// Clé actuelle d'une carte (une note renommée a changé la clé de ses cartes).
fn cle_actuelle(conn: &Connection, cle: &str) -> rusqlite::Result<String> {
    let mut cle = cle.to_string();
    for _ in 0..10 {
        match conn
            .query_row("SELECT nouvelle FROM cles_renommees WHERE ancienne = ?1", [&cle], |r| r.get::<_, String>(0))
            .optional()?
        {
            Some(n) => cle = n,
            None => break,
        }
    }
    Ok(cle)
}

fn carte_id(conn: &Connection, cle: &str) -> rusqlite::Result<Option<i64>> {
    let cle = cle_actuelle(conn, cle)?;
    conn.query_row("SELECT id FROM cartes WHERE cle = ?1", [cle], |r| r.get(0)).optional()
}

/// Fusionne le journal d'un autre appareil. Les notes modifiées ne sont écrites que si `ecriture` est donné
/// (dossier de notes choisi et écriture autorisée) ; sinon elles attendent la prochaine synchronisation.
pub fn importer(conn: &mut Connection, journal: &Journal, ecriture: Option<&ecriture::Contexte>) -> Result<Bilan, String> {
    if journal.version > VERSION_FORMAT {
        return Err("Ce téléphone utilise une version plus récente de Révisio : mets Révisio à jour sur le PC.".into());
    }
    let e = |e: rusqlite::Error| e.to_string();
    let mut bilan = Bilan::default();
    let tx = conn.transaction().map_err(e)?;

    for r in &journal.revisions {
        let Some(id) = carte_id(&tx, &r.cle).map_err(e)? else {
            bilan.ignorees += 1;
            continue;
        };
        bilan.revisions += tx
            .execute(
                "INSERT OR IGNORE INTO revisions(uid, carte_id, date, note, confiance, correcte, reponse_donnee, duree_ms)
                 VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8)",
                params![r.uid, id, r.date, r.note.clamp(1, 4), r.confiance, r.correcte, r.reponse_donnee, r.duree_ms.max(0)],
            )
            .map_err(e)?;
    }
    for s in &journal.etats {
        let Some(id) = carte_id(&tx, &s.cle).map_err(e)? else { continue };
        bilan.etats += tx
            .execute(
                "UPDATE cartes SET etat_fsrs = ?2, echeance = ?3, introduite_le = coalesce(introduite_le, ?4), etat_maj = ?5
                 WHERE id = ?1 AND etat_maj < ?5",
                params![id, s.etat_fsrs, s.echeance, s.introduite_le.unwrap_or(s.maj), s.maj],
            )
            .map_err(e)?;
    }
    for x in &journal.erreurs {
        let Some(id) = carte_id(&tx, &x.cle).map_err(e)? else { continue };
        bilan.erreurs += tx
            .execute(
                "INSERT OR IGNORE INTO erreurs(uid, carte_id, date, genre, matiere, chapitre, note_chemin, section,
                                               question, reponse_attendue, reponse_donnee, extrait)
                 SELECT ?1, id, ?3, genre, matiere, chapitre, note_chemin, section, question, reponse, ?4, extrait
                 FROM cartes WHERE id = ?2",
                params![x.uid, id, x.date, x.reponse_donnee],
            )
            .map_err(e)?;
    }
    for c in &journal.comprises {
        bilan.comprises += tx
            .execute(
                "UPDATE erreurs SET comprise = ?2, comprise_le = ?3 WHERE uid = ?1 AND comprise_le < ?3",
                params![c.uid, c.comprise, c.date],
            )
            .map_err(e)?;
    }
    for cle in &journal.suspendues {
        let Some(id) = carte_id(&tx, cle).map_err(e)? else { continue };
        bilan.suspendues += tx.execute("UPDATE cartes SET suspendue = 1 WHERE id = ?1 AND suspendue = 0", [id]).map_err(e)?;
    }
    tx.commit().map_err(e)?;

    for n in &journal.notes {
        let id = format!("{}|{}|{}", journal.appareil, n.chemin, n.date);
        let deja: bool = conn
            .query_row("SELECT 1 FROM synchro_appliques WHERE id = ?1", [&id], |_| Ok(true))
            .optional()
            .map_err(e)?
            .is_some();
        if deja {
            continue;
        }
        let Some(ctx) = ecriture.filter(|c| c.autorisee) else {
            bilan.notes_en_attente += 1;
            continue;
        };
        let actuel = vault::lire_octets(ctx.racine, &n.chemin).ok();
        if actuel.as_deref() == Some(n.contenu.as_bytes()) {
            // Déjà identique (synchronisation précédente interrompue après l'écriture).
        } else if actuel.as_ref().map(|o| empreinte(o)) == Some(n.base.clone()) {
            ecriture::modifier(conn, ctx, &n.chemin, &n.contenu, &|o| empreinte(o) == n.base)?;
            bilan.notes_ecrites += 1;
        } else {
            conn.execute(
                "INSERT INTO propositions_vault(date, lot_id, chemin, avant, apres, raison) VALUES (?1, NULL, ?2, ?3, ?4, ?5)",
                params![
                    ctx.maintenant,
                    n.chemin,
                    actuel.unwrap_or_default(),
                    n.contenu,
                    format!(
                        "Modifiée sur {} alors qu'elle avait aussi changé sur ce PC : vérifie avant d'accepter.",
                        if journal.nom.is_empty() { "ton téléphone" } else { journal.nom.as_str() }
                    )
                ],
            )
            .map_err(e)?;
            bilan.notes_a_valider += 1;
        }
        conn.execute("INSERT INTO synchro_appliques(id, date) VALUES (?1, ?2)", params![id, ctx.maintenant]).map_err(e)?;
    }
    Ok(bilan)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn base() -> Connection {
        let conn = db::ouvrir_en_memoire().unwrap();
        conn.execute_batch(
            "INSERT INTO cartes(id, cle, genre, note_chemin, matiere, chapitre, question, reponse, creee_le, etat_fsrs, echeance, etat_maj)
               VALUES (1, 'trou|SVT/Méiose.md|1', 'trou', 'SVT/Méiose.md', 'SVT', 'Méiose', 'q1', 'r1', 0, '{\"s\":1}', 1000, 500),
                      (2, 'trou|SVT/Méiose.md|2', 'trou', 'SVT/Méiose.md', 'SVT', 'Méiose', 'q2', 'r2', 0, NULL, NULL, 0);
             INSERT INTO revisions(carte_id, date, note, confiance, correcte) VALUES (1, 500, 3, 'sur', 1);",
        )
        .unwrap();
        conn
    }

    fn journal() -> Journal {
        Journal { version: 1, appareil: "iphone-1".into(), nom: "iPhone".into(), ..Default::default() }
    }

    #[test]
    fn empreinte_stable() {
        assert_eq!(empreinte(b"abc"), empreinte(b"abc"));
        assert_ne!(empreinte(b"abc"), empreinte(b"abd"));
        assert!(empreinte(b"").ends_with("-0"));
    }

    #[test]
    fn export_avec_uids_et_etats() {
        let conn = base();
        let c = exporter(&conn, &Reglages::default(), None, 1000).unwrap();
        assert_eq!(c.cartes.len(), 2);
        assert_eq!(c.cartes[0].etat_maj, 500);
        assert_eq!(c.revisions.len(), 1);
        assert!(c.revisions[0].uid.starts_with("pc-"));
        assert_eq!(c.revisions[0].cle, "trou|SVT/Méiose.md|1");
        // Même appareil d'une fois sur l'autre, uids inchangés.
        let c2 = exporter(&conn, &Reglages::default(), None, 1000).unwrap();
        assert_eq!(c.appareil, c2.appareil);
        assert_eq!(c.revisions[0].uid, c2.revisions[0].uid);
    }

    #[test]
    fn fusion_sans_doublon_et_etat_le_plus_recent() {
        let mut conn = base();
        let mut j = journal();
        j.revisions = vec![
            RevisionSynchro { uid: "iphone-1:1".into(), cle: "trou|SVT/Méiose.md|2".into(), date: 900, note: 1, confiance: "hesitant".into(), correcte: false, reponse_donnee: Some("x".into()), duree_ms: 4000 },
            RevisionSynchro { uid: "iphone-1:2".into(), cle: "inconnue".into(), date: 900, note: 3, confiance: "sur".into(), correcte: true, reponse_donnee: None, duree_ms: 0 },
        ];
        j.etats = vec![
            // Plus récent que l'état du PC : appliqué.
            EtatSynchro { cle: "trou|SVT/Méiose.md|2".into(), etat_fsrs: "{\"t\":2}".into(), echeance: 5000, introduite_le: Some(900), maj: 900 },
            // Plus ancien que l'état du PC (révisée sur le PC depuis) : ignoré.
            EtatSynchro { cle: "trou|SVT/Méiose.md|1".into(), etat_fsrs: "{\"vieux\":1}".into(), echeance: 2, introduite_le: None, maj: 400 },
        ];
        j.erreurs = vec![ErreurNouvelle { uid: "iphone-1:e1".into(), cle: "trou|SVT/Méiose.md|2".into(), date: 900, reponse_donnee: Some("x".into()) }];
        j.suspendues = vec!["trou|SVT/Méiose.md|1".into()];
        let b = importer(&mut conn, &j, None).unwrap();
        assert_eq!((b.revisions, b.etats, b.erreurs, b.suspendues, b.ignorees), (1, 1, 1, 1, 1));
        let (etat, maj, intro): (String, i64, i64) = conn
            .query_row("SELECT etat_fsrs, etat_maj, introduite_le FROM cartes WHERE id = 2", [], |r| Ok((r.get(0)?, r.get(1)?, r.get(2)?)))
            .unwrap();
        assert_eq!((etat.as_str(), maj, intro), ("{\"t\":2}", 900, 900));
        let vieux: String = conn.query_row("SELECT etat_fsrs FROM cartes WHERE id = 1", [], |r| r.get(0)).unwrap();
        assert_eq!(vieux, "{\"s\":1}");
        let (question, reponse): (String, String) =
            conn.query_row("SELECT question, reponse_donnee FROM erreurs WHERE uid = 'iphone-1:e1'", [], |r| Ok((r.get(0)?, r.get(1)?))).unwrap();
        assert_eq!((question.as_str(), reponse.as_str()), ("q2", "x"));
        // Deuxième import du même journal : rien ne change.
        let b2 = importer(&mut conn, &j, None).unwrap();
        assert_eq!((b2.revisions, b2.etats, b2.erreurs, b2.suspendues), (0, 0, 0, 0));
        // « J'ai compris » sur le téléphone, plus récent que sur le PC.
        j.comprises = vec![Comprise { uid: "iphone-1:e1".into(), comprise: true, date: 950 }];
        assert_eq!(importer(&mut conn, &j, None).unwrap().comprises, 1);
        assert_eq!(importer(&mut conn, &j, None).unwrap().comprises, 0);
    }

    #[test]
    fn revision_sous_une_ancienne_cle() {
        let mut conn = base();
        crate::renommage::migrer_chemins(&conn, "SVT/Méiose.md", "SVT/La méiose.md").unwrap();
        let nouvelle: String = conn.query_row("SELECT cle FROM cartes WHERE id = 2", [], |r| r.get(0)).unwrap();
        assert_eq!(nouvelle, "trou|SVT/La méiose.md|2");
        let mut j = journal();
        j.revisions = vec![RevisionSynchro { uid: "iphone-1:9".into(), cle: "trou|SVT/Méiose.md|2".into(), date: 900, note: 3, confiance: "sur".into(), correcte: true, reponse_donnee: None, duree_ms: 0 }];
        assert_eq!(importer(&mut conn, &j, None).unwrap().revisions, 1);
    }

    #[test]
    fn notes_modifiees_ecrites_ou_proposees() {
        let racine = std::env::temp_dir().join(format!("revisio-synchro-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&racine);
        std::fs::create_dir_all(racine.join("SVT")).unwrap();
        std::fs::write(racine.join("SVT/A.md"), "avant A").unwrap();
        std::fs::write(racine.join("SVT/B.md"), "avant B").unwrap();
        let mut conn = base();
        let ctx = ecriture::Contexte {
            racine: &racine,
            autorisee: true,
            origine: "appareil",
            lot_id: None,
            raison: None,
            maintenant: 2000,
            groupe: None,
        };
        let mut j = journal();
        j.notes = vec![
            NoteModifiee { chemin: "SVT/A.md".into(), base: empreinte(b"avant A"), contenu: "après A".into(), date: 10 },
            // B a changé sur le PC depuis la version modifiée sur le téléphone.
            NoteModifiee { chemin: "SVT/B.md".into(), base: empreinte(b"vieux B"), contenu: "après B".into(), date: 11 },
        ];
        // Écriture désactivée : rien n'est fait, tout attend.
        let sans = ecriture::Contexte { autorisee: false, ..ctx };
        assert_eq!(importer(&mut conn, &j, Some(&sans)).unwrap().notes_en_attente, 2);
        let b = importer(&mut conn, &j, Some(&ctx)).unwrap();
        assert_eq!((b.notes_ecrites, b.notes_a_valider), (1, 1));
        assert_eq!(std::fs::read_to_string(racine.join("SVT/A.md")).unwrap(), "après A");
        assert_eq!(std::fs::read_to_string(racine.join("SVT/B.md")).unwrap(), "avant B");
        let n: i64 = conn.query_row("SELECT count(*) FROM propositions_vault WHERE chemin = 'SVT/B.md'", [], |r| r.get(0)).unwrap();
        assert_eq!(n, 1);
        // Déjà traitées : la synchronisation suivante n'y touche plus.
        let b2 = importer(&mut conn, &j, Some(&ctx)).unwrap();
        assert_eq!((b2.notes_ecrites, b2.notes_a_valider), (0, 0));
        let _ = std::fs::remove_dir_all(&racine);
    }
}
