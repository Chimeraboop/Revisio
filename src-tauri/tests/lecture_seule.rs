//! Tests de la règle n°1 : Révisio ne modifie JAMAIS le vault.
//!
//! 1. On prend une « photo » du vault (chaque fichier et dossier, avec taille,
//!    date de modification et empreinte SHA-256 du contenu), on indexe, on
//!    reprend une photo : les deux doivent être identiques.
//! 2. On relit le code source de `vault.rs` (le seul module qui accède au vault)
//!    et on vérifie qu'il n'appelle aucune fonction qui écrit sur le disque.

use std::collections::BTreeMap;
use std::fs;
use std::path::{Path, PathBuf};

use revisio_lib::{cartes, db, indexer, settings, vault};
use sha2::{Digest, Sha256};

type Photo = BTreeMap<String, (bool, u64, u128, Vec<u8>)>;

/// Photo de TOUT le dossier (y compris .obsidian, .git, _Templates…).
fn photo(racine: &Path) -> Photo {
    let mut p = Photo::new();
    for e in walkdir::WalkDir::new(racine).follow_links(false) {
        let e = e.expect("parcours du vault");
        let meta = e.metadata().unwrap();
        let modifie = meta
            .modified()
            .unwrap()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap()
            .as_nanos();
        let empreinte = if meta.is_file() {
            Sha256::digest(fs::read(e.path()).unwrap()).to_vec()
        } else {
            vec![]
        };
        let rel = e.path().strip_prefix(racine).unwrap().to_string_lossy().into_owned();
        p.insert(rel, (meta.is_dir(), meta.len(), modifie, empreinte));
    }
    p
}

fn copier_dossier(source: &Path, cible: &Path) {
    fs::create_dir_all(cible).unwrap();
    for e in fs::read_dir(source).unwrap() {
        let e = e.unwrap();
        let dest = cible.join(e.file_name());
        if e.file_type().unwrap().is_dir() {
            copier_dossier(&e.path(), &dest);
        } else {
            fs::copy(e.path(), &dest).unwrap();
        }
    }
}

fn changer_lecture_seule(racine: &Path, lecture_seule: bool) {
    for e in walkdir::WalkDir::new(racine) {
        let e = e.unwrap();
        if e.file_type().is_file() {
            let mut perms = e.metadata().unwrap().permissions();
            perms.set_readonly(lecture_seule);
            fs::set_permissions(e.path(), perms).unwrap();
        }
    }
}

fn vault_exemple() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR")).join("tests").join("vault_exemple")
}

#[test]
fn indexer_le_vault_exemple_ne_le_modifie_pas() {
    // Copie du vault d'exemple dans un dossier temporaire, fichiers en lecture seule :
    // toute tentative d'écriture échouerait en plus d'être détectée par la photo.
    let copie = std::env::temp_dir().join(format!("revisio-vault-exemple-{}", std::process::id()));
    let _ = fs::remove_dir_all(&copie);
    copier_dossier(&vault_exemple(), &copie);
    changer_lecture_seule(&copie, true);

    let avant = photo(&copie);
    // La surveillance des changements est active pendant toute l'indexation, comme dans l'app.
    let surveillance = vault::surveiller(&copie, || {}).unwrap();
    let mut conn = db::ouvrir_en_memoire().unwrap();
    let rapport = indexer::indexer(&mut conn, &copie).unwrap();
    // Deuxième passage : rien ne doit être relu ni changé.
    let rapport2 = indexer::indexer(&mut conn, &copie).unwrap();
    std::thread::sleep(std::time::Duration::from_millis(500));
    drop(surveillance);
    let apres = photo(&copie);

    changer_lecture_seule(&copie, false);
    fs::remove_dir_all(&copie).unwrap();

    assert_eq!(avant, apres, "le vault a été modifié par l'indexation !");
    assert!(rapport.erreurs.is_empty(), "{:?}", rapport.erreurs);
    // 6 notes : _Templates et .obsidian sont ignorés.
    assert_eq!(rapport.total, 6, "{rapport:?}");
    assert_eq!(rapport.ajoutees, 6);
    assert_eq!(rapport.fichiers_annexes, 1); // _Assets/pixel.png
    assert_eq!((rapport2.ajoutees, rapport2.modifiees, rapport2.supprimees), (0, 0, 0));

    let genres: Vec<(String, String)> = conn
        .prepare("SELECT titre, genre FROM notes ORDER BY chemin")
        .unwrap()
        .query_map([], |r| Ok((r.get(0)?, r.get(1)?)))
        .unwrap()
        .map(Result::unwrap)
        .collect();
    assert!(genres.contains(&("La méiose (schéma)".into(), "schema".into())));
    assert!(genres.contains(&("Fiche de synthèse - Chapitre 3 La méiose".into(), "fiche_synthese".into())));
    assert!(!genres.iter().any(|(t, _)| t == "Template Cours"));
}

#[test]
fn indexer_le_vrai_vault_ne_le_modifie_pas() {
    // Vault indiqué par REVISIO_VAULT, sinon le dernier vault ouvert dans Obsidian sur ce PC.
    let chemin = std::env::var("REVISIO_VAULT")
        .ok()
        .or_else(|| settings::vaults_obsidian().into_iter().next().map(|v| v.chemin))
        .unwrap_or_default();
    let racine = PathBuf::from(chemin);
    if !racine.is_dir() {
        eprintln!("Vault réel absent ({}) : test ignoré.", racine.display());
        return;
    }
    // `.obsidian-mcp` appartient au connecteur « Obsidian MCP » (programme externe) qui y gère
    // ses verrous à tout moment ; Révisio ne l'ouvre jamais (dossier ignoré, voir vault.rs).
    // Le test du vault d'exemple, lui, compare absolument tout.
    let sans_outils_externes = |p: Photo| -> Photo {
        p.into_iter().filter(|(chemin, _)| !chemin.starts_with(".obsidian-mcp")).collect()
    };
    let avant = sans_outils_externes(photo(&racine));
    let mut conn = db::ouvrir_en_memoire().unwrap();
    let rapport = indexer::indexer(&mut conn, &racine).unwrap();
    cartes::synchroniser(&mut conn, 0).unwrap();
    let apres = sans_outils_externes(photo(&racine));

    assert!(rapport.total > 0);
    // Si OneDrive ou Obsidian modifient le vault pendant le test, il peut échouer :
    // relancer pour confirmer.
    assert_eq!(avant.len(), apres.len(), "des fichiers ont été créés ou supprimés dans le vault !");
    for (chemin, etat) in &avant {
        assert_eq!(Some(etat), apres.get(chemin), "fichier modifié dans le vault : {chemin}");
    }
}

#[test]
fn vault_rs_n_appelle_aucune_fonction_d_ecriture() {
    let source = include_str!("../src/vault.rs");
    let interdits = [
        "write", "create", "remove", "rename", "copy", "set_permissions", "set_len", "truncate", "append",
        "set_modified", "set_times", "hard_link", "symlink", "set_readonly",
    ];
    for (numero, ligne) in source.lines().enumerate() {
        let code = ligne.split("//").next().unwrap_or("").to_lowercase();
        for mot in interdits {
            assert!(
                !code.contains(mot),
                "vault.rs ligne {} utilise « {mot} » : interdit (lecture seule) → {ligne}",
                numero + 1
            );
        }
    }
    assert!(source.contains(".read(true)"), "vault.rs doit ouvrir les fichiers avec .read(true)");
}
