//! Extensions installées à côté de Révisio, dans le dossier `extensions` des données
//! (`%APPDATA%\Revisio\extensions\<id>\extension.json`).
//!
//! Une extension est un programme : Révisio lui écrit une demande JSON sur son entrée standard et lit
//! sa réponse JSON sur sa sortie standard. Elle ne fait que rédiger : Révisio garde ses vérifications,
//! l'écriture dans le vault, le journal et l'annulation. Pour l'instant, une seule tâche peut être
//! confiée à une extension : la rédaction des notes tirées des photos de cours (« photos »).
//!
//! Demande : `{ version, tache, systeme, consigne, images, schema }` (`images` : chemins des photos préparées, JPEG / PNG / WebP / HEIC, ou de PDF).
//! Réponse : `{ "ok": true, "reponse": <JSON conforme au schéma>, "modele": "…" }`
//! ou `{ "ok": false, "erreur": "message en français" }`.

use std::path::{Path, PathBuf};
use std::process::Stdio;
use std::time::Duration;

use serde::{Deserialize, Serialize};
use serde_json::Value;
use tokio::io::{AsyncReadExt, AsyncWriteExt};

/// Version du protocole envoyée dans chaque demande.
pub const VERSION_PROTOCOLE: u32 = 1;
const MANIFESTE: &str = "extension.json";
/// Temps laissé à une extension pour répondre (une rédaction avec réflexion peut être longue).
const DELAI: Duration = Duration::from_secs(20 * 60);

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct Extension {
    /// Nom du dossier de l'extension.
    #[serde(default)]
    pub id: String,
    pub nom: String,
    #[serde(default)]
    pub description: String,
    #[serde(default)]
    pub version: String,
    /// Tâches que l'extension sait faire (« photos »).
    #[serde(default)]
    pub taches: Vec<String>,
    /// Programme et arguments, lancés depuis le dossier de l'extension (par ex. `["node", "extension.mjs"]`).
    #[serde(default, skip_serializing)]
    pub commande: Vec<String>,
    #[serde(skip)]
    pub dossier: PathBuf,
}

pub fn dossier(donnees: &Path) -> PathBuf {
    donnees.join("extensions")
}

/// Extensions installées (manifeste lisible et commande renseignée), triées par nom.
pub fn lister(donnees: &Path) -> Vec<Extension> {
    let Ok(entrees) = std::fs::read_dir(dossier(donnees)) else { return Vec::new() };
    let mut liste: Vec<Extension> = entrees
        .filter_map(Result::ok)
        .filter(|e| e.file_type().map(|t| t.is_dir()).unwrap_or(false))
        .filter_map(|e| {
            let texte = std::fs::read_to_string(e.path().join(MANIFESTE)).ok()?;
            let mut ext: Extension = serde_json::from_str(texte.trim_start_matches('\u{feff}')).ok()?;
            ext.id = e.file_name().to_string_lossy().into_owned();
            ext.dossier = e.path();
            (!ext.commande.is_empty() && !ext.nom.trim().is_empty()).then_some(ext)
        })
        .collect();
    liste.sort_by(|a, b| a.nom.to_lowercase().cmp(&b.nom.to_lowercase()));
    liste
}

pub fn trouver(donnees: &Path, id: &str, tache: &str) -> Option<Extension> {
    lister(donnees).into_iter().find(|e| e.id == id && e.taches.iter().any(|t| t == tache))
}

#[derive(Debug, Serialize)]
pub struct Demande<'a> {
    pub version: u32,
    pub tache: &'a str,
    pub systeme: &'a str,
    pub consigne: &'a str,
    pub images: Vec<String>,
    pub schema: Option<&'a Value>,
}

#[derive(Debug, Deserialize)]
struct Sortie {
    ok: bool,
    #[serde(default)]
    reponse: Value,
    #[serde(default)]
    erreur: String,
    #[serde(default)]
    modele: String,
}

/// Lit la sortie de l'extension : (réponse en texte JSON, modèle qui a répondu).
fn lire_sortie(ext: &Extension, octets: &[u8]) -> Result<(String, String), String> {
    let texte = String::from_utf8_lossy(octets);
    // Seule la dernière ligne non vide compte : une extension bavarde peut écrire avant.
    let ligne = texte.lines().rev().find(|l| !l.trim().is_empty()).unwrap_or("");
    let sortie: Sortie =
        serde_json::from_str(ligne.trim()).map_err(|_| format!("l'extension « {} » a renvoyé une réponse illisible", ext.nom))?;
    if !sortie.ok {
        let erreur = sortie.erreur.trim();
        return Err(if erreur.is_empty() { format!("échec de l'extension « {} »", ext.nom) } else { erreur.to_string() });
    }
    // Une réponse donnée en texte (JSON dans une chaîne) est acceptée aussi.
    let reponse = match sortie.reponse {
        Value::String(s) => s,
        Value::Null => return Err(format!("l'extension « {} » n'a rien répondu", ext.nom)),
        v => v.to_string(),
    };
    Ok((reponse, sortie.modele.trim().to_string()))
}

/// Envoie une demande à l'extension et attend sa réponse. Le programme est arrêté si la tâche est
/// abandonnée (bouton Arrêter) ou s'il dépasse le délai.
pub async fn appeler(ext: &Extension, demande: &Demande<'_>) -> Result<(String, String), String> {
    let mut commande = tokio::process::Command::new(&ext.commande[0]);
    commande
        .args(&ext.commande[1..])
        .current_dir(&ext.dossier)
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .kill_on_drop(true);
    #[cfg(windows)]
    {
        // Pas de fenêtre de console qui s'ouvre pendant le traitement.
        const CREATE_NO_WINDOW: u32 = 0x0800_0000;
        commande.creation_flags(CREATE_NO_WINDOW);
    }
    let mut enfant = commande
        .spawn()
        .map_err(|e| format!("impossible de lancer l'extension « {} » ({} : {e})", ext.nom, ext.commande[0]))?;
    let entree = serde_json::to_vec(demande).map_err(|e| e.to_string())?;
    let mut stdin = enfant.stdin.take().ok_or("entrée de l'extension indisponible")?;
    let mut stdout = enfant.stdout.take().ok_or("sortie de l'extension indisponible")?;
    let mut stderr = enfant.stderr.take().ok_or("sortie d'erreur de l'extension indisponible")?;
    let travail = async {
        stdin.write_all(&entree).await.map_err(|e| e.to_string())?;
        drop(stdin);
        let (mut sortie, mut erreurs) = (Vec::new(), Vec::new());
        let (a, b) = tokio::join!(stdout.read_to_end(&mut sortie), stderr.read_to_end(&mut erreurs));
        a.and(b).map_err(|e| e.to_string())?;
        let statut = enfant.wait().await.map_err(|e| e.to_string())?;
        Ok::<_, String>((statut, sortie, erreurs))
    };
    let (statut, sortie, erreurs) = tokio::time::timeout(DELAI, travail)
        .await
        .map_err(|_| format!("l'extension « {} » n'a pas répondu en {} minutes", ext.nom, DELAI.as_secs() / 60))??;
    match lire_sortie(ext, &sortie) {
        Ok(r) => Ok(r),
        Err(e) if !statut.success() => {
            let detail = String::from_utf8_lossy(&erreurs);
            let detail: String = detail.trim().chars().rev().take(300).collect::<Vec<_>>().into_iter().rev().collect();
            Err(if detail.is_empty() { e } else { format!("{e} : {detail}") })
        }
        Err(e) => Err(e),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn installer(donnees: &Path, id: &str, manifeste: &str) {
        let d = dossier(donnees).join(id);
        std::fs::create_dir_all(&d).unwrap();
        std::fs::write(d.join(MANIFESTE), manifeste).unwrap();
    }

    #[test]
    fn extensions_listees_et_trouvees() {
        let donnees = std::env::temp_dir().join(format!("revisio-extensions-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&donnees);
        assert!(lister(&donnees).is_empty());
        installer(&donnees, "redacteur", r#"{"nom":"Rédacteur","taches":["photos"],"commande":["node","extension.mjs"]}"#);
        installer(&donnees, "cassee", "pas du json");
        installer(&donnees, "sans-commande", r#"{"nom":"Rien","taches":["photos"],"commande":[]}"#);
        let liste = lister(&donnees);
        assert_eq!(liste.len(), 1);
        assert_eq!(liste[0].id, "redacteur");
        assert!(trouver(&donnees, "redacteur", "photos").is_some());
        assert!(trouver(&donnees, "redacteur", "podcasts").is_none());
        // La commande n'est jamais envoyée à l'interface.
        assert!(!serde_json::to_string(&liste[0]).unwrap().contains("extension.mjs"));
        let _ = std::fs::remove_dir_all(&donnees);
    }

    /// Vraie extension (petit programme Node qui renvoie la tâche et le nombre d'images reçues).
    #[tokio::test]
    async fn extension_appelee() {
        if std::process::Command::new("node").arg("--version").output().is_err() {
            return; // Node absent : rien à essayer.
        }
        let donnees = std::env::temp_dir().join(format!("revisio-extensions-appel-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&donnees);
        installer(&donnees, "echo", r#"{"nom":"Écho","taches":["photos"],"commande":["node","echo.mjs"]}"#);
        std::fs::write(
            dossier(&donnees).join("echo").join("echo.mjs"),
            "let t='';for await(const c of process.stdin)t+=c;const d=JSON.parse(t);\
             if(d.consigne==='erreur'){console.log(JSON.stringify({ok:false,erreur:'Non connecté.'}));process.exit(1)}\
             console.log(JSON.stringify({ok:true,modele:'test',reponse:{tache:d.tache,images:d.images.length}}))",
        )
        .unwrap();
        let ext = trouver(&donnees, "echo", "photos").unwrap();
        let demande = |consigne| Demande {
            version: VERSION_PROTOCOLE,
            tache: "photos",
            systeme: "",
            consigne,
            images: vec!["a.jpg".into()],
            schema: None,
        };
        let (texte, modele) = appeler(&ext, &demande("bonjour")).await.unwrap();
        assert_eq!(serde_json::from_str::<Value>(&texte).unwrap(), serde_json::json!({"tache": "photos", "images": 1}));
        assert_eq!(modele, "test");
        assert_eq!(appeler(&ext, &demande("erreur")).await.unwrap_err(), "Non connecté.");
        let _ = std::fs::remove_dir_all(&donnees);
    }

    #[test]
    fn sortie_lue() {
        let ext: Extension = serde_json::from_str(r#"{"nom":"Test"}"#).unwrap();
        let ok = lire_sortie(&ext, b"bavardage\n{\"ok\":true,\"reponse\":{\"a\":1},\"modele\":\"m\"}\n").unwrap();
        assert_eq!(ok, (r#"{"a":1}"#.to_string(), "m".to_string()));
        let texte = lire_sortie(&ext, br#"{"ok":true,"reponse":"{\"a\":2}"}"#).unwrap();
        assert_eq!(texte.0, r#"{"a":2}"#);
        assert_eq!(lire_sortie(&ext, br#"{"ok":false,"erreur":"Pas connecte."}"#).unwrap_err(), "Pas connecte.");
        assert!(lire_sortie(&ext, b"").is_err());
        assert!(lire_sortie(&ext, br#"{"ok":true}"#).is_err());
    }
}
