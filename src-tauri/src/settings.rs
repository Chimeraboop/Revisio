//! Réglages de l'app, enregistrés dans `%APPDATA%\Revisio\settings.json`.
//! (La clé API Gemini n'y sera jamais : elle ira dans le Gestionnaire d'identification Windows.)

use std::path::{Path, PathBuf};

use serde::{Deserialize, Serialize};

use crate::paths::verifier_hors_vault;

const NOM_FICHIER: &str = "settings.json";

/// Couleurs d'accent proposées (les teintes exactes sont dans l'interface).
pub const ACCENTS: [&str; 7] = ["monochrome", "bleu", "violet", "vert", "orange", "rose", "turquoise"];

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(default, rename_all = "camelCase")]
pub struct Reglages {
    /// Dossier du vault Obsidian (lu, jamais modifié).
    pub chemin_vault: Option<String>,
    /// Nom du vault dans Obsidian, pour les liens `obsidian://open?vault=...`.
    /// Vide : le nom du dossier du vault (c'est ce qu'Obsidian utilise).
    pub nom_vault_obsidian: String,
    /// Guide de démarrage terminé ? Absent des anciens settings.json : on le considère
    /// terminé si un vault était déjà choisi (voir `charger`).
    pub accueil_termine: Option<bool>,
    /// "systeme", "clair" ou "sombre".
    pub theme: String,
    /// Couleur d'accent (voir `ACCENTS`) ; "monochrome" = noir et blanc.
    pub accent: String,
    /// Nombre de nouvelles cartes introduites par jour (séance « Autres matières »).
    pub nouvelles_par_jour: usize,
    /// Nombre de nouvelles cartes de vocabulaire par jour (séance « Langues »).
    pub nouvelles_par_jour_langues: usize,
    /// Mélanger matières et chapitres pendant une séance.
    pub entrelacement: bool,
    /// Vocabulaire langue étrangère → français : "retourner" (auto-notation) ou "taper".
    pub vocab_vers_francais: String,
    /// Vocabulaire français → langue étrangère : "retourner" ou "taper".
    pub vocab_depuis_francais: String,
    /// Durée visée pour la séance du jour « Autres matières », en minutes.
    pub duree_seance: u32,
    /// Durée visée pour la séance du jour « Langues », en minutes.
    pub duree_seance_langues: u32,
    /// Modèle Gemini pour les tâches exigeantes (questions, corrections). Jamais écrit en dur ailleurs.
    pub modele_ia: String,
    /// Modèle Gemini rapide et économique (tâches simples).
    pub modele_ia_rapide: String,
    /// Appels à Gemini autorisés par minute (pour rester dans le quota gratuit).
    pub ia_par_minute: u32,
    /// Plafond d'appels à Gemini par jour.
    pub ia_par_jour: u32,
    /// Modèle Gemini de synthèse vocale (podcasts).
    pub modele_tts: String,
    /// Voix prédéfinies des deux personnes du podcast (noms donnés par Google : Kore, Puck…).
    pub voix_a: String,
    pub voix_b: String,
    /// Voix de la narration des vidéos.
    pub voix_video: String,
    /// IA locale (Ollama) pour les tâches simples : "jamais", "secours" (quand Gemini est indisponible) ou "toujours".
    pub ia_locale: String,
    /// Modèle Ollama choisi (vide : IA locale pas encore configurée).
    pub modele_local: String,
    /// Adresse d'Ollama sur ce PC.
    pub adresse_ollama: String,
    /// Mode local choisi avec l'interrupteur « En ligne » : Gemini coupé, l'IA locale répond aux tâches simples.
    pub mode_local: bool,
    /// Matières laissées de côté dans les deux séances du jour (« Hors matière » pour les notes sans matière).
    /// Leurs cartes restent accessibles en séance ciblée.
    pub matieres_exclues: Vec<String>,
    /// L'élève autorise Révisio à écrire dans le vault (notes tirées des photos, mode Modifier).
    /// Désactivé par défaut : le vault reste alors en lecture seule.
    pub ecriture_autorisee: bool,
    /// Dossier surveillé où arrivent les photos de cours (par ex. un dossier Google Drive synchronisé).
    pub dossier_photos: Option<String>,
}

impl Default for Reglages {
    fn default() -> Self {
        Self {
            chemin_vault: None,
            nom_vault_obsidian: String::new(),
            accueil_termine: None,
            theme: "systeme".into(),
            accent: "monochrome".into(),
            nouvelles_par_jour: 20,
            nouvelles_par_jour_langues: 20,
            entrelacement: true,
            vocab_vers_francais: "retourner".into(),
            vocab_depuis_francais: "taper".into(),
            duree_seance: 20,
            duree_seance_langues: 10,
            // Modèles stables du niveau gratuit d'après ai.google.dev (octobre 2026) ;
            // modifiables dans les Réglages à partir de la liste fournie par Google.
            modele_ia: "gemini-3.8-flash".into(),
            modele_ia_rapide: "gemini-3.5-flash-lite".into(),
            ia_par_minute: 8,
            ia_par_jour: 200,
            modele_tts: "gemini-3.8-flash-tts".into(),
            voix_a: "Kore".into(),
            voix_b: "Puck".into(),
            voix_video: "Charon".into(),
            ia_locale: "secours".into(),
            modele_local: String::new(),
            adresse_ollama: crate::ia::ollama::ADRESSE_DEFAUT.into(),
            mode_local: false,
            matieres_exclues: Vec::new(),
            ecriture_autorisee: false,
            dossier_photos: None,
        }
    }
}

impl Reglages {
    pub fn vault(&self) -> Option<PathBuf> {
        self.chemin_vault.as_ref().map(PathBuf::from)
    }

    /// Nom du vault pour les liens vers Obsidian.
    pub fn nom_obsidian(&self) -> String {
        if !self.nom_vault_obsidian.is_empty() {
            return self.nom_vault_obsidian.clone();
        }
        self.vault()
            .and_then(|v| v.file_name().map(|n| n.to_string_lossy().into_owned()))
            .unwrap_or_default()
    }
}

/// Lit les réglages ; en cas d'absence ou de fichier abîmé, renvoie les valeurs par défaut.
pub fn charger(dossier: &Path) -> Reglages {
    let mut r: Reglages = std::fs::read_to_string(dossier.join(NOM_FICHIER))
        .ok()
        .and_then(|t| serde_json::from_str(&t).ok())
        .unwrap_or_default();
    // Mise à jour d'une version sans guide de démarrage : un élève qui avait déjà choisi
    // son vault ne revoit pas le guide.
    if r.accueil_termine.is_none() {
        r.accueil_termine = Some(r.chemin_vault.is_some());
    }
    r
}

/// Enregistre les réglages (écriture dans un fichier temporaire puis renommage,
/// pour ne jamais laisser un fichier à moitié écrit).
pub fn enregistrer(dossier: &Path, reglages: &Reglages) -> Result<(), String> {
    let cible = dossier.join(NOM_FICHIER);
    verifier_hors_vault(&cible, reglages.vault().as_deref())?;
    std::fs::create_dir_all(dossier).map_err(|e| format!("Création du dossier de données : {e}"))?;
    let texte = serde_json::to_string_pretty(reglages).map_err(|e| e.to_string())?;
    let temporaire = dossier.join(format!("{NOM_FICHIER}.tmp"));
    std::fs::write(&temporaire, texte).map_err(|e| format!("Écriture des réglages : {e}"))?;
    std::fs::rename(&temporaire, &cible).map_err(|e| format!("Écriture des réglages : {e}"))?;
    Ok(())
}

/// Vérifie qu'un dossier peut servir de dossier de notes : n'importe quel dossier (vault Obsidian
/// ou non), sauf la racine d'un disque et le dossier des données de Révisio.
pub fn valider_vault(chemin: &Path, dossier_donnees: &Path) -> Result<(), String> {
    if !chemin.is_dir() {
        return Err("Ce dossier n'existe pas.".into());
    }
    if chemin.parent().is_none() {
        return Err("Choisis un dossier précis, pas la racine d'un disque.".into());
    }
    if crate::paths::est_dans(chemin, dossier_donnees) || crate::paths::est_dans(dossier_donnees, chemin) {
        return Err("Ce dossier contient les données de Révisio : choisis un autre dossier pour tes notes.".into());
    }
    Ok(())
}

/// Vrai si le dossier est un vault Obsidian (bouton « Ouvrir dans Obsidian »).
pub fn est_vault_obsidian(chemin: &Path) -> bool {
    chemin.join(".obsidian").is_dir()
}

/// Première note d'un dossier de notes créé par Révisio (genre « accueil » : aucune carte n'en est tirée).
pub const NOTE_ACCUEIL: &str = r#"# Bienvenue dans ton dossier de notes

Ce dossier t'appartient : ce sont de simples fichiers Markdown (`.md`), lisibles par n'importe quel éditeur de texte, et par Obsidian si tu l'utilises un jour.

## Ranger tes notes
- Un dossier par **matière** (par exemple « Histoire »), et dedans un dossier par **chapitre**.
- Une note par notion, plutôt courte : Révisio en tire des cartes de révision.

## Ce qui devient des cartes de révision
- Dans une note dont le titre commence par « Fiche de synthèse », les mots en **gras** deviennent des textes à trous.
- Dans un encadré « Définition », les mots en gras aussi :

```markdown
> [!info]+ Définition
> La **photosynthèse** produit du glucose à partir de la lumière.
```

- Dans une note dont le titre commence par « Vocabulaire », chaque ligne `* mot = traduction` devient une carte.
- Une liste numérotée d'étapes devient un exercice « remettre dans l'ordre ».

## Relier tes notes
Écris `[[Titre d'une note]]` pour faire un lien vers une autre note. Tu peux aussi coller une image dans une note.

Tu peux supprimer cette note quand tu veux.
"#;

/// Un vault qu'Obsidian connaît sur ce PC.
#[derive(Debug, Clone, Serialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct VaultDetecte {
    pub chemin: String,
    pub nom: String,
}

/// Vaults ouverts récemment dans Obsidian, lus (sans jamais l'écrire) dans `obsidian.json`
/// (`%APPDATA%\obsidian` sous Windows, `~/Library/Application Support/obsidian` sous macOS).
/// Ne garde que les dossiers qui existent encore.
pub fn vaults_obsidian() -> Vec<VaultDetecte> {
    let Ok(applications) = crate::paths::dossier_applications() else { return Vec::new() };
    let Ok(texte) = std::fs::read_to_string(applications.join("obsidian").join("obsidian.json")) else {
        return Vec::new();
    };
    chemins_obsidian(&texte)
        .into_iter()
        .filter(|c| Path::new(c).is_dir())
        .map(|chemin| VaultDetecte {
            nom: Path::new(&chemin).file_name().map(|n| n.to_string_lossy().into_owned()).unwrap_or_default(),
            chemin,
        })
        .collect()
}

/// Chemins des vaults de `obsidian.json`, du plus récemment ouvert au plus ancien.
/// Format : `{"vaults": {"<id>": {"path": "...", "ts": 1700000000000, "open": true}}}`.
fn chemins_obsidian(texte: &str) -> Vec<String> {
    let Ok(json) = serde_json::from_str::<serde_json::Value>(texte) else { return Vec::new() };
    let Some(vaults) = json.get("vaults").and_then(|v| v.as_object()) else { return Vec::new() };
    let mut liste: Vec<(i64, String)> = vaults
        .values()
        .filter_map(|v| {
            let chemin = v.get("path")?.as_str()?.to_string();
            Some((v.get("ts").and_then(|t| t.as_i64()).unwrap_or(0), chemin))
        })
        .collect();
    liste.sort_by(|a, b| b.0.cmp(&a.0));
    let mut chemins: Vec<String> = Vec::new();
    for (_, c) in liste {
        if !chemins.contains(&c) {
            chemins.push(c);
        }
    }
    chemins
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn aller_retour_des_reglages() {
        let dossier = std::env::temp_dir().join(format!("revisio-test-reglages-{}", std::process::id()));
        let r = Reglages {
            chemin_vault: Some(r"D:\un\vault".into()),
            accueil_termine: Some(false),
            theme: "sombre".into(),
            ..Default::default()
        };
        enregistrer(&dossier, &r).unwrap();
        assert_eq!(charger(&dossier), r);
        std::fs::remove_dir_all(&dossier).unwrap();
    }

    #[test]
    fn lit_les_vaults_d_obsidian_du_plus_recent_au_plus_ancien() {
        let texte = r#"{"vaults": {
            "a1": {"path": "C:\\Cours\\Ancien", "ts": 100},
            "b2": {"path": "D:\\Notes\\Lycee", "ts": 300, "open": true},
            "c3": {"ts": 200},
            "d4": {"path": "D:\\Notes\\Lycee", "ts": 50}
        }}"#;
        assert_eq!(chemins_obsidian(texte), vec![r"D:\Notes\Lycee".to_string(), r"C:\Cours\Ancien".to_string()]);
        assert!(chemins_obsidian("pas du json").is_empty());
        assert!(chemins_obsidian(r#"{"autre": 1}"#).is_empty());
    }

    #[test]
    fn guide_de_demarrage_deja_fait_pour_un_ancien_eleve() {
        let dossier = std::env::temp_dir().join(format!("revisio-test-accueil-{}", std::process::id()));
        std::fs::create_dir_all(&dossier).unwrap();
        // Ancien settings.json (sans accueilTermine) avec un vault : pas de guide.
        std::fs::write(dossier.join(NOM_FICHIER), r#"{"cheminVault": "D:\\vault"}"#).unwrap();
        assert_eq!(charger(&dossier).accueil_termine, Some(true));
        // Ancien settings.json sans vault, ou premier lancement : guide.
        std::fs::write(dossier.join(NOM_FICHIER), r#"{"theme": "sombre"}"#).unwrap();
        assert_eq!(charger(&dossier).accueil_termine, Some(false));
        std::fs::remove_file(dossier.join(NOM_FICHIER)).unwrap();
        assert_eq!(charger(&dossier).accueil_termine, Some(false));
        std::fs::remove_dir_all(&dossier).unwrap();
    }

    #[test]
    fn n_importe_quel_dossier_de_notes_sauf_racine_et_donnees() {
        let base = std::env::temp_dir().join(format!("revisio-test-dossier-notes-{}", std::process::id()));
        let notes = base.join("Mes cours");
        let donnees = base.join("Revisio");
        std::fs::create_dir_all(&notes).unwrap();
        std::fs::create_dir_all(&donnees).unwrap();
        // Pas besoin de .obsidian : un simple dossier suffit.
        assert!(valider_vault(&notes, &donnees).is_ok());
        assert!(!est_vault_obsidian(&notes));
        assert!(valider_vault(&base.join("Absent"), &donnees).is_err());
        assert!(valider_vault(&donnees, &donnees).is_err());
        assert!(valider_vault(&base, &donnees).is_err(), "contient les données de Révisio");
        let racine = notes.ancestors().last().unwrap();
        assert!(valider_vault(racine, &donnees).is_err());
        std::fs::create_dir_all(notes.join(".obsidian")).unwrap();
        assert!(est_vault_obsidian(&notes));
        std::fs::remove_dir_all(&base).unwrap();
    }

    #[test]
    fn nom_du_vault_pour_obsidian() {
        let vault = std::env::temp_dir().join("Mon vault");
        let mut r = Reglages { chemin_vault: Some(vault.to_string_lossy().into()), ..Default::default() };
        assert_eq!(r.nom_obsidian(), "Mon vault");
        r.nom_vault_obsidian = "autre".into();
        assert_eq!(r.nom_obsidian(), "autre");
    }

    #[test]
    fn refuse_un_dossier_de_donnees_dans_le_vault() {
        let vault = std::env::temp_dir().join(format!("revisio-test-vault-{}", std::process::id()));
        std::fs::create_dir_all(&vault).unwrap();
        let r = Reglages { chemin_vault: Some(vault.to_string_lossy().into()), ..Default::default() };
        assert!(enregistrer(&vault.join("donnees"), &r).is_err());
        assert!(!vault.join("donnees").exists());
        std::fs::remove_dir_all(&vault).unwrap();
    }
}
