//! Favoris de l'espace web (par matière) et passages de pages web enregistrés comme sources
//! d'un chapitre. Tout est rangé dans la base de Révisio, jamais dans le vault. L'IA ne s'en sert
//! que si l'élève coche « inclure mes sources web ».

use rusqlite::{params, Connection};
use serde::{Deserialize, Serialize};

use crate::ia::ancrage::Extrait;

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct Favori {
    #[serde(default)]
    pub id: i64,
    pub matiere: Option<String>,
    pub titre: String,
    pub url: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct SourceWeb {
    #[serde(default)]
    pub id: i64,
    #[serde(default)]
    pub date: i64,
    pub url: String,
    pub titre: String,
    pub texte: String,
    pub matiere: String,
    pub chapitre: Option<String>,
}

fn url_valide(url: &str) -> bool {
    url.starts_with("https://") || url.starts_with("http://")
}

pub fn ajouter_favori(conn: &Connection, f: &Favori, date: i64) -> Result<i64, String> {
    if !url_valide(&f.url) || f.titre.trim().is_empty() {
        return Err("Favori invalide (adresse http/https et titre obligatoires).".into());
    }
    conn.execute(
        "INSERT INTO favoris(matiere, titre, url, creee_le) VALUES (?1, ?2, ?3, ?4)",
        params![f.matiere, f.titre.trim(), f.url, date],
    )
    .map_err(|e| e.to_string())?;
    Ok(conn.last_insert_rowid())
}

pub fn favoris(conn: &Connection) -> rusqlite::Result<Vec<Favori>> {
    conn.prepare("SELECT id, matiere, titre, url FROM favoris ORDER BY matiere IS NULL, matiere, titre")?
        .query_map([], |r| Ok(Favori { id: r.get(0)?, matiere: r.get(1)?, titre: r.get(2)?, url: r.get(3)? }))?
        .collect()
}

pub fn supprimer_favori(conn: &Connection, id: i64) -> rusqlite::Result<bool> {
    Ok(conn.execute("DELETE FROM favoris WHERE id = ?1", [id])? > 0)
}

pub fn ajouter_source(conn: &Connection, s: &SourceWeb, date: i64) -> Result<i64, String> {
    if !url_valide(&s.url) {
        return Err("Adresse de la page invalide.".into());
    }
    let texte = s.texte.trim();
    if texte.split_whitespace().count() < 5 {
        return Err("Sélectionne un passage plus long dans la page (au moins quelques phrases).".into());
    }
    if texte.chars().count() > 20_000 {
        return Err("Passage trop long : sélectionne une partie de la page (20 000 caractères au plus).".into());
    }
    if s.matiere.trim().is_empty() {
        return Err("Choisis la matière de cette source.".into());
    }
    conn.execute(
        "INSERT INTO sources_web(date, url, titre, texte, matiere, chapitre) VALUES (?1, ?2, ?3, ?4, ?5, ?6)",
        params![date, s.url, s.titre.trim(), texte, s.matiere, s.chapitre],
    )
    .map_err(|e| e.to_string())?;
    Ok(conn.last_insert_rowid())
}

pub fn sources(conn: &Connection, matiere: Option<&str>, chapitre: Option<Option<&str>>) -> rusqlite::Result<Vec<SourceWeb>> {
    let mut req = conn.prepare(
        "SELECT id, date, url, titre, texte, matiere, chapitre FROM sources_web
         WHERE (?1 IS NULL OR matiere = ?1) AND (?2 = 0 OR chapitre IS ?3) ORDER BY date DESC",
    )?;
    let (filtrer, ch) = match chapitre {
        Some(c) => (1, c),
        None => (0, None),
    };
    req.query_map(params![matiere, filtrer, ch], |r| {
        Ok(SourceWeb {
            id: r.get(0)?,
            date: r.get(1)?,
            url: r.get(2)?,
            titre: r.get(3)?,
            texte: r.get(4)?,
            matiere: r.get(5)?,
            chapitre: r.get(6)?,
        })
    })?
    .collect()
}

pub fn supprimer_source(conn: &Connection, id: i64) -> rusqlite::Result<bool> {
    Ok(conn.execute("DELETE FROM sources_web WHERE id = ?1", [id])? > 0)
}

/// Sources web d'un chapitre en extraits pour l'IA, numérotés à la suite de `premier`.
pub fn en_extraits(sources: &[SourceWeb], premier: usize) -> Vec<Extrait> {
    sources
        .iter()
        .enumerate()
        .map(|(i, s)| Extrait {
            id: format!("E{}", premier + i),
            note_chemin: s.url.clone(),
            titre_note: format!("Source web : {}", if s.titre.is_empty() { &s.url } else { &s.titre }),
            section: String::new(),
            texte: s.texte.clone(),
            markdown: s.texte.clone(),
        })
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::db;

    fn source(matiere: &str, chapitre: Option<&str>) -> SourceWeb {
        SourceWeb {
            id: 0,
            date: 0,
            url: "https://fr.wikipedia.org/wiki/Méiose".into(),
            titre: "Méiose — Wikipédia".into(),
            texte: "La méiose est une division cellulaire en deux étapes.".into(),
            matiere: matiere.into(),
            chapitre: chapitre.map(str::to_owned),
        }
    }

    #[test]
    fn favoris_par_matiere() {
        let conn = db::ouvrir_en_memoire().unwrap();
        let f = Favori { id: 0, matiere: Some("Spé SVT".into()), titre: "SVT".into(), url: "https://svt.fr".into() };
        let id = ajouter_favori(&conn, &f, 1).unwrap();
        assert!(ajouter_favori(&conn, &Favori { url: "file:///C:/x".into(), ..f.clone() }, 1).is_err());
        assert_eq!(favoris(&conn).unwrap(), vec![Favori { id, ..f }]);
        assert!(supprimer_favori(&conn, id).unwrap());
    }

    #[test]
    fn sources_par_chapitre() {
        let conn = db::ouvrir_en_memoire().unwrap();
        let id = ajouter_source(&conn, &source("Spé SVT", Some("Méiose")), 5).unwrap();
        ajouter_source(&conn, &source("Spé SVT", None), 6).unwrap();
        assert!(ajouter_source(&conn, &SourceWeb { texte: "trop court".into(), ..source("Spé SVT", None) }, 7).is_err());
        assert_eq!(sources(&conn, Some("Spé SVT"), None).unwrap().len(), 2);
        let du_chapitre = sources(&conn, Some("Spé SVT"), Some(Some("Méiose"))).unwrap();
        assert_eq!(du_chapitre.len(), 1);
        assert_eq!(du_chapitre[0].id, id);
        assert_eq!(sources(&conn, Some("Spé SVT"), Some(None)).unwrap().len(), 1);
        let e = en_extraits(&du_chapitre, 7);
        assert_eq!((e[0].id.as_str(), e[0].titre_note.as_str()), ("E7", "Source web : Méiose — Wikipédia"));
        assert!(supprimer_source(&conn, id).unwrap());
    }
}
