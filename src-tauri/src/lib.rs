//! Cœur Rust de Révisio.
//!
//! L'interface (React) appelle les fonctions marquées `#[tauri::command]`
//! (dans `commandes.rs`) avec `invoke("nom_de_la_fonction", { ...arguments })`.

pub mod audio;
pub mod calcul;
pub mod cartes;
pub mod cartes_mentales;
pub mod db;
pub mod discussion;
pub mod ecriture;
pub mod ia;
pub mod indexer;
pub mod navigation;
pub mod parser;
pub mod photos;
pub mod paths;
pub mod planning;
pub mod podcasts;
pub mod renommage;
pub mod revision;
pub mod search;
pub mod secret;
pub mod settings;
pub mod sources_web;
pub mod taches;
pub mod vault;
pub mod videos;
pub mod web;

mod commandes;

use std::path::PathBuf;
use std::sync::Mutex;

use rusqlite::Connection;
use serde::Serialize;
use tauri::{AppHandle, Emitter, Manager};

use settings::Reglages;

/// État partagé entre toutes les commandes (l'équivalent d'un objet global en Python,
/// protégé par des verrous `Mutex` car plusieurs appels peuvent arriver en même temps).
pub struct Etat {
    /// `%APPDATA%\Revisio`
    pub dossier: PathBuf,
    pub reglages: Mutex<Reglages>,
    pub db: Mutex<Connection>,
    pub indexation: Mutex<EtatIndexation>,
    /// Surveillance du vault (réindexation automatique quand une note change).
    pub surveillance: Mutex<Option<vault::Surveillance>>,
    /// Accès à Gemini (clé, limites de débit).
    pub ia: ia::ServiceIa,
    /// Générations par l'IA en cours (pour pouvoir les interrompre).
    pub taches: taches::Taches,
    /// Surveillance du dossier des photos de cours.
    pub photos: photos::Suivi,
}

impl Etat {
    pub fn limites_ia(&self) -> ia::Limites {
        let r = self.reglages.lock().unwrap();
        ia::Limites { par_minute: r.ia_par_minute, par_jour: r.ia_par_jour }
    }

    /// IA locale réglée pour les tâches simples (aucune si désactivée ou si aucun modèle n'est choisi).
    /// En mode local (interrupteur « En ligne » coupé), elle répond toujours, quel que soit le réglage.
    pub fn config_locale(&self) -> Option<ia::ConfigLocale> {
        let r = self.reglages.lock().unwrap();
        let mode = match r.ia_locale.as_str() {
            _ if r.mode_local => ia::ModeLocal::Toujours,
            "toujours" => ia::ModeLocal::Toujours,
            "secours" => ia::ModeLocal::Secours,
            _ => return None,
        };
        (!r.modele_local.is_empty())
            .then(|| ia::ConfigLocale { adresse: r.adresse_ollama.clone(), modele: r.modele_local.clone(), mode })
    }
}

/// (Re)démarre la surveillance du vault choisi dans les réglages.
pub fn demarrer_surveillance(app: &AppHandle) {
    let etat = app.state::<Etat>();
    let mut surveillance = etat.surveillance.lock().unwrap();
    *surveillance = None; // arrête l'ancienne surveillance
    let Some(racine) = etat.reglages.lock().unwrap().vault() else { return };
    let app_rappel = app.clone();
    match vault::surveiller(&racine, move || lancer_indexation(&app_rappel)) {
        Ok(s) => *surveillance = Some(s),
        // Sans surveillance, l'app reste utilisable : bouton « Réindexer » sur l'accueil.
        Err(e) => etat.indexation.lock().unwrap().erreur = Some(e),
    }
}

#[derive(Debug, Clone, Default, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct EtatIndexation {
    pub en_cours: bool,
    /// Une nouvelle indexation a été demandée pendant la précédente.
    #[serde(skip)]
    pub a_relancer: bool,
    pub dernier: Option<indexer::Rapport>,
    pub cartes: Option<cartes::RapportCartes>,
    pub erreur: Option<String>,
}

/// Heure actuelle en millisecondes depuis 1970.
pub fn maintenant_ms() -> i64 {
    std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|d| d.as_millis() as i64)
        .unwrap_or(0)
}

/// Lance l'indexation du vault en arrière-plan (l'interface reste utilisable)
/// et prévient l'interface par l'événement « indexation » à chaque changement d'état.
pub fn lancer_indexation(app: &AppHandle) {
    let etat = app.state::<Etat>();
    {
        let mut ix = etat.indexation.lock().unwrap();
        if ix.en_cours {
            ix.a_relancer = true;
            return;
        }
        ix.en_cours = true;
        let _ = app.emit("indexation", ix.clone());
    }
    let app = app.clone();
    std::thread::spawn(move || loop {
        let etat = app.state::<Etat>();
        let vault = etat.reglages.lock().unwrap().vault();
        let resultat = match vault {
            Some(v) => {
                let mut conn = etat.db.lock().unwrap();
                // Après l'index, les cartes de révision sont recalculées (sans perdre la progression).
                indexer::indexer(&mut conn, &v)
                    .and_then(|r| cartes::synchroniser(&mut conn, maintenant_ms()).map(|c| (r, c)))
            }
            None => Err("Aucun vault choisi.".into()),
        };
        let mut ix = etat.indexation.lock().unwrap();
        match resultat {
            Ok((r, c)) => {
                ix.dernier = Some(r);
                ix.cartes = Some(c);
                ix.erreur = None;
            }
            Err(e) => ix.erreur = Some(e),
        }
        if ix.a_relancer {
            ix.a_relancer = false;
            continue;
        }
        ix.en_cours = false;
        let _ = app.emit("indexation", ix.clone());
        break;
    });
}

#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    let dossier = paths::dossier_donnees_par_defaut().expect("dossier %APPDATA% introuvable");
    let reglages = settings::charger(&dossier);
    let conn = db::ouvrir(&dossier.join("app.db"), reglages.vault().as_deref()).expect("ouverture de la base");
    let ia = ia::ServiceIa::default();
    ia.couper_gemini(reglages.mode_local);

    tauri::Builder::default()
        .plugin(tauri_plugin_opener::init())
        .plugin(tauri_plugin_dialog::init())
        .manage(web::EspaceWeb::default())
        .manage(Etat {
            dossier,
            reglages: Mutex::new(reglages),
            db: Mutex::new(conn),
            indexation: Mutex::new(EtatIndexation::default()),
            surveillance: Mutex::new(None),
            ia,
            taches: taches::Taches::default(),
            photos: Default::default(),
        })
        .setup(|app| {
            {
                // Lots interrompus par la fermeture de Révisio : à relancer depuis l'écran Photos de cours.
                let etat = app.state::<Etat>();
                let conn = etat.db.lock().unwrap();
                let _ = conn.execute(
                    "UPDATE lots_photos SET statut = 'erreur', message = 'Interrompu : Révisio a été fermé pendant le traitement.'
                     WHERE statut IN ('attente', 'en_cours')",
                    [],
                );
            }
            photos::demarrer_surveillance(app.handle().clone());
            if app.state::<Etat>().reglages.lock().unwrap().chemin_vault.is_some() {
                lancer_indexation(app.handle());
                demarrer_surveillance(app.handle());
            }
            Ok(())
        })
        .invoke_handler(tauri::generate_handler![
            commandes::infos_app,
            commandes::lire_reglages,
            commandes::definir_vault,
            commandes::definir_accueil_termine,
            commandes::creer_dossier_notes,
            commandes::est_vault_obsidian,
            commandes::definir_ecriture_autorisee,
            commandes::enregistrer_note,
            commandes::historique_vault,
            commandes::annuler_ecriture,
            commandes::dossiers_notes,
            commandes::creer_dossier,
            commandes::creer_note,
            commandes::renommer_element,
            commandes::deplacer_element,
            commandes::supprimer_element,
            commandes::corbeille,
            commandes::restaurer,
            commandes::ajouter_image,
            commandes::importer_image,
            commandes::importer_photos,
            commandes::relancer_lot_photos,
            commandes::lots_photos,
            commandes::propositions_vault,
            commandes::accepter_proposition,
            commandes::refuser_proposition,
            commandes::definir_dossier_photos,
            commandes::compter_photos_dossier,
            commandes::etat_photos,
            commandes::dossiers_drive,
            commandes::definir_theme,
            commandes::definir_accent,
            commandes::definir_mode_local,
            commandes::etat_indexation,
            commandes::reindexer,
            commandes::statistiques,
            commandes::arbre,
            commandes::note,
            commandes::resoudre_lien,
            commandes::lire_fichier_vault,
            commandes::ouvrir_dans_obsidian,
            commandes::ouvrir_lien_externe,
            commandes::rechercher,
            commandes::definir_reglages_revision,
            commandes::resume_revision,
            commandes::preparer_seance,
            discussion::liste_conversations,
            discussion::creer_conversation,
            discussion::ouvrir_conversation,
            discussion::renommer_conversation,
            discussion::supprimer_conversation,
            discussion::definir_sources_conversation,
            discussion::envoyer_message,
            discussion::relancer_reponse,
            discussion::decider_creation,
            discussion::creations_ia,
            discussion::renommer_creation,
            discussion::retirer_creation,
            discussion::points_fonction,
            discussion::ajouter_creation_a_note,
            commandes::definir_matieres_exclues,
            commandes::enregistrer_revision,
            commandes::suspendre_carte,
            commandes::carnet_erreurs,
            commandes::marquer_erreur_comprise,
            commandes::cartes_choisies,
            commandes::tableau_de_bord,
            commandes::marquer_cartes_apprises,
            commandes::etat_ia,
            commandes::enregistrer_cle_ia,
            commandes::supprimer_cle_ia,
            commandes::lister_modeles_ia,
            commandes::definir_reglages_ia,
            commandes::tester_ia,
            commandes::verifier_internet,
            commandes::annuler_tache,
            commandes::etat_ia_locale,
            commandes::definir_reglages_locaux,
            commandes::tester_ia_locale,
            commandes::telecharger_modele_local,
            commandes::annuler_telechargement_local,
            commandes::supprimer_modele_local,
            commandes::chapitres_ia,
            commandes::estimer_questions,
            commandes::generer_questions,
            commandes::corriger_reponse,
            commandes::notions_feynman,
            commandes::evaluer_feynman,
            commandes::historique_feynman,
            commandes::types_exercices,
            commandes::generer_sujet,
            commandes::sujets_enregistres,
            commandes::sujet_enregistre,
            commandes::supprimer_sujet,
            commandes::rendre_copie,
            commandes::corriger_redactions,
            commandes::retirer_question,
            commandes::dernieres_notes,
            commandes::enregistrer_export,
            commandes::montrer_fichier,
            commandes::enregistrer_evaluation,
            commandes::evaluations,
            commandes::supprimer_evaluation,
            commandes::revues_du_jour,
            commandes::web_onglets,
            commandes::web_ouvrir,
            commandes::web_naviguer,
            commandes::web_historique,
            commandes::web_activer,
            commandes::web_fermer,
            commandes::web_zone,
            commandes::web_masquer,
            commandes::web_accueil,
            commandes::web_selection,
            commandes::web_eval_debug,
            commandes::favoris,
            commandes::ajouter_favori,
            commandes::supprimer_favori,
            commandes::sources_web,
            commandes::ajouter_source_web,
            commandes::supprimer_source_web,
            commandes::schemas_ia,
            commandes::generer_questions_schema,
            commandes::plan_chapitre,
            commandes::cartes_mentales_chapitre,
            commandes::supprimer_carte_mentale,
            commandes::generer_carte_mentale,
            commandes::estimer_podcast,
            commandes::podcasts,
            commandes::ecrire_podcast,
            commandes::enregistrer_morceau_podcast,
            commandes::assembler_podcast,
            commandes::lire_audio_podcast,
            commandes::remplacer_audio_podcast,
            commandes::position_podcast,
            commandes::supprimer_podcast,
            commandes::videos,
            commandes::ecrire_video,
            commandes::enregistrer_narration_video,
            commandes::narration_video,
            commandes::enregistrer_fichier_video,
            commandes::lire_fichier_video,
            commandes::chemin_video,
            commandes::supprimer_video,
        ])
        .run(tauri::generate_context!())
        .expect("erreur au lancement de Révisio");
}
