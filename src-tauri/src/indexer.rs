//! Indexation du vault dans SQLite : matière → chapitre → note → sections.
//!
//! L'indexation est incrémentale : une note dont la taille et la date de
//! modification n'ont pas changé n'est pas relue.

use std::collections::{HashMap, HashSet};
use std::path::Path;
use std::time::Instant;

use rusqlite::{params, Connection, Transaction};
use serde::Serialize;

use crate::db;
use crate::parser::{self, NoteAnalysee};
use crate::vault::{self, FichierVault};

/// À augmenter à chaque changement du parseur ou du classement : force une réindexation
/// complète (sinon les notes inchangées ne seraient pas relues avec le nouveau parseur).
const VERSION_PARSEUR: &str = "3";

/// Dossiers de premier niveau qui ne sont pas des matières.
const DOSSIERS_SPECIAUX: &[&str] = &["_Révisions", "_Assets", "Web clip", "Travail maison", "Quiz"];

#[derive(Debug, Clone, Default, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Rapport {
    pub ajoutees: usize,
    pub modifiees: usize,
    pub supprimees: usize,
    pub total: usize,
    pub fichiers_annexes: usize,
    pub erreurs: Vec<String>,
    pub duree_ms: u128,
}

#[derive(Debug, Clone, PartialEq)]
pub struct Classement {
    pub nom: String,
    pub titre: String,
    pub matiere: Option<String>,
    pub chapitre: Option<String>,
    pub genre: &'static str,
}

/// Déduit matière, chapitre et type de note à partir du chemin.
pub fn classer(chemin: &str) -> Classement {
    let morceaux: Vec<&str> = chemin.split('/').collect();
    let fichier = morceaux.last().copied().unwrap_or(chemin);
    let nom = fichier.strip_suffix(".md").unwrap_or(fichier).to_string();
    let titre = nom.strip_suffix(".excalidraw").unwrap_or(&nom).to_string();
    let dossiers = &morceaux[..morceaux.len() - 1];

    let premier = dossiers.first().copied();
    let special = premier.is_some_and(|p| DOSSIERS_SPECIAUX.iter().any(|s| s.eq_ignore_ascii_case(p)));
    let (matiere, chapitre) = match (premier, special) {
        (None, _) => (None, None),
        (Some(p), true) => (None, Some(p.to_string())),
        (Some(p), false) => (Some(p.to_string()), dossiers.get(1).map(|c| c.to_string())),
    };

    let genre = if fichier.ends_with(".excalidraw.md") {
        "schema"
    } else if premier == Some("_Révisions") {
        "revision"
    } else if titre == "Accueil" || titre == "Home" {
        "accueil"
    } else if titre.starts_with("Fiche de synthèse") {
        "fiche_synthese"
    } else if titre.starts_with("Wortschatz") || titre.starts_with("Vocabulaire") || titre.contains("(Quizlet)") {
        "vocabulaire"
    } else if titre.starts_with("Méthodologie") {
        "methodologie"
    } else if titre.contains("(fiche de référence)") {
        "reference"
    } else if titre.starts_with("Étude linéaire") {
        "etude_lineaire"
    } else {
        "cours"
    };

    Classement { nom, titre, matiere, chapitre, genre }
}

fn est_note(chemin: &str) -> bool {
    chemin.to_lowercase().ends_with(".md")
}

/// Indexe (ou met à jour l'index de) tout le vault.
pub fn indexer(conn: &mut Connection, racine: &Path) -> Result<Rapport, String> {
    let debut = Instant::now();
    let fichiers = vault::lister_fichiers(racine)?;
    let mut rapport = Rapport::default();

    let existants: HashMap<String, (i64, i64, i64)> = {
        let mut req = conn
            .prepare("SELECT chemin, id, taille, modifie FROM notes")
            .map_err(|e| e.to_string())?;
        let lignes = req
            .query_map([], |r| Ok((r.get::<_, String>(0)?, (r.get(1)?, r.get(2)?, r.get(3)?))))
            .map_err(|e| e.to_string())?;
        lignes.filter_map(Result::ok).collect()
    };

    let tout_relire = db::lire_meta(conn, "version_parseur").as_deref() != Some(VERSION_PARSEUR);

    let tx = conn.transaction().map_err(|e| e.to_string())?;
    let mut vus: HashSet<&str> = HashSet::new();
    let (notes, annexes): (Vec<&FichierVault>, Vec<&FichierVault>) = fichiers.iter().partition(|f| est_note(&f.chemin));

    for f in &notes {
        vus.insert(&f.chemin);
        let deja = existants.get(&f.chemin);
        if let Some(&(_, taille, modifie)) = deja {
            if !tout_relire && taille == f.taille as i64 && modifie == f.modifie {
                continue; // inchangée
            }
        }
        let texte = match vault::lire_texte(racine, &f.chemin) {
            Ok(t) => t,
            Err(e) => {
                rapport.erreurs.push(e);
                continue; // on garde l'ancien index de cette note s'il existe
            }
        };
        let classement = classer(&f.chemin);
        let analyse = parser::analyser(&texte, classement.genre == "schema");
        enregistrer_note(&tx, f, &classement, &analyse, deja.map(|d| d.0)).map_err(|e| e.to_string())?;
        if deja.is_some() {
            rapport.modifiees += 1;
        } else {
            rapport.ajoutees += 1;
        }
    }

    for (chemin, (id, _, _)) in &existants {
        if !vus.contains(chemin.as_str()) {
            supprimer_note(&tx, *id).map_err(|e| e.to_string())?;
            rapport.supprimees += 1;
        }
    }

    tx.execute("DELETE FROM fichiers", []).map_err(|e| e.to_string())?;
    for f in &annexes {
        let nom = f.chemin.rsplit('/').next().unwrap_or(&f.chemin);
        tx.execute(
            "INSERT INTO fichiers(chemin, nom, taille, modifie) VALUES (?1, ?2, ?3, ?4)",
            params![f.chemin, nom, f.taille as i64, f.modifie],
        )
        .map_err(|e| e.to_string())?;
    }

    db::ecrire_meta(&tx, "version_parseur", VERSION_PARSEUR).map_err(|e| e.to_string())?;
    tx.commit().map_err(|e| e.to_string())?;
    rapport.total = notes.len();
    rapport.fichiers_annexes = annexes.len();
    rapport.duree_ms = debut.elapsed().as_millis();
    Ok(rapport)
}

fn supprimer_note(tx: &Transaction, id: i64) -> rusqlite::Result<()> {
    tx.execute("DELETE FROM recherche WHERE note_id = ?1", [id])?;
    tx.execute("DELETE FROM notes WHERE id = ?1", [id])?; // sections et liens suivent (ON DELETE CASCADE)
    Ok(())
}

fn enregistrer_note(
    tx: &Transaction,
    f: &FichierVault,
    c: &Classement,
    a: &NoteAnalysee,
    id_existant: Option<i64>,
) -> rusqlite::Result<()> {
    let tags = serde_json::to_string(&a.tags).unwrap_or_else(|_| "[]".into());
    let frontmatter = serde_json::to_string(&a.frontmatter).unwrap_or_else(|_| "{}".into());

    let id = match id_existant {
        Some(id) => {
            tx.execute(
                "UPDATE notes SET nom=?2, titre=?3, matiere=?4, chapitre=?5, genre=?6, tags=?7, frontmatter=?8, taille=?9, modifie=?10 WHERE id=?1",
                params![id, c.nom, c.titre, c.matiere, c.chapitre, c.genre, tags, frontmatter, f.taille as i64, f.modifie],
            )?;
            tx.execute("DELETE FROM sections WHERE note_id = ?1", [id])?;
            tx.execute("DELETE FROM liens WHERE source_id = ?1", [id])?;
            tx.execute("DELETE FROM recherche WHERE note_id = ?1", [id])?;
            id
        }
        None => {
            tx.execute(
                "INSERT INTO notes(chemin, nom, titre, matiere, chapitre, genre, tags, frontmatter, taille, modifie)
                 VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10)",
                params![f.chemin, c.nom, c.titre, c.matiere, c.chapitre, c.genre, tags, frontmatter, f.taille as i64, f.modifie],
            )?;
            tx.last_insert_rowid()
        }
    };

    let indexer_texte = c.genre != "accueil"; // les pages d'accueil ne sont que des menus de liens
    let mut a_une_ligne_de_recherche = false;
    for s in &a.sections {
        tx.execute(
            "INSERT INTO sections(note_id, ordre, genre, niveau, type_callout, titre, texte, ligne)
             VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8)",
            params![id, s.ordre as i64, s.genre, s.niveau, s.type_callout, s.titre, s.texte, s.ligne as i64],
        )?;
        if indexer_texte {
            let section_id = tx.last_insert_rowid();
            // Le titre d'un callout est sa première ligne : il est déjà dans la colonne « section ».
            let corps = if s.genre == "callout" {
                s.texte.split_once('\n').map(|(_, reste)| reste).unwrap_or("")
            } else {
                &s.texte
            };
            tx.execute(
                "INSERT INTO recherche(titre, section, texte, note_id, section_id) VALUES (?1, ?2, ?3, ?4, ?5)",
                params![c.titre, parser::texte_brut(&s.titre).trim(), parser::texte_brut(corps), id, section_id],
            )?;
            a_une_ligne_de_recherche = true;
        }
    }
    if !a_une_ligne_de_recherche {
        // Note vide ou page d'accueil : trouvable au moins par son titre.
        tx.execute(
            "INSERT INTO recherche(titre, section, texte, note_id, section_id) VALUES (?1, '', '', ?2, NULL)",
            params![c.titre, id],
        )?;
    }

    for l in &a.liens {
        tx.execute(
            "INSERT INTO liens(source_id, cible, ancre, integre) VALUES (?1, ?2, ?3, ?4)",
            params![id, l.cible, l.ancre, l.integre],
        )?;
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn classement_des_notes() {
        let c = classer("Spé SVT/Chapitre 3 - La méiose/Fiche de synthèse - Chapitre 3 La méiose.md");
        assert_eq!(c.matiere.as_deref(), Some("Spé SVT"));
        assert_eq!(c.chapitre.as_deref(), Some("Chapitre 3 - La méiose"));
        assert_eq!(c.genre, "fiche_synthese");

        let c = classer("Spé SVT/Rappels/La mutation (schéma).excalidraw.md");
        assert_eq!((c.genre, c.nom.as_str(), c.titre.as_str()), ("schema", "La mutation (schéma).excalidraw", "La mutation (schéma)"));

        let c = classer("Français/Méthodologie - La dissertation littéraire.md");
        assert_eq!((c.matiere.as_deref(), c.chapitre.as_deref(), c.genre), (Some("Français"), None, "methodologie"));

        assert_eq!(classer("Spé Maths/Accueil.md").genre, "accueil");
        assert_eq!(classer("Home.md").matiere, None);
        assert_eq!(classer("DNL histoire géo/Fuir/Vocabulaire allemand - Fuite et frontières (Quizlet).md").genre, "vocabulaire");
        assert_eq!(classer("Allemand/Identités et échanges/Wortschatz - So fern, so nah.md").genre, "vocabulaire");
        assert_eq!(classer("Spé Physique-Chimie/Les unités et conversions (fiche de référence).md").genre, "reference");

        let c = classer("_Révisions/Semaine du 20 au 26 septembre.md");
        assert_eq!((c.matiere, c.chapitre.as_deref(), c.genre), (None, Some("_Révisions"), "revision"));
        assert_eq!(classer("Web clip/Article.md").matiere, None);
    }
}
