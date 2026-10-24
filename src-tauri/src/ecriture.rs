//! SEUL module de Révisio qui écrit dans le vault, et seulement quand l'élève l'a permis
//! (Réglages → Écriture dans le vault ; désactivé par défaut).
//!
//! Garde-fous :
//! - uniquement des notes `.md`, jamais dans un dossier caché (`.obsidian`, `.git`…) ni dans
//!   `_Templates`, jamais un dessin `.excalidraw.md`, jamais en dehors du vault ;
//! - une création ne remplace jamais un fichier existant ;
//! - une modification vérifie que la note n'a pas changé depuis qu'on l'a lue ;
//! - chaque écriture est notée dans `historique_vault` avec le contenu d'avant : elle peut
//!   toujours être annulée (si la note n'a pas été retouchée depuis).
//!
//! La lecture du vault reste l'affaire de `vault.rs` (lecture seule).

use std::fs::OpenOptions;
use std::io::Write;
use std::path::{Component, Path, PathBuf};

use rusqlite::{params, Connection, OptionalExtension};
use serde::Serialize;

use crate::paths::est_dans;
use crate::vault;

pub const DESACTIVEE: &str =
    "L'écriture dans le vault est désactivée. Tu peux l'autoriser dans Réglages, Écriture dans le vault.";

/// Caractères interdits dans un nom de fichier Windows (plus « # ^ [ ] » qui gênent les liens Obsidian).
const INTERDITS: &[char] = &['<', '>', ':', '"', '/', '\\', '|', '?', '*', '#', '^', '[', ']'];

/// Transforme un titre de note en nom de fichier valide (sans « .md »).
pub fn nom_de_fichier(titre: &str) -> String {
    let propre: String = titre
        .chars()
        .map(|c| if INTERDITS.contains(&c) || c.is_control() { ' ' } else { c })
        .collect();
    let mut nom = propre.split_whitespace().collect::<Vec<_>>().join(" ");
    nom = nom.trim_matches(|c: char| c == '.' || c == ' ').to_string();
    if nom.chars().count() > 120 {
        nom = nom.chars().take(120).collect::<String>().trim_end().to_string();
    }
    nom
}

/// Chemin absolu d'une note où Révisio a le droit d'écrire, ou la raison du refus.
pub fn chemin_ecrivable(racine: &Path, relatif: &str) -> Result<PathBuf, String> {
    let refus = |raison: &str| Err(format!("Écriture refusée ({raison}) : {relatif}"));
    let chemin = Path::new(relatif);
    if relatif.is_empty() || chemin.components().any(|c| !matches!(c, Component::Normal(_))) {
        return refus("chemin invalide");
    }
    let morceaux: Vec<String> = chemin.components().map(|c| c.as_os_str().to_string_lossy().into_owned()).collect();
    if morceaux.iter().any(|m| vault::dossier_ignore(m)) {
        return refus("dossier réservé à Obsidian ou aux modèles");
    }
    if morceaux.iter().any(|m| m.chars().any(|c| INTERDITS.contains(&c) || c.is_control()) || m.ends_with(' ')) {
        return refus("caractère interdit dans le nom");
    }
    let minuscule = relatif.to_lowercase();
    if !minuscule.ends_with(".md") {
        return refus("seules les notes .md sont modifiables");
    }
    if minuscule.ends_with(".excalidraw.md") {
        return refus("les dessins Excalidraw ne sont jamais modifiés");
    }
    let complet = racine.join(chemin);
    if !est_dans(&complet, racine) || complet == racine {
        return refus("hors du vault");
    }
    Ok(complet)
}

/// Une écriture faite par Révisio (pour le journal et l'annulation).
#[derive(Debug, Clone, Serialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct EntreeHistorique {
    pub id: i64,
    pub date: i64,
    pub chemin: String,
    pub genre: String,
    pub origine: String,
    pub lot_id: Option<i64>,
    pub raison: Option<String>,
    pub annulee_le: Option<i64>,
}

/// Infos communes à une écriture.
pub struct Contexte<'a> {
    pub racine: &'a Path,
    pub autorisee: bool,
    pub origine: &'a str,
    pub lot_id: Option<i64>,
    pub raison: Option<&'a str>,
    pub maintenant: i64,
}

/// Crée une nouvelle note (et les dossiers manquants). Ne remplace jamais un fichier existant.
pub fn creer(conn: &Connection, ctx: &Contexte, relatif: &str, contenu: &str) -> Result<i64, String> {
    if !ctx.autorisee {
        return Err(DESACTIVEE.into());
    }
    let complet = chemin_ecrivable(ctx.racine, relatif)?;
    if complet.exists() {
        return Err(format!("Une note porte déjà ce nom : {relatif}"));
    }
    // Dossiers manquants, créés un par un pour pouvoir les retirer à l'annulation.
    let mut dossiers_crees: Vec<String> = Vec::new();
    let morceaux: Vec<&str> = relatif.split('/').collect();
    for i in 1..morceaux.len() {
        let dossier = morceaux[..i].join("/");
        let absolu = ctx.racine.join(&dossier);
        if !absolu.is_dir() {
            std::fs::create_dir(&absolu).map_err(|e| format!("Création du dossier {dossier} : {e}"))?;
            dossiers_crees.push(dossier);
        }
    }
    let mut fichier = OpenOptions::new()
        .write(true)
        .create_new(true)
        .open(&complet)
        .map_err(|e| format!("Création de {relatif} : {e}"))?;
    fichier.write_all(contenu.as_bytes()).map_err(|e| format!("Écriture de {relatif} : {e}"))?;
    drop(fichier);
    conn.execute(
        "INSERT INTO historique_vault(date, chemin, genre, avant, apres, origine, lot_id, raison, dossiers_crees)
         VALUES (?1, ?2, 'creation', NULL, ?3, ?4, ?5, ?6, ?7)",
        params![
            ctx.maintenant,
            relatif,
            contenu.as_bytes(),
            ctx.origine,
            ctx.lot_id,
            ctx.raison,
            serde_json::to_string(&dossiers_crees).unwrap()
        ],
    )
    .map_err(|e| e.to_string())?;
    Ok(conn.last_insert_rowid())
}

/// Remplace le contenu d'une note existante. `encore_pareille` reçoit le contenu actuel
/// (octets exacts) et dit s'il correspond à ce que l'élève ou l'IA avait lu : sinon, refus.
pub fn modifier(
    conn: &Connection,
    ctx: &Contexte,
    relatif: &str,
    nouveau: &str,
    encore_pareille: &dyn Fn(&[u8]) -> bool,
) -> Result<i64, String> {
    if !ctx.autorisee {
        return Err(DESACTIVEE.into());
    }
    let complet = chemin_ecrivable(ctx.racine, relatif)?;
    if !complet.is_file() {
        return Err(format!("Note introuvable : {relatif}"));
    }
    let avant = vault::lire_octets(ctx.racine, relatif)?;
    if !encore_pareille(&avant) {
        return Err("La note a été modifiée entre-temps (dans un autre programme ?). Recharge-la avant de l'enregistrer.".into());
    }
    if avant == nouveau.as_bytes() {
        return Err("Aucun changement à enregistrer.".into());
    }
    // Sauvegarde d'abord, écriture ensuite : en cas de coupure, l'ancien contenu est déjà à l'abri.
    conn.execute(
        "INSERT INTO historique_vault(date, chemin, genre, avant, apres, origine, lot_id, raison)
         VALUES (?1, ?2, 'modification', ?3, ?4, ?5, ?6, ?7)",
        params![ctx.maintenant, relatif, avant, nouveau.as_bytes(), ctx.origine, ctx.lot_id, ctx.raison],
    )
    .map_err(|e| e.to_string())?;
    let id = conn.last_insert_rowid();
    if let Err(e) = std::fs::write(&complet, nouveau.as_bytes()) {
        conn.execute("DELETE FROM historique_vault WHERE id = ?1", [id]).ok();
        return Err(format!("Écriture de {relatif} : {e}"));
    }
    Ok(id)
}

/// Annule une écriture : supprime la note créée, ou remet le contenu d'avant.
/// Refusé si la note a été retouchée depuis (pour ne jamais perdre ces changements).
/// Possible même si l'écriture a été désactivée ensuite : on ne fait que revenir en arrière.
pub fn annuler(conn: &Connection, racine: &Path, id: i64, maintenant: i64) -> Result<String, String> {
    let ligne: Option<(String, String, Option<Vec<u8>>, Vec<u8>, String, Option<i64>)> = conn
        .query_row(
            "SELECT chemin, genre, avant, apres, dossiers_crees, annulee_le FROM historique_vault WHERE id = ?1",
            [id],
            |r| Ok((r.get(0)?, r.get(1)?, r.get(2)?, r.get(3)?, r.get(4)?, r.get(5)?)),
        )
        .optional()
        .map_err(|e| e.to_string())?;
    let (chemin, genre, avant, apres, dossiers, annulee) = ligne.ok_or("Modification introuvable.")?;
    if annulee.is_some() {
        return Err("Cette modification est déjà annulée.".into());
    }
    let complet = chemin_ecrivable(racine, &chemin)?;
    let marquer = || {
        conn.execute("UPDATE historique_vault SET annulee_le = ?1 WHERE id = ?2", params![maintenant, id])
            .map_err(|e| e.to_string())
    };
    if genre == "creation" && !complet.exists() {
        marquer()?;
        return Ok(format!("{chemin} n'existait déjà plus."));
    }
    let actuel = vault::lire_octets(racine, &chemin)?;
    if actuel != apres {
        return Err(format!(
            "{chemin} a été modifiée depuis : l'annulation effacerait ces changements. Corrige-la plutôt \
             avec le bouton Modifier."
        ));
    }
    if genre == "creation" {
        std::fs::remove_file(&complet).map_err(|e| format!("Suppression de {chemin} : {e}"))?;
        let dossiers: Vec<String> = serde_json::from_str(&dossiers).unwrap_or_default();
        for dossier in dossiers.iter().rev() {
            // Seulement les dossiers créés pour cette note, et seulement s'ils sont restés vides.
            if chemin_dossier_sur(racine, dossier) {
                let _ = std::fs::remove_dir(racine.join(dossier));
            }
        }
        marquer()?;
        Ok(format!("Note supprimée : {chemin}"))
    } else {
        let avant = avant.ok_or("Contenu d'origine introuvable.")?;
        std::fs::write(&complet, &avant).map_err(|e| format!("Écriture de {chemin} : {e}"))?;
        marquer()?;
        Ok(format!("Note remise comme avant : {chemin}"))
    }
}

/// Un dossier relatif à l'intérieur du vault, hors dossiers réservés.
fn chemin_dossier_sur(racine: &Path, relatif: &str) -> bool {
    let p = Path::new(relatif);
    !relatif.is_empty()
        && p.components().all(|c| matches!(c, Component::Normal(_)))
        && !p.components().any(|c| vault::dossier_ignore(&c.as_os_str().to_string_lossy()))
        && est_dans(&racine.join(p), racine)
}

/// Dernières écritures, de la plus récente à la plus ancienne.
pub fn historique(conn: &Connection, limite: usize) -> rusqlite::Result<Vec<EntreeHistorique>> {
    let mut req = conn.prepare(
        "SELECT id, date, chemin, genre, origine, lot_id, raison, annulee_le FROM historique_vault
         ORDER BY id DESC LIMIT ?1",
    )?;
    let lignes = req.query_map([limite as i64], |r| {
        Ok(EntreeHistorique {
            id: r.get(0)?,
            date: r.get(1)?,
            chemin: r.get(2)?,
            genre: r.get(3)?,
            origine: r.get(4)?,
            lot_id: r.get(5)?,
            raison: r.get(6)?,
            annulee_le: r.get(7)?,
        })
    })?;
    lignes.collect()
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::db;

    fn vault_temporaire(nom: &str) -> PathBuf {
        let d = std::env::temp_dir().join(format!("revisio-ecriture-{nom}-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&d);
        std::fs::create_dir_all(d.join(".obsidian")).unwrap();
        std::fs::create_dir_all(d.join("Spé SVT")).unwrap();
        std::fs::write(d.join("Spé SVT").join("Méiose.md"), "Avant\r\n").unwrap();
        d
    }

    fn ctx(racine: &Path, autorisee: bool) -> Contexte<'_> {
        Contexte { racine, autorisee, origine: "manuel", lot_id: None, raison: None, maintenant: 1 }
    }

    #[test]
    fn noms_de_fichier() {
        assert_eq!(nom_de_fichier("La méiose : étapes"), "La méiose étapes");
        assert_eq!(nom_de_fichier("  Vitesse v = d/t ?  "), "Vitesse v = d t");
        assert_eq!(nom_de_fichier("[[Lien]] #tag"), "Lien tag");
        assert_eq!(nom_de_fichier("..."), "");
    }

    #[test]
    fn chemins_refuses() {
        let r = vault_temporaire("refus");
        assert!(chemin_ecrivable(&r, "Spé SVT/Nouvelle.md").is_ok());
        for mauvais in [
            "../dehors.md",
            ".obsidian/app.md",
            "Spé SVT/.cache/x.md",
            "_Templates/Modèle.md",
            "Spé SVT/image.png",
            "Spé SVT/Dessin.excalidraw.md",
            r"C:\Windows\x.md",
            "Spé SVT/a:b.md",
            "",
        ] {
            assert!(chemin_ecrivable(&r, mauvais).is_err(), "{mauvais} devrait être refusé");
        }
        std::fs::remove_dir_all(&r).unwrap();
    }

    #[test]
    fn rien_sans_autorisation() {
        let r = vault_temporaire("interdit");
        let conn = db::ouvrir_en_memoire().unwrap();
        assert_eq!(creer(&conn, &ctx(&r, false), "Spé SVT/X.md", "x").unwrap_err(), DESACTIVEE);
        assert_eq!(modifier(&conn, &ctx(&r, false), "Spé SVT/Méiose.md", "x", &|_| true).unwrap_err(), DESACTIVEE);
        assert!(!r.join("Spé SVT").join("X.md").exists());
        assert_eq!(std::fs::read(r.join("Spé SVT").join("Méiose.md")).unwrap(), b"Avant\r\n");
        std::fs::remove_dir_all(&r).unwrap();
    }

    #[test]
    fn creation_puis_annulation_retire_note_et_dossiers() {
        let r = vault_temporaire("creation");
        let conn = db::ouvrir_en_memoire().unwrap();
        let id = creer(&conn, &ctx(&r, true), "Spé SVT/Chapitre 4/Mutations.md", "# Mutations\n").unwrap();
        assert_eq!(std::fs::read_to_string(r.join("Spé SVT/Chapitre 4/Mutations.md")).unwrap(), "# Mutations\n");
        // Jamais d'écrasement.
        assert!(creer(&conn, &ctx(&r, true), "Spé SVT/Méiose.md", "x").is_err());
        assert_eq!(std::fs::read(r.join("Spé SVT").join("Méiose.md")).unwrap(), b"Avant\r\n");
        annuler(&conn, &r, id, 2).unwrap();
        assert!(!r.join("Spé SVT/Chapitre 4").exists());
        assert!(r.join("Spé SVT").is_dir());
        assert!(annuler(&conn, &r, id, 3).is_err());
        assert_eq!(historique(&conn, 10).unwrap()[0].annulee_le, Some(2));
        std::fs::remove_dir_all(&r).unwrap();
    }

    #[test]
    fn modification_verifiee_et_annulable() {
        let r = vault_temporaire("modif");
        let conn = db::ouvrir_en_memoire().unwrap();
        let note = r.join("Spé SVT").join("Méiose.md");
        // La note a changé depuis la lecture : refus.
        assert!(modifier(&conn, &ctx(&r, true), "Spé SVT/Méiose.md", "Après", &|a| a == b"Autre").is_err());
        let id = modifier(&conn, &ctx(&r, true), "Spé SVT/Méiose.md", "Après", &|a| a == b"Avant\r\n").unwrap();
        assert_eq!(std::fs::read_to_string(&note).unwrap(), "Après");
        annuler(&conn, &r, id, 2).unwrap();
        assert_eq!(std::fs::read(&note).unwrap(), b"Avant\r\n");

        // Retouchée après l'écriture de Révisio : l'annulation est refusée, rien n'est perdu.
        let id = modifier(&conn, &ctx(&r, true), "Spé SVT/Méiose.md", "Après", &|_| true).unwrap();
        std::fs::write(&note, "Retouchée dans Obsidian").unwrap();
        assert!(annuler(&conn, &r, id, 3).is_err());
        assert_eq!(std::fs::read_to_string(&note).unwrap(), "Retouchée dans Obsidian");
        std::fs::remove_dir_all(&r).unwrap();
    }
}
