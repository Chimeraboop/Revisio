//! Synchronisation entre ordinateurs, par le même dossier caché du Google Drive que le téléphone.
//!
//! Chaque PC publie son propre journal (`journal-<appareil>.json.gz`, champ `pc`), qui contient TOUT ce qu'il
//! sait : révisions, état des cartes, erreurs, cartes rédigées par l'IA, notes déjà traitées par l'IA,
//! évaluations (et celles supprimées), et le texte de chaque note. Les autres PC le fusionnent comme celui
//! du téléphone (union par identifiant, le plus récent gagne). Comme en Python avec des `set` : refaire la
//! fusion une deuxième fois ne change rien.
//!
//! Les dessins Excalidraw (`.excalidraw.md`) voyagent aussi, copiés à l'octet près (`ecriture::creer_copie`) :
//! Révisio ne les rédige ni ne les retouche jamais.
//!
//! Les notes sont fusionnées « à trois versions » : pour chaque autre PC et chaque note, on retient la
//! dernière version que les deux avaient en commun (la base). Si seul l'autre PC a changé la note, on prend sa
//! version ; si seul ce PC l'a changée, on ne fait rien (l'autre prendra la nôtre) ; si les deux l'ont changée,
//! la version de l'autre PC est proposée à l'élève. Une note supprimée là-bas n'est supprimée ici que si elle
//! n'a pas changé ici, et jamais plus de `SUPPRESSIONS_MAX` d'un coup (dossier de notes débranché ?). Toutes
//! les écritures passent par `ecriture.rs` : historique, corbeille, annulation.

use std::collections::HashSet;
use std::path::Path;

use rusqlite::{params, Connection, OptionalExtension};
use serde::{Deserialize, Serialize};

use crate::synchro::{empreinte, lire_cartes, Bilan, Comprise, ErreurNouvelle, EtatSynchro, Journal, RevisionSynchro, VERSION_FORMAT};
use crate::{ecriture, planning, settings, vault};

/// Au-delà, des suppressions venues d'un autre PC ne sont pas appliquées (l'élève est prévenu).
const SUPPRESSIONS_MAX: usize = 20;
/// Une note plus grosse n'est pas recopiée (le PC dit seulement qu'elle existe).
const TAILLE_MAX_NOTE: usize = 300_000;

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct NoteIa {
    pub chemin: String,
    pub statut: String,
    pub taille: i64,
    pub cartes: i64,
    pub maj: i64,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct EvaluationSynchro {
    pub uid: String,
    pub jour: String,
    pub matiere: String,
    pub titre: String,
    pub genre: String,
    pub chapitres: Vec<Option<String>>,
    pub maj: i64,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct Suppression {
    pub uid: String,
    pub date: i64,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct FichierNote {
    pub chemin: String,
    pub empreinte: String,
    /// Absent pour une note trop grosse ou illisible : elle existe, mais n'est pas recopiée.
    #[serde(default)]
    pub contenu: Option<String>,
}

/// Nom de cet ordinateur, pour les messages (« Modifiée sur l'ordinateur … »).
fn nom_ordinateur() -> String {
    std::env::var("COMPUTERNAME")
        .or_else(|_| std::env::var("HOSTNAME"))
        .map(|n| format!("l'ordinateur « {n} »"))
        .unwrap_or_else(|_| "ton autre ordinateur".into())
}

/// Journal publié par ce PC pour les autres ordinateurs.
pub fn journal(conn: &Connection, racine: Option<&Path>) -> Result<Journal, String> {
    let e = |e: rusqlite::Error| e.to_string();
    let appareil = crate::synchro::appareil(conn);
    crate::synchro::assurer_uids(conn, &appareil).map_err(e)?;

    let revisions = conn
        .prepare(
            "SELECT r.uid, c.cle, r.date, r.note, r.confiance, r.correcte, r.reponse_donnee, r.duree_ms
             FROM revisions r JOIN cartes c ON c.id = r.carte_id WHERE r.uid IS NOT NULL ORDER BY r.date",
        )
        .map_err(e)?
        .query_map([], |r| {
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
    let etats = conn
        .prepare(
            "SELECT cle, etat_fsrs, echeance, introduite_le, etat_maj FROM cartes
             WHERE active = 1 AND etat_fsrs IS NOT NULL AND echeance IS NOT NULL",
        )
        .map_err(e)?
        .query_map([], |r| Ok(EtatSynchro { cle: r.get(0)?, etat_fsrs: r.get(1)?, echeance: r.get(2)?, introduite_le: r.get(3)?, maj: r.get(4)? }))
        .map_err(e)?
        .collect::<rusqlite::Result<Vec<_>>>()
        .map_err(e)?;
    let erreurs = conn
        .prepare("SELECT e.uid, c.cle, e.date, e.reponse_donnee FROM erreurs e JOIN cartes c ON c.id = e.carte_id WHERE e.uid IS NOT NULL")
        .map_err(e)?
        .query_map([], |r| Ok(ErreurNouvelle { uid: r.get(0)?, cle: r.get(1)?, date: r.get(2)?, reponse_donnee: r.get(3)? }))
        .map_err(e)?
        .collect::<rusqlite::Result<Vec<_>>>()
        .map_err(e)?;
    let comprises = conn
        .prepare("SELECT uid, comprise, comprise_le FROM erreurs WHERE uid IS NOT NULL AND comprise_le > 0")
        .map_err(e)?
        .query_map([], |r| Ok(Comprise { uid: r.get(0)?, comprise: r.get(1)?, date: r.get(2)? }))
        .map_err(e)?
        .collect::<rusqlite::Result<Vec<_>>>()
        .map_err(e)?;
    let suspendues = conn
        .prepare("SELECT cle FROM cartes WHERE active = 1 AND suspendue = 1")
        .map_err(e)?
        .query_map([], |r| r.get(0))
        .map_err(e)?
        .collect::<rusqlite::Result<Vec<String>>>()
        .map_err(e)?;
    let notes_ia = conn
        .prepare("SELECT chemin, statut, taille, cartes, maj FROM cartes_ia_notes WHERE statut IN ('faite', 'vide')")
        .map_err(e)?
        .query_map([], |r| Ok(NoteIa { chemin: r.get(0)?, statut: r.get(1)?, taille: r.get(2)?, cartes: r.get(3)?, maj: r.get(4)? }))
        .map_err(e)?
        .collect::<rusqlite::Result<Vec<_>>>()
        .map_err(e)?;
    let evaluations = conn
        .prepare("SELECT uid, jour, matiere, titre, genre, chapitres, maj FROM evaluations WHERE uid IS NOT NULL")
        .map_err(e)?
        .query_map([], |r| {
            Ok(EvaluationSynchro {
                uid: r.get(0)?,
                jour: r.get(1)?,
                matiere: r.get(2)?,
                titre: r.get(3)?,
                genre: r.get(4)?,
                chapitres: serde_json::from_str(&r.get::<_, String>(5)?).unwrap_or_default(),
                maj: r.get(6)?,
            })
        })
        .map_err(e)?
        .collect::<rusqlite::Result<Vec<_>>>()
        .map_err(e)?;
    let evaluations_supprimees = conn
        .prepare("SELECT uid, date FROM evaluations_supprimees")
        .map_err(e)?
        .query_map([], |r| Ok(Suppression { uid: r.get(0)?, date: r.get(1)? }))
        .map_err(e)?
        .collect::<rusqlite::Result<Vec<_>>>()
        .map_err(e)?;

    let mut fichiers = Vec::new();
    let mut fichiers_complets = false;
    if let Some(racine) = racine.filter(|r| r.is_dir()) {
        let chemins: Vec<String> = conn
            .prepare("SELECT chemin FROM notes ORDER BY chemin")
            .map_err(e)?
            .query_map([], |r| r.get(0))
            .map_err(e)?
            .collect::<rusqlite::Result<_>>()
            .map_err(e)?;
        for chemin in chemins {
            // Notes et dessins Excalidraw (copiés tels quels) ; pas les modèles ni les dossiers réservés.
            if ecriture::chemin_synchronisable(racine, &chemin).is_err() {
                continue;
            }
            match vault::lire_octets(racine, &chemin) {
                Ok(octets) => {
                    let contenu = (octets.len() <= TAILLE_MAX_NOTE).then(|| String::from_utf8(octets.clone()).ok()).flatten();
                    fichiers.push(FichierNote { chemin, empreinte: empreinte(&octets), contenu });
                }
                // Indexée mais introuvable (supprimée à l'instant ?) : on ne dit rien de plus.
                Err(_) => continue,
            }
        }
        // Un index vide (première indexation pas finie) ne veut pas dire que tout a été supprimé.
        fichiers_complets = !fichiers.is_empty();
    }

    Ok(Journal {
        version: VERSION_FORMAT,
        appareil,
        nom: nom_ordinateur(),
        revisions,
        etats,
        erreurs,
        comprises,
        suspendues,
        notes: Vec::new(),
        pc: true,
        cartes: lire_cartes(conn, "active = 1 AND cle LIKE 'ia|%'").map_err(e)?,
        cartes_retirees: lire_cartes(conn, "active = 0 AND id IN (SELECT carte_id FROM revisions)").map_err(e)?,
        notes_ia,
        evaluations,
        evaluations_supprimees,
        fichiers,
        fichiers_complets,
    })
}

/// Cartes de l'IA, notes déjà traitées par l'IA et évaluations d'un autre PC (dans la transaction de la fusion,
/// avant les révisions : une révision d'une carte de l'IA doit trouver sa carte).
pub fn importer_donnees(tx: &Connection, journal: &Journal, bilan: &mut Bilan) -> rusqlite::Result<()> {
    for c in &journal.cartes {
        if !c.cle.starts_with("ia|") {
            continue;
        }
        bilan.cartes += tx.execute(
            "INSERT OR IGNORE INTO cartes(cle, genre, note_chemin, matiere, chapitre, section, question, reponse, donnees,
                                          extrait, active, suspendue, creee_le)
             VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10, 1, ?11, ?12)",
            params![
                c.cle,
                c.genre,
                c.note_chemin,
                c.matiere,
                c.chapitre,
                c.section,
                c.question,
                c.reponse,
                c.donnees.to_string(),
                c.extrait,
                c.suspendue,
                c.creee_le
            ],
        )?;
    }
    for c in &journal.cartes_retirees {
        tx.execute(
            "INSERT OR IGNORE INTO cartes(cle, genre, note_chemin, matiere, chapitre, section, question, reponse, donnees,
                                          extrait, active, suspendue, creee_le)
             VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10, 0, ?11, ?12)",
            params![
                c.cle,
                c.genre,
                c.note_chemin,
                c.matiere,
                c.chapitre,
                c.section,
                c.question,
                c.reponse,
                c.donnees.to_string(),
                c.extrait,
                c.suspendue,
                c.creee_le
            ],
        )?;
    }
    // Note déjà traitée par l'IA là-bas : ce PC ne la refait pas (quota) et retire ses textes à trous.
    for n in &journal.notes_ia {
        tx.execute(
            "INSERT INTO cartes_ia_notes(chemin, statut, taille, cartes, maj) VALUES (?1, ?2, ?3, ?4, ?5)
             ON CONFLICT(chemin) DO UPDATE SET statut = excluded.statut, taille = excluded.taille,
                 cartes = excluded.cartes, maj = excluded.maj, erreur = NULL
             WHERE cartes_ia_notes.statut = 'erreur'",
            params![n.chemin, n.statut, n.taille, n.cartes, n.maj],
        )?;
    }
    for s in &journal.evaluations_supprimees {
        tx.execute("INSERT OR IGNORE INTO evaluations_supprimees(uid, date) VALUES (?1, ?2)", params![s.uid, s.date])?;
        bilan.evaluations += tx.execute("DELETE FROM evaluations WHERE uid = ?1 AND maj <= ?2", params![s.uid, s.date])?;
    }
    for ev in &journal.evaluations {
        let supprimee = tx
            .query_row("SELECT 1 FROM evaluations_supprimees WHERE uid = ?1", [&ev.uid], |_| Ok(()))
            .optional()?
            .is_some();
        let valide = planning::valider(&planning::Evaluation {
            id: 0,
            jour: ev.jour.clone(),
            matiere: ev.matiere.clone(),
            titre: ev.titre.clone(),
            genre: ev.genre.clone(),
            chapitres: ev.chapitres.clone(),
        })
        .is_ok();
        if supprimee || !valide {
            continue;
        }
        let chapitres = serde_json::to_string(&ev.chapitres).unwrap_or_else(|_| "[]".into());
        let existe = tx.query_row("SELECT maj FROM evaluations WHERE uid = ?1", [&ev.uid], |r| r.get::<_, i64>(0)).optional()?;
        bilan.evaluations += match existe {
            Some(maj) if maj >= ev.maj => 0,
            Some(_) => tx.execute(
                "UPDATE evaluations SET jour = ?2, matiere = ?3, titre = ?4, genre = ?5, chapitres = ?6, maj = ?7 WHERE uid = ?1",
                params![ev.uid, ev.jour, ev.matiere, ev.titre, ev.genre, chapitres, ev.maj],
            )?,
            None => tx.execute(
                "INSERT INTO evaluations(uid, jour, matiere, titre, genre, chapitres, creee_le, maj) VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?7)",
                params![ev.uid, ev.jour, ev.matiere, ev.titre, ev.genre, chapitres, ev.maj],
            )?,
        };
    }
    Ok(())
}

fn base(conn: &Connection, appareil: &str, chemin: &str) -> rusqlite::Result<Option<String>> {
    conn.query_row(
        "SELECT empreinte FROM synchro_notes_base WHERE appareil = ?1 AND chemin = ?2",
        params![appareil, chemin],
        |r| r.get(0),
    )
    .optional()
}

fn poser_base(conn: &Connection, appareil: &str, chemin: &str, emp: &str) -> rusqlite::Result<()> {
    conn.execute(
        "INSERT OR REPLACE INTO synchro_notes_base(appareil, chemin, empreinte) VALUES (?1, ?2, ?3)",
        params![appareil, chemin, emp],
    )?;
    Ok(())
}

fn retirer_base(conn: &Connection, appareil: &str, chemin: &str) -> rusqlite::Result<()> {
    conn.execute("DELETE FROM synchro_notes_base WHERE appareil = ?1 AND chemin = ?2", params![appareil, chemin])?;
    Ok(())
}

/// Les notes d'un autre PC, fusionnées dans le dossier de notes de celui-ci (voir l'en-tête du module).
pub fn fusionner_fichiers(conn: &Connection, journal: &Journal, ecriture: Option<&ecriture::Contexte>, bilan: &mut Bilan) -> Result<(), String> {
    let e = |e: rusqlite::Error| e.to_string();
    let Some(ctx) = ecriture else { return Ok(()) };
    let app = journal.appareil.as_str();
    let lire = |chemin: &str| vault::lire_octets(ctx.racine, chemin).ok();

    if !ctx.autorisee {
        // Rien n'est écrit : on compte seulement ce qui attend, et le journal sera relu plus tard.
        bilan.notes_en_attente +=
            journal.fichiers.iter().filter(|f| f.contenu.is_some() && lire(&f.chemin).map(|o| empreinte(&o)) != Some(f.empreinte.clone())).count();
        return Ok(());
    }
    let raison = format!("Synchronisée depuis {}", if journal.nom.is_empty() { "un autre ordinateur" } else { journal.nom.as_str() });
    let groupe = format!("synchro-{}-{}", app, ctx.maintenant);
    let ctx = ecriture::Contexte { raison: Some(&raison), groupe: Some(&groupe), ..*ctx };

    for f in &journal.fichiers {
        let Some(contenu) = &f.contenu else { continue };
        let local = lire(&f.chemin);
        let l = local.as_deref().map(empreinte);
        if l.as_deref() == Some(f.empreinte.as_str()) {
            poser_base(conn, app, &f.chemin, &f.empreinte).map_err(e)?;
            continue;
        }
        let b = base(conn, app, &f.chemin).map_err(e)?;
        match (local, l) {
            (None, _) => {
                // Supprimée ici depuis la dernière fois, inchangée là-bas : l'autre PC la supprimera à son tour.
                if b.as_deref() == Some(f.empreinte.as_str()) {
                    continue;
                }
                match ecriture::creer_copie(conn, &ctx, &f.chemin, contenu) {
                    Ok(_) => {
                        bilan.notes_recues += 1;
                        poser_base(conn, app, &f.chemin, &f.empreinte).map_err(e)?;
                    }
                    Err(_) => bilan.ignorees += 1,
                }
            }
            (Some(octets), Some(le)) => {
                // Note d'accueil créée par Révisio avec un dossier neuf, jamais retouchée : celle de l'autre PC la remplace.
                let accueil_neuve = b.is_none() && octets == settings::NOTE_ACCUEIL.as_bytes();
                if b.as_deref() == Some(le.as_str()) || accueil_neuve {
                    // Seul l'autre PC l'a changée.
                    if ecriture::modifier_copie(conn, &ctx, &f.chemin, contenu, &|o| empreinte(o) == le).is_ok() {
                        bilan.notes_recues += 1;
                        poser_base(conn, app, &f.chemin, &f.empreinte).map_err(e)?;
                    }
                } else if b.as_deref() == Some(f.empreinte.as_str()) {
                    // Seul ce PC l'a changée : l'autre prendra notre version.
                } else {
                    // Changée des deux côtés (ou deux notes différentes du même nom) : l'élève choisit.
                    let deja: bool = conn
                        .query_row(
                            "SELECT 1 FROM propositions_vault WHERE chemin = ?1 AND apres = ?2 AND statut = 'attente'",
                            params![f.chemin, contenu],
                            |_| Ok(true),
                        )
                        .optional()
                        .map_err(e)?
                        .is_some();
                    if !deja {
                        conn.execute(
                            "INSERT INTO propositions_vault(date, lot_id, chemin, avant, apres, raison) VALUES (?1, NULL, ?2, ?3, ?4, ?5)",
                            params![
                                ctx.maintenant,
                                f.chemin,
                                String::from_utf8_lossy(&octets),
                                contenu,
                                format!("Modifiée aussi sur {} : choisis la version à garder.", journal.nom)
                            ],
                        )
                        .map_err(e)?;
                        bilan.notes_a_valider += 1;
                    }
                }
            }
            (Some(_), None) => {}
        }
    }

    // Notes disparues de l'autre PC depuis la dernière version commune.
    let distantes: HashSet<&str> = journal.fichiers.iter().map(|f| f.chemin.as_str()).collect();
    let bases: Vec<(String, String)> = conn
        .prepare("SELECT chemin, empreinte FROM synchro_notes_base WHERE appareil = ?1")
        .map_err(e)?
        .query_map([app], |r| Ok((r.get(0)?, r.get(1)?)))
        .map_err(e)?
        .collect::<rusqlite::Result<_>>()
        .map_err(e)?;
    let disparues: Vec<(String, String)> = bases.into_iter().filter(|(c, _)| !distantes.contains(c.as_str())).collect();
    if disparues.is_empty() || !journal.fichiers_complets {
        return Ok(());
    }
    let a_supprimer: Vec<&String> = disparues
        .iter()
        .filter_map(|(chemin, b)| match lire(chemin) {
            Some(o) if empreinte(&o) == *b => Some(chemin),
            _ => None,
        })
        .collect();
    if a_supprimer.len() > SUPPRESSIONS_MAX {
        // Trop d'un coup : sans doute un dossier débranché là-bas. Rien n'est supprimé, l'élève est prévenu.
        bilan.suppressions_refusees += a_supprimer.len();
        return Ok(());
    }
    for (chemin, b) in &disparues {
        if a_supprimer.contains(&chemin) {
            // Dans la corbeille de Révisio : restaurable.
            if ecriture::supprimer(conn, &ctx, chemin).is_ok() {
                bilan.notes_supprimees += 1;
            }
            retirer_base(conn, app, chemin).map_err(e)?;
        } else if lire(chemin).map(|o| empreinte(&o)) != Some(b.clone()) {
            // Absente ici aussi, ou modifiée ici depuis : on la garde, sans base commune.
            retirer_base(conn, app, chemin).map_err(e)?;
        }
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::db;
    use crate::synchro::importer;

    fn dossier(nom: &str) -> std::path::PathBuf {
        let d = std::env::temp_dir().join(format!("revisio-pc-{nom}-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&d);
        std::fs::create_dir_all(&d).unwrap();
        d
    }

    fn ctx(racine: &Path) -> ecriture::Contexte<'_> {
        ecriture::Contexte { racine, autorisee: true, origine: "appareil", lot_id: None, raison: None, maintenant: 5000, groupe: None }
    }

    fn fichier(chemin: &str, texte: &str) -> FichierNote {
        FichierNote { chemin: chemin.into(), empreinte: empreinte(texte.as_bytes()), contenu: Some(texte.into()) }
    }

    fn journal_pc(fichiers: Vec<FichierNote>) -> Journal {
        Journal { version: 1, appareil: "pc-b".into(), nom: "l'ordinateur « B »".into(), pc: true, fichiers, fichiers_complets: true, ..Default::default() }
    }

    /// Essai lancé à la main sur une COPIE des vraies données (REVISIO_ESSAI_A = dossier avec app.db et vault/) :
    /// un second PC vide reçoit tout, puis le premier relit le journal du second (rien ne doit bouger).
    /// `cargo test --lib essai_deux_pc -- --ignored --nocapture`
    #[test]
    #[ignore]
    fn essai_deux_pc() {
        let a_dir = std::path::PathBuf::from(std::env::var("REVISIO_ESSAI_A").unwrap());
        let a_vault = a_dir.join("vault");
        let mut a = db::ouvrir(&a_dir.join("app.db"), Some(&a_vault)).unwrap();
        let t = std::time::Instant::now();
        let ja = journal(&a, Some(&a_vault)).unwrap();
        let json = serde_json::to_vec(&ja).unwrap();
        println!(
            "journal A : {} révisions, {} états, {} erreurs, {} cartes IA, {} notes IA, {} évaluations, {} fichiers ({} Ko compressé) en {:?}",
            ja.revisions.len(), ja.etats.len(), ja.erreurs.len(), ja.cartes.len(), ja.notes_ia.len(), ja.evaluations.len(),
            ja.fichiers.len(), crate::synchro::compresser(&json).len() / 1024, t.elapsed()
        );
        let b_dir = dossier("essai-b");
        let b_vault = b_dir.join("vault");
        std::fs::create_dir_all(&b_vault).unwrap();
        let mut b = db::ouvrir(&b_dir.join("app.db"), None).unwrap();
        let cb = ecriture::Contexte { maintenant: crate::maintenant_ms(), ..ctx(&b_vault) };
        println!("B, 1re fusion : {:?}", importer(&mut b, &ja, Some(&cb)).unwrap());
        crate::indexer::indexer(&mut b, &b_vault).unwrap();
        crate::cartes::synchroniser(&mut b, crate::maintenant_ms()).unwrap();
        println!("B, après indexation : {:?}", importer(&mut b, &ja, Some(&cb)).unwrap());
        let compte = |c: &Connection, sql: &str| c.query_row(sql, [], |r| r.get::<_, i64>(0)).unwrap();
        for sql in [
            "SELECT count(*) FROM revisions",
            "SELECT count(*) FROM cartes WHERE active = 1",
            "SELECT count(*) FROM cartes WHERE active = 1 AND etat_fsrs IS NOT NULL",
            "SELECT count(*) FROM erreurs WHERE carte_id IS NOT NULL",
            "SELECT count(*) FROM evaluations",
        ] {
            println!("{sql} : A = {}, B = {}", compte(&a, sql), compte(&b, sql));
        }
        let pareilles = ja
            .fichiers
            .iter()
            .filter(|f| f.contenu.is_some() && vault::lire_octets(&b_vault, &f.chemin).ok().map(|o| empreinte(&o)) == Some(f.empreinte.clone()))
            .count();
        println!("notes identiques sur B : {pareilles} / {}", ja.fichiers.iter().filter(|f| f.contenu.is_some()).count());
        // Retour : A relit B, rien ne doit être écrit ni proposé.
        let jb = journal(&b, Some(&b_vault)).unwrap();
        let ca = ecriture::Contexte { maintenant: crate::maintenant_ms(), ..ctx(&a_vault) };
        println!("A relit B : {:?}", importer(&mut a, &jb, Some(&ca)).unwrap());
    }

    #[test]
    fn notes_fusion_a_trois_versions() {
        let racine = dossier("fusion");
        std::fs::create_dir_all(racine.join("SVT")).unwrap();
        let mut conn = db::ouvrir_en_memoire().unwrap();
        let c = ctx(&racine);

        // 1re fois : tout est nouveau ici.
        let j = journal_pc(vec![
            fichier("SVT/A.md", "A1"),
            fichier("SVT/B.md", "B1"),
            fichier("SVT/C.md", "C1"),
            fichier("SVT/Cellule (schéma).excalidraw.md", "dessin"),
            fichier("_Templates/Modèle.md", "non"),
        ]);
        let b = importer(&mut conn, &j, Some(&c)).unwrap();
        assert_eq!((b.notes_recues, b.ignorees), (4, 1));
        assert_eq!(std::fs::read_to_string(racine.join("SVT/Cellule (schéma).excalidraw.md")).unwrap(), "dessin");
        assert!(!racine.join("_Templates").exists());
        // L'éditeur et l'IA n'ont toujours pas le droit de toucher un dessin.
        assert!(ecriture::modifier(&conn, &c, "SVT/Cellule (schéma).excalidraw.md", "x", &|_| true).is_err());
        assert_eq!(std::fs::read_to_string(racine.join("SVT/A.md")).unwrap(), "A1");

        // Ici on modifie B ; là-bas on modifie A, et B et C restent pareilles.
        std::fs::write(racine.join("SVT/B.md"), "B2-ici").unwrap();
        let j = journal_pc(vec![fichier("SVT/A.md", "A2"), fichier("SVT/B.md", "B1"), fichier("SVT/C.md", "C1"), fichier("SVT/Cellule (schéma).excalidraw.md", "dessin")]);
        let b = importer(&mut conn, &j, Some(&c)).unwrap();
        assert_eq!((b.notes_recues, b.notes_a_valider), (1, 0));
        assert_eq!(std::fs::read_to_string(racine.join("SVT/A.md")).unwrap(), "A2");
        assert_eq!(std::fs::read_to_string(racine.join("SVT/B.md")).unwrap(), "B2-ici");

        // Les deux changent C : proposée, rien d'écrasé ; une seule proposition même si le journal est relu.
        std::fs::write(racine.join("SVT/C.md"), "C2-ici").unwrap();
        let j = journal_pc(vec![fichier("SVT/A.md", "A2"), fichier("SVT/B.md", "B1"), fichier("SVT/C.md", "C2-la"), fichier("SVT/Cellule (schéma).excalidraw.md", "dessin")]);
        assert_eq!(importer(&mut conn, &j, Some(&c)).unwrap().notes_a_valider, 1);
        assert_eq!(importer(&mut conn, &j, Some(&c)).unwrap().notes_a_valider, 0);
        assert_eq!(std::fs::read_to_string(racine.join("SVT/C.md")).unwrap(), "C2-ici");

        // A supprimée là-bas (inchangée ici) : à la corbeille. B (modifiée ici) gardée.
        let j = journal_pc(vec![fichier("SVT/C.md", "C2-la"), fichier("SVT/Cellule (schéma).excalidraw.md", "dessin")]);
        let b = importer(&mut conn, &j, Some(&c)).unwrap();
        assert_eq!(b.notes_supprimees, 1);
        assert!(!racine.join("SVT/A.md").exists());
        assert!(racine.join("SVT/B.md").exists());
        let corbeille: i64 = conn.query_row("SELECT count(*) FROM historique_vault WHERE genre = 'suppression'", [], |r| r.get(0)).unwrap();
        assert_eq!(corbeille, 1);
        let _ = std::fs::remove_dir_all(&racine);
    }

    #[test]
    fn pas_de_suppression_massive_ni_sans_liste_complete() {
        let racine = dossier("massive");
        let mut conn = db::ouvrir_en_memoire().unwrap();
        let c = ctx(&racine);
        let tous: Vec<FichierNote> = (0..25).map(|i| fichier(&format!("N{i}.md"), "x")).collect();
        assert_eq!(importer(&mut conn, &journal_pc(tous), Some(&c)).unwrap().notes_recues, 25);
        // Liste incomplète (dossier illisible là-bas) : rien.
        let mut vide = journal_pc(vec![]);
        vide.fichiers_complets = false;
        assert_eq!(importer(&mut conn, &vide, Some(&c)).unwrap().notes_supprimees, 0);
        // 25 suppressions d'un coup : refusées.
        let b = importer(&mut conn, &journal_pc(vec![fichier("N0.md", "x")]), Some(&c)).unwrap();
        assert_eq!((b.notes_supprimees, b.suppressions_refusees), (0, 24));
        assert!(racine.join("N5.md").exists());
        let _ = std::fs::remove_dir_all(&racine);
    }

    #[test]
    fn accueil_neuve_remplacee_et_ecriture_desactivee() {
        let racine = dossier("accueil");
        std::fs::write(racine.join("Accueil.md"), settings::NOTE_ACCUEIL).unwrap();
        let mut conn = db::ouvrir_en_memoire().unwrap();
        let j = journal_pc(vec![fichier("Accueil.md", "Mon accueil"), fichier("X.md", "x")]);
        let sans = ecriture::Contexte { autorisee: false, ..ctx(&racine) };
        let b = importer(&mut conn, &j, Some(&sans)).unwrap();
        assert_eq!((b.notes_recues, b.notes_en_attente), (0, 2));
        let b = importer(&mut conn, &j, Some(&ctx(&racine))).unwrap();
        assert_eq!((b.notes_recues, b.notes_a_valider), (2, 0));
        assert_eq!(std::fs::read_to_string(racine.join("Accueil.md")).unwrap(), "Mon accueil");
        let _ = std::fs::remove_dir_all(&racine);
    }

    #[test]
    fn cartes_ia_evaluations_et_aller_retour() {
        // PC A : une carte de l'IA révisée, une note traitée par l'IA, une évaluation, une évaluation supprimée.
        let a = db::ouvrir_en_memoire().unwrap();
        a.execute_batch(
            "INSERT INTO cartes(id, cle, genre, note_chemin, matiere, chapitre, question, reponse, creee_le, etat_fsrs, echeance, etat_maj)
               VALUES (1, 'ia|SVT/M.md|1', 'question', 'SVT/M.md', 'SVT', 'M', 'q', 'r', 10, '{\"s\":1}', 900, 800);
             INSERT INTO revisions(carte_id, date, note, confiance, correcte) VALUES (1, 800, 3, 'sur', 1);
             INSERT INTO cartes_ia_notes(chemin, statut, taille, cartes, maj) VALUES ('SVT/M.md', 'faite', 100, 1, 700);",
        )
        .unwrap();
        let ev = planning::Evaluation { id: 0, jour: "2026-12-01".into(), matiere: "SVT".into(), titre: "DS".into(), genre: "ds".into(), chapitres: vec![Some("M".into())] };
        planning::enregistrer(&a, &ev, 1000).unwrap();
        let id2 = planning::enregistrer(&a, &planning::Evaluation { titre: "Oral".into(), ..ev.clone() }, 1000).unwrap();
        let ja = journal(&a, None).unwrap();
        assert!(ja.pc);
        assert_eq!((ja.cartes.len(), ja.revisions.len(), ja.etats.len(), ja.evaluations.len()), (1, 1, 1, 2));

        // PC B reçoit tout.
        let mut b = db::ouvrir_en_memoire().unwrap();
        b.execute("INSERT INTO cartes_ia_notes(chemin, statut, taille, cartes, essais, erreur, maj) VALUES ('SVT/M.md', 'erreur', 0, 0, 1, 'quota', 1)", []).unwrap();
        let bilan = importer(&mut b, &ja, None).unwrap();
        assert_eq!((bilan.cartes, bilan.revisions, bilan.etats, bilan.evaluations), (1, 1, 1, 2));
        let statut: String = b.query_row("SELECT statut FROM cartes_ia_notes WHERE chemin = 'SVT/M.md'", [], |r| r.get(0)).unwrap();
        assert_eq!(statut, "faite");
        // Rejoué : rien de plus.
        let bilan = importer(&mut b, &ja, None).unwrap();
        assert_eq!((bilan.cartes, bilan.revisions, bilan.etats, bilan.evaluations), (0, 0, 0, 0));

        // A supprime l'Oral : B le supprime aussi, et ne le reprend pas d'un journal plus ancien.
        planning::supprimer(&a, id2).unwrap();
        assert_eq!(importer(&mut b, &journal(&a, None).unwrap(), None).unwrap().evaluations, 1);
        importer(&mut b, &ja, None).unwrap();
        let n: i64 = b.query_row("SELECT count(*) FROM evaluations", [], |r| r.get(0)).unwrap();
        assert_eq!(n, 1);

        // B déplace le DS (plus récent) : A le reçoit.
        b.execute("UPDATE evaluations SET jour = '2026-12-05', maj = 2000", []).unwrap();
        let mut a = a;
        assert_eq!(importer(&mut a, &journal(&b, None).unwrap(), None).unwrap().evaluations, 1);
        let jour: String = a.query_row("SELECT jour FROM evaluations", [], |r| r.get(0)).unwrap();
        assert_eq!(jour, "2026-12-05");
    }
}
