//! Recherche plein texte hors ligne (SQLite FTS5), insensible aux accents et à la casse.

use rusqlite::{params, Connection};
use serde::Serialize;

/// Marqueurs placés autour des mots trouvés dans l'extrait ; l'interface les
/// remplace par du surlignage (ainsi aucun HTML ne transite).
pub const DEBUT_SURLIGNAGE: char = '\u{1}';
pub const FIN_SURLIGNAGE: char = '\u{2}';

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Resultat {
    pub chemin: String,
    pub titre: String,
    pub matiere: Option<String>,
    pub chapitre: Option<String>,
    pub genre: String,
    /// Titre de la section (ou du callout) où se trouve le passage.
    pub section: String,
    pub extrait: String,
}

/// Transforme le texte tapé en requête FTS5 sûre : chaque mot devient `"mot"*`
/// (préfixe, pour trouver pendant la frappe), et tous les mots doivent être présents.
pub fn requete_fts(texte: &str) -> Option<String> {
    let mots: Vec<String> = texte
        .split(|c: char| !c.is_alphanumeric())
        .filter(|m| !m.is_empty())
        .map(|m| format!("\"{m}\"*"))
        .collect();
    (!mots.is_empty()).then(|| mots.join(" "))
}

pub fn rechercher(
    conn: &Connection,
    texte: &str,
    matiere: Option<&str>,
    limite: usize,
) -> rusqlite::Result<Vec<Resultat>> {
    let Some(requete) = requete_fts(texte) else { return Ok(vec![]) };
    let mut req = conn.prepare(
        "SELECT n.chemin, n.titre, n.matiere, n.chapitre, n.genre, r.section,
                snippet(recherche, -1, char(1), char(2), '…', 16)
         FROM recherche r JOIN notes n ON n.id = r.note_id
         WHERE recherche MATCH ?1 AND (?2 IS NULL OR n.matiere = ?2)
         ORDER BY bm25(recherche, 10.0, 4.0, 1.0)
         LIMIT ?3",
    )?;
    let lignes = req.query_map(params![requete, matiere, limite as i64], |r| {
        Ok(Resultat {
            chemin: r.get(0)?,
            titre: r.get(1)?,
            matiere: r.get(2)?,
            chapitre: r.get(3)?,
            genre: r.get(4)?,
            section: r.get(5)?,
            extrait: r.get(6)?,
        })
    })?;
    lignes.collect()
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{db, indexer};
    use std::path::Path;

    fn base_exemple() -> Connection {
        let mut conn = db::ouvrir_en_memoire().unwrap();
        let racine = Path::new(env!("CARGO_MANIFEST_DIR")).join("tests").join("vault_exemple");
        indexer::indexer(&mut conn, &racine).unwrap();
        conn
    }

    #[test]
    fn requete_securisee() {
        assert_eq!(requete_fts("méiose 2n").as_deref(), Some("\"méiose\"* \"2n\"*"));
        assert_eq!(requete_fts("  \" OR * ( ").as_deref(), Some("\"OR\"*"));
        assert_eq!(requete_fts("!!"), None);
    }

    #[test]
    fn sans_accents_et_par_prefixe() {
        let conn = base_exemple();
        let r = rechercher(&conn, "meiose haplo", None, 20).unwrap();
        assert!(!r.is_empty());
        assert!(r.iter().all(|x| x.matiere.as_deref() == Some("Spé SVT")));
        assert!(r[0].extrait.contains(DEBUT_SURLIGNAGE));
    }

    #[test]
    fn filtre_par_matiere_et_legendes_de_schema() {
        let conn = base_exemple();
        assert!(rechercher(&conn, "Verfolgung", Some("Spé SVT"), 20).unwrap().is_empty());
        assert_eq!(rechercher(&conn, "Verfolgung", Some("Allemand"), 20).unwrap().len(), 1);
        let r = rechercher(&conn, "gametes", None, 20).unwrap();
        assert!(r.iter().any(|x| x.genre == "schema"));
    }
}
