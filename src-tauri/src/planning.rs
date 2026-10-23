//! Dates d'évaluation. Le planning lui-même (quel chapitre réviser quel jour) est recalculé
//! chaque jour par l'interface à partir de ces dates et de la maîtrise de chaque chapitre :
//! il s'adapte tout seul quand on progresse ou qu'on prend du retard.

use rusqlite::{params, Connection};
use serde::{Deserialize, Serialize};

pub const GENRES: &[&str] = &["controle", "ds", "oral", "bac_blanc", "autre"];

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct Evaluation {
    /// 0 pour une nouvelle évaluation.
    #[serde(default)]
    pub id: i64,
    /// Jour local AAAA-MM-JJ.
    pub jour: String,
    pub matiere: String,
    pub titre: String,
    pub genre: String,
    /// Chapitres à réviser (`None` = notes générales de la matière).
    pub chapitres: Vec<Option<String>>,
}

/// AAAA-MM-JJ avec un mois et un jour plausibles.
fn jour_valide(j: &str) -> bool {
    let b = j.as_bytes();
    if b.len() != 10 || b[4] != b'-' || b[7] != b'-' {
        return false;
    }
    let nombre = |s: &str| s.parse::<u32>().ok();
    match (nombre(&j[0..4]), nombre(&j[5..7]), nombre(&j[8..10])) {
        (Some(a), Some(m), Some(d)) => (2000..=2100).contains(&a) && (1..=12).contains(&m) && (1..=31).contains(&d),
        _ => false,
    }
}

pub fn valider(e: &Evaluation) -> Result<(), String> {
    if !jour_valide(&e.jour) {
        return Err("Date invalide.".into());
    }
    if e.matiere.trim().is_empty() {
        return Err("Choisis une matière.".into());
    }
    if e.titre.trim().is_empty() || e.titre.chars().count() > 120 {
        return Err("Le titre doit faire entre 1 et 120 caractères.".into());
    }
    if !GENRES.contains(&e.genre.as_str()) {
        return Err(format!("Type d'évaluation inconnu : {}", e.genre));
    }
    if e.chapitres.is_empty() {
        return Err("Choisis au moins un chapitre à réviser.".into());
    }
    Ok(())
}

/// Ajoute (id = 0) ou modifie une évaluation ; renvoie son identifiant.
pub fn enregistrer(conn: &Connection, e: &Evaluation, maintenant: i64) -> Result<i64, String> {
    valider(e)?;
    let chapitres = serde_json::to_string(&e.chapitres).map_err(|x| x.to_string())?;
    if e.id == 0 {
        conn.execute(
            "INSERT INTO evaluations(jour, matiere, titre, genre, chapitres, creee_le) VALUES (?1, ?2, ?3, ?4, ?5, ?6)",
            params![e.jour, e.matiere, e.titre.trim(), e.genre, chapitres, maintenant],
        )
        .map_err(|x| x.to_string())?;
        Ok(conn.last_insert_rowid())
    } else {
        let n = conn
            .execute(
                "UPDATE evaluations SET jour = ?2, matiere = ?3, titre = ?4, genre = ?5, chapitres = ?6 WHERE id = ?1",
                params![e.id, e.jour, e.matiere, e.titre.trim(), e.genre, chapitres],
            )
            .map_err(|x| x.to_string())?;
        if n == 0 {
            return Err("Évaluation introuvable.".into());
        }
        Ok(e.id)
    }
}

/// Évaluations à partir du jour `depuis` (AAAA-MM-JJ), la plus proche d'abord.
pub fn lister(conn: &Connection, depuis: &str) -> rusqlite::Result<Vec<Evaluation>> {
    let mut req =
        conn.prepare("SELECT id, jour, matiere, titre, genre, chapitres FROM evaluations WHERE jour >= ?1 ORDER BY jour, id")?;
    let lignes = req.query_map([depuis], |r| {
        Ok(Evaluation {
            id: r.get(0)?,
            jour: r.get(1)?,
            matiere: r.get(2)?,
            titre: r.get(3)?,
            genre: r.get(4)?,
            chapitres: serde_json::from_str(&r.get::<_, String>(5)?).unwrap_or_default(),
        })
    })?;
    lignes.collect()
}

pub fn supprimer(conn: &Connection, id: i64) -> rusqlite::Result<bool> {
    Ok(conn.execute("DELETE FROM evaluations WHERE id = ?1", [id])? > 0)
}

#[derive(Debug, Serialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct RevuesChapitre {
    pub matiere: Option<String>,
    pub chapitre: Option<String>,
    /// Cartes différentes révisées depuis le début de la journée.
    pub cartes: i64,
}

/// Avancement du jour, chapitre par chapitre (pour cocher le planning).
pub fn revues_depuis(conn: &Connection, debut_jour: i64) -> rusqlite::Result<Vec<RevuesChapitre>> {
    let mut req = conn.prepare(
        "SELECT c.matiere, c.chapitre, count(DISTINCT c.id) FROM revisions r JOIN cartes c ON c.id = r.carte_id
         WHERE r.date >= ?1 GROUP BY c.matiere, c.chapitre",
    )?;
    let lignes = req.query_map([debut_jour], |r| Ok(RevuesChapitre { matiere: r.get(0)?, chapitre: r.get(1)?, cartes: r.get(2)? }))?;
    lignes.collect()
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::db;

    fn evaluation() -> Evaluation {
        Evaluation {
            id: 0,
            jour: "2026-10-28".into(),
            matiere: "Spé SVT".into(),
            titre: "Contrôle de SVT".into(),
            genre: "controle".into(),
            chapitres: vec![Some("Chapitre 3 - La méiose".into()), None],
        }
    }

    #[test]
    fn validation() {
        assert!(valider(&evaluation()).is_ok());
        for jour in ["2026-13-01", "28/10/2026", "2026-10-1", ""] {
            assert!(valider(&Evaluation { jour: jour.into(), ..evaluation() }).is_err(), "{jour}");
        }
        assert!(valider(&Evaluation { genre: "interro".into(), ..evaluation() }).is_err());
        assert!(valider(&Evaluation { chapitres: vec![], ..evaluation() }).is_err());
        assert!(valider(&Evaluation { titre: "  ".into(), ..evaluation() }).is_err());
    }

    #[test]
    fn ajout_modification_suppression() {
        let conn = db::ouvrir_en_memoire().unwrap();
        let id = enregistrer(&conn, &evaluation(), 1).unwrap();
        let passee = Evaluation { jour: "2026-10-01".into(), ..evaluation() };
        enregistrer(&conn, &passee, 1).unwrap();
        let liste = lister(&conn, "2026-10-19").unwrap();
        assert_eq!(liste.len(), 1);
        assert_eq!(liste[0], Evaluation { id, ..evaluation() });

        let modifiee = Evaluation { id, jour: "2026-10-30".into(), ..evaluation() };
        assert_eq!(enregistrer(&conn, &modifiee, 1).unwrap(), id);
        assert_eq!(lister(&conn, "2026-10-19").unwrap()[0].jour, "2026-10-30");
        assert!(enregistrer(&conn, &Evaluation { id: 999, ..evaluation() }, 1).is_err());
        assert!(supprimer(&conn, id).unwrap());
        assert!(lister(&conn, "2026-10-19").unwrap().is_empty());
    }

    #[test]
    fn avancement_du_jour() {
        let conn = db::ouvrir_en_memoire().unwrap();
        conn.execute_batch(
            "INSERT INTO cartes(id, cle, genre, note_chemin, matiere, chapitre, question, reponse, creee_le)
               VALUES (1, 'a', 'trou', 'n.md', 'Spé SVT', 'Méiose', 'q', 'r', 0),
                      (2, 'b', 'trou', 'n.md', 'Spé SVT', 'Méiose', 'q', 'r', 0);
             INSERT INTO revisions(carte_id, date, note, confiance, correcte) VALUES
               (1, 100, 3, 'sur', 1), (1, 200, 3, 'sur', 1), (2, 50, 3, 'sur', 1);",
        )
        .unwrap();
        let r = revues_depuis(&conn, 80).unwrap();
        assert_eq!(r, vec![RevuesChapitre { matiere: Some("Spé SVT".into()), chapitre: Some("Méiose".into()), cartes: 1 }]);
    }
}
