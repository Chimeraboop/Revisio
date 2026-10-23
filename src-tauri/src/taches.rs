//! Générations par l'IA en cours, pour pouvoir les interrompre (bouton « Arrêter » de l'interface).
//!
//! L'interface donne un identifiant à chaque génération ; `annuler(id)` arrête la tâche tout de suite :
//! la requête en cours vers Gemini ou Ollama est abandonnée (comme un `task.cancel()` d'asyncio en Python).
//! Ce qui avait déjà été enregistré (un lot de questions, un morceau de podcast…) reste valable.

use std::collections::HashMap;
use std::future::Future;
use std::sync::Mutex;

use tokio::sync::oneshot;

/// Message renvoyé à l'interface quand l'élève a interrompu la génération.
pub const INTERROMPUE: &str = "Génération interrompue.";

#[derive(Default)]
pub struct Taches {
    en_cours: Mutex<HashMap<String, oneshot::Sender<()>>>,
}

impl Taches {
    /// Exécute `travail` ; s'arrête dès que `annuler(id)` est appelé. Sans identifiant, rien ne change.
    pub async fn annulable<T>(
        &self,
        id: Option<String>,
        travail: impl Future<Output = Result<T, String>>,
    ) -> Result<T, String> {
        let Some(id) = id else { return travail.await };
        let (envoi, arret) = oneshot::channel();
        self.en_cours.lock().unwrap().insert(id.clone(), envoi);
        let _menage = Menage { taches: self, id };
        tokio::select! {
            resultat = travail => resultat,
            _ = arret => Err(INTERROMPUE.into()),
        }
    }

    /// Demande l'arrêt d'une tâche ; `false` si elle était déjà terminée.
    pub fn annuler(&self, id: &str) -> bool {
        match self.en_cours.lock().unwrap().remove(id) {
            Some(envoi) => envoi.send(()).is_ok(),
            None => false,
        }
    }

    pub fn nombre(&self) -> usize {
        self.en_cours.lock().unwrap().len()
    }
}

/// Retire la tâche du registre quand elle se termine (normalement, par erreur ou interrompue).
struct Menage<'a> {
    taches: &'a Taches,
    id: String,
}

impl Drop for Menage<'_> {
    fn drop(&mut self) {
        self.taches.en_cours.lock().unwrap().remove(&self.id);
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::time::Duration;

    #[tokio::test(flavor = "current_thread", start_paused = true)]
    async fn une_tache_interrompue_s_arrete_aussitot() {
        let taches = std::sync::Arc::new(Taches::default());
        let t = taches.clone();
        let longue = tokio::spawn(async move {
            t.annulable(Some("a".into()), async {
                tokio::time::sleep(Duration::from_secs(3600)).await;
                Ok::<_, String>(42)
            })
            .await
        });
        tokio::task::yield_now().await;
        assert_eq!(taches.nombre(), 1);
        assert!(taches.annuler("a"));
        assert_eq!(longue.await.unwrap(), Err(INTERROMPUE.to_string()));
        assert_eq!(taches.nombre(), 0);
        assert!(!taches.annuler("a"), "déjà terminée");
    }

    #[tokio::test(flavor = "current_thread")]
    async fn une_tache_terminee_quitte_le_registre() {
        let taches = Taches::default();
        assert_eq!(taches.annulable(Some("b".into()), async { Ok::<_, String>(1) }).await, Ok(1));
        assert_eq!(taches.annulable(None, async { Err::<i32, _>("x".to_string()) }).await, Err("x".into()));
        assert_eq!(taches.nombre(), 0);
    }
}
