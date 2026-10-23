//! Espace web : des onglets de navigation (webviews enfants) dans la fenêtre de Révisio.
//!
//! Sécurité :
//! - les pages web n'ont AUCUN accès aux commandes de Révisio (les permissions ne valent que pour
//!   le contenu local de l'app) ; un onglet ne peut aller que sur des adresses http(s) publiques,
//!   jamais sur l'app elle-même ni sur un fichier du disque (donc jamais dans le vault) ;
//! - les téléchargements vont dans le dossier Téléchargements de Windows ;
//! - Révisio peut lire le texte sélectionné dans la page (bouton « Enregistrer la sélection »).

use std::sync::atomic::{AtomicU32, Ordering};
use std::sync::Mutex;

use serde::{Deserialize, Serialize};
use tauri::webview::{DownloadEvent, NewWindowResponse, PageLoadEvent, WebviewBuilder};
use tauri::{AppHandle, Emitter, LogicalPosition, LogicalSize, Manager, Url, WebviewUrl};

/// Zone de la fenêtre où s'affiche la page (en pixels logiques, mesurée par l'interface).
#[derive(Debug, Clone, Copy, Deserialize, PartialEq)]
pub struct Zone {
    pub x: f64,
    pub y: f64,
    pub largeur: f64,
    pub hauteur: f64,
}

#[derive(Debug, Clone, Serialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct Onglet {
    pub id: u32,
    pub url: String,
    pub titre: String,
    pub actif: bool,
}

#[derive(Default)]
pub struct EspaceWeb {
    onglets: Mutex<Vec<Onglet>>,
    prochain: AtomicU32,
    zone: Mutex<Option<Zone>>,
}

fn label(id: u32) -> String {
    format!("web-{id}")
}

/// Adresse tapée → URL : « wikipedia.org » → https://wikipedia.org ; une phrase → recherche Google.
pub fn vers_url(saisie: &str) -> Option<Url> {
    let s = saisie.trim();
    if s.is_empty() {
        return None;
    }
    let candidate = if s.contains("://") {
        s.to_string()
    } else if !s.contains(' ') && s.contains('.') {
        format!("https://{s}")
    } else {
        let mut u = Url::parse("https://www.google.com/search").ok()?;
        u.query_pairs_mut().append_pair("q", s);
        return Some(u);
    };
    Url::parse(&candidate).ok().filter(adresse_autorisee)
}

/// Seulement des pages web publiques : ni l'app (qui a accès aux commandes), ni fichiers locaux.
pub fn adresse_autorisee(u: &Url) -> bool {
    if u.as_str() == "about:blank" {
        return true;
    }
    if !matches!(u.scheme(), "http" | "https") {
        return false;
    }
    let hote = u.host_str().unwrap_or("").to_lowercase();
    !(hote.is_empty() || hote == "tauri.localhost" || hote == "ipc.localhost" || hote.ends_with(".tauri.localhost"))
}

impl EspaceWeb {
    pub fn liste(&self) -> Vec<Onglet> {
        self.onglets.lock().unwrap().clone()
    }

    fn annoncer(&self, app: &AppHandle) {
        let _ = app.emit_to("main", "web-onglets", self.liste());
    }

    fn modifier(&self, app: &AppHandle, id: u32, f: impl FnOnce(&mut Onglet)) {
        if let Some(o) = self.onglets.lock().unwrap().iter_mut().find(|o| o.id == id) {
            f(o);
        }
        self.annoncer(app);
    }

    /// Ouvre un onglet (il devient l'onglet actif).
    pub fn ouvrir(&self, app: &AppHandle, saisie: &str, zone: Zone) -> Result<u32, String> {
        let url = vers_url(saisie).ok_or("Adresse invalide (seules les pages web http/https sont autorisées).")?;
        let id = self.prochain.fetch_add(1, Ordering::SeqCst) + 1;
        *self.zone.lock().unwrap() = Some(zone);
        let fenetre = app.get_window("main").ok_or("Fenêtre principale introuvable.")?;

        let (a1, a2, a3, a4) = (app.clone(), app.clone(), app.clone(), app.clone());
        let builder = WebviewBuilder::new(label(id), WebviewUrl::External(url.clone()))
            .on_navigation(move |u| {
                let ok = adresse_autorisee(u);
                if ok {
                    let web = a1.state::<EspaceWeb>();
                    web.modifier(&a1, id, |o| o.url = u.to_string());
                }
                ok
            })
            .on_page_load(move |webview, charge| {
                if charge.event() != PageLoadEvent::Finished {
                    return;
                }
                let app = a2.clone();
                let url = charge.url().to_string();
                let _ = webview.eval_with_callback("document.title", move |titre| {
                    let titre: String = serde_json::from_str(&titre).unwrap_or_default();
                    let web = app.state::<EspaceWeb>();
                    web.modifier(&app, id, |o| {
                        o.url = url.clone();
                        o.titre = titre.trim().to_string();
                    });
                });
            })
            .on_new_window(move |u, _| {
                // « Ouvrir dans un nouvel onglet » : l'interface ouvre un onglet Révisio.
                if adresse_autorisee(&u) {
                    let _ = a3.emit_to("main", "web-nouvel-onglet", u.to_string());
                }
                NewWindowResponse::Deny
            })
            .on_download(move |_, evenement| {
                if let DownloadEvent::Finished { path, success, .. } = evenement {
                    let nom = path.and_then(|p| p.file_name().map(|n| n.to_string_lossy().to_string()));
                    let _ = a4.emit_to("main", "web-telechargement", (success, nom));
                }
                true
            });
        fenetre
            .add_child(builder, LogicalPosition::new(zone.x, zone.y), LogicalSize::new(zone.largeur, zone.hauteur))
            .map_err(|e| format!("Impossible d'ouvrir l'onglet : {e}"))?;
        {
            let mut onglets = self.onglets.lock().unwrap();
            onglets.iter_mut().for_each(|o| o.actif = false);
            onglets.push(Onglet { id, url: url.to_string(), titre: String::new(), actif: true });
        }
        self.montrer_seulement(app, Some(id));
        self.annoncer(app);
        Ok(id)
    }

    /// Affiche l'onglet `id` dans la zone (et masque les autres) ; `None` masque tout.
    pub fn montrer_seulement(&self, app: &AppHandle, id: Option<u32>) {
        let zone = *self.zone.lock().unwrap();
        for o in self.onglets.lock().unwrap().iter() {
            let Some(w) = app.get_webview(&label(o.id)) else { continue };
            if Some(o.id) == id {
                if let Some(z) = zone {
                    let _ = w.set_position(LogicalPosition::new(z.x, z.y));
                    let _ = w.set_size(LogicalSize::new(z.largeur, z.hauteur));
                }
                let _ = w.show();
            } else {
                let _ = w.hide();
            }
        }
    }

    /// Page d'accueil de l'espace web : plus aucun onglet actif (ils restent ouverts).
    pub fn accueil(&self, app: &AppHandle) {
        self.onglets.lock().unwrap().iter_mut().for_each(|o| o.actif = false);
        self.montrer_seulement(app, None);
        self.annoncer(app);
    }

    pub fn activer(&self, app: &AppHandle, id: u32) {
        self.onglets.lock().unwrap().iter_mut().for_each(|o| o.actif = o.id == id);
        self.montrer_seulement(app, Some(id));
        self.annoncer(app);
    }

    pub fn actif(&self) -> Option<u32> {
        self.onglets.lock().unwrap().iter().find(|o| o.actif).map(|o| o.id)
    }

    pub fn placer(&self, app: &AppHandle, zone: Zone) {
        *self.zone.lock().unwrap() = Some(zone);
        self.montrer_seulement(app, self.actif());
    }

    pub fn fermer(&self, app: &AppHandle, id: u32) {
        if let Some(w) = app.get_webview(&label(id)) {
            let _ = w.close();
        }
        let suivant = {
            let mut onglets = self.onglets.lock().unwrap();
            let position = onglets.iter().position(|o| o.id == id);
            let etait_actif = position.is_some_and(|p| onglets[p].actif);
            if let Some(p) = position {
                onglets.remove(p);
            }
            if etait_actif {
                onglets.last_mut().map(|o| {
                    o.actif = true;
                    o.id
                })
            } else {
                None
            }
        };
        if suivant.is_some() {
            self.montrer_seulement(app, suivant);
        }
        self.annoncer(app);
    }

    pub fn webview(&self, app: &AppHandle, id: u32) -> Result<tauri::Webview, String> {
        app.get_webview(&label(id)).ok_or_else(|| "Onglet introuvable.".into())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn adresses_tapees() {
        assert_eq!(vers_url("fr.wikipedia.org").unwrap().as_str(), "https://fr.wikipedia.org/");
        assert_eq!(vers_url("https://gemini.google.com/app").unwrap().as_str(), "https://gemini.google.com/app");
        let recherche = vers_url("méiose prophase I").unwrap();
        assert_eq!(recherche.host_str(), Some("www.google.com"));
        assert!(recherche.as_str().contains("q=m%C3%A9iose+prophase+I"));
        assert!(vers_url("   ").is_none());
    }

    #[test]
    fn adresses_interdites() {
        for interdite in [
            "file:///C:/Users/eleve/Documents/MonVault/note.md",
            "http://tauri.localhost/index.html",
            "https://ipc.localhost/infos_app",
            "javascript:alert(1)",
        ] {
            assert!(!adresse_autorisee(&Url::parse(interdite).unwrap()), "{interdite}");
            // Tapée dans la barre d'adresse : refusée, ou transformée en simple recherche Google.
            assert!(vers_url(interdite).is_none_or(|u| u.host_str() == Some("www.google.com")), "{interdite}");
        }
        assert!(adresse_autorisee(&Url::parse("http://example.com").unwrap()));
    }
}
