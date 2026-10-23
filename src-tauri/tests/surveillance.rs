//! La surveillance déclenche la réindexation quand une note change, et ignore `.obsidian`.
//! Le test travaille sur une copie temporaire (jamais sur le vrai vault).

use std::fs;
use std::sync::mpsc;
use std::time::Duration;

use revisio_lib::vault;

#[test]
fn signale_les_notes_modifiees_mais_pas_obsidian() {
    let dossier = std::env::temp_dir().join(format!("revisio-surveillance-{}", std::process::id()));
    let _ = fs::remove_dir_all(&dossier);
    fs::create_dir_all(dossier.join(".obsidian")).unwrap();
    fs::create_dir_all(dossier.join("Maths")).unwrap();

    let (envoi, reception) = mpsc::channel();
    let surveillance = vault::surveiller(&dossier, move || {
        let _ = envoi.send(());
    })
    .unwrap();

    // Un changement dans .obsidian (ex. Obsidian qui enregistre son espace de travail) : ignoré.
    fs::write(dossier.join(".obsidian").join("workspace.json"), "{}").unwrap();
    assert!(
        reception.recv_timeout(Duration::from_secs(3)).is_err(),
        "un changement dans .obsidian ne doit pas relancer l'indexation"
    );

    // Une nouvelle note : signalée.
    fs::write(dossier.join("Maths").join("Nouvelle note.md"), "# Test").unwrap();
    assert!(
        reception.recv_timeout(Duration::from_secs(5)).is_ok(),
        "la nouvelle note aurait dû être signalée"
    );

    drop(surveillance);
    fs::remove_dir_all(&dossier).unwrap();
}
