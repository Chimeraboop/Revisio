//! Stockage des sujets (générés une fois par l'IA) et des copies corrigées.
//! Un sujet enregistré peut être refait hors ligne ; seules les rédactions attendent Internet pour être corrigées.

use rusqlite::{params, Connection, OptionalExtension};
use serde::{Deserialize, Serialize};

use super::ancrage::Extrait;
use super::exercices::{self, ResultatQuestion, Sujet};

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct ChapitreChoisi {
    pub matiere: String,
    pub chapitre: Option<String>,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Copie {
    pub id: i64,
    pub sujet_id: i64,
    pub date: i64,
    pub duree_ms: i64,
    pub reponses: Vec<String>,
    pub resultats: Vec<ResultatQuestion>,
    pub appreciation: String,
    /// Sur 20, calculée sur les questions déjà corrigées.
    pub note: Option<f64>,
}

impl Copie {
    /// Questions encore à corriger par l'IA.
    pub fn a_corriger(&self) -> Vec<usize> {
        self.resultats.iter().enumerate().filter(|(_, r)| r.verdict == "a_corriger").map(|(i, _)| i).collect()
    }
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct SujetEnregistre {
    pub id: i64,
    pub date: i64,
    pub type_id: String,
    pub type_nom: String,
    pub matiere: String,
    pub chapitres: Vec<ChapitreChoisi>,
    pub modele: String,
    pub duree_minutes: u32,
    pub sujet: Sujet,
    pub extraits: Vec<Extrait>,
    /// Copies déjà rendues pour ce sujet, la plus récente d'abord.
    pub copies: Vec<Copie>,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ResumeSujet {
    pub id: i64,
    pub date: i64,
    pub type_id: String,
    pub type_nom: String,
    pub matiere: String,
    pub chapitres: Vec<ChapitreChoisi>,
    pub duree_minutes: u32,
    pub titre: String,
    pub questions: usize,
    pub copies: i64,
    pub derniere_note: Option<f64>,
}

fn json<T: Serialize>(v: &T) -> String {
    serde_json::to_string(v).unwrap_or_default()
}

fn lire_json<T: for<'de> Deserialize<'de>>(texte: &str, colonne: usize) -> rusqlite::Result<T> {
    serde_json::from_str(texte).map_err(|e| rusqlite::Error::FromSqlConversionFailure(colonne, rusqlite::types::Type::Text, Box::new(e)))
}

#[allow(clippy::too_many_arguments)]
pub fn enregistrer_sujet(
    conn: &Connection,
    type_id: &str,
    matiere: &str,
    chapitres: &[ChapitreChoisi],
    modele: &str,
    duree_minutes: u32,
    sujet: &Sujet,
    extraits: &[Extrait],
    date: i64,
) -> rusqlite::Result<i64> {
    conn.execute(
        "INSERT INTO sujets(date, type, matiere, chapitres, modele, duree_minutes, sujet, extraits)
         VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8)",
        params![date, type_id, matiere, json(&chapitres), modele, duree_minutes, json(sujet), json(&extraits)],
    )?;
    Ok(conn.last_insert_rowid())
}

/// Titres des sujets déjà proposés sur les mêmes chapitres (pour ne pas les répéter).
pub fn titres_deja_proposes(conn: &Connection, type_id: &str, chapitres: &[ChapitreChoisi]) -> rusqlite::Result<Vec<String>> {
    let mut req = conn.prepare(
        "SELECT json_extract(sujet, '$.titre') FROM sujets WHERE type = ?1 AND chapitres = ?2 ORDER BY date DESC LIMIT 10",
    )?;
    let lignes = req.query_map(params![type_id, json(&chapitres)], |r| r.get::<_, Option<String>>(0))?;
    Ok(lignes.filter_map(|l| l.ok().flatten()).collect())
}

pub fn lire_sujet(conn: &Connection, id: i64) -> rusqlite::Result<Option<SujetEnregistre>> {
    let sujet = conn
        .query_row(
            "SELECT id, date, type, matiere, chapitres, modele, duree_minutes, sujet, extraits FROM sujets WHERE id = ?1",
            [id],
            |r| {
                let type_id: String = r.get(2)?;
                Ok(SujetEnregistre {
                    id: r.get(0)?,
                    date: r.get(1)?,
                    type_nom: exercices::nom_du_type(&type_id).into(),
                    type_id,
                    matiere: r.get(3)?,
                    chapitres: lire_json(&r.get::<_, String>(4)?, 4)?,
                    modele: r.get(5)?,
                    duree_minutes: r.get(6)?,
                    sujet: lire_json(&r.get::<_, String>(7)?, 7)?,
                    extraits: lire_json(&r.get::<_, String>(8)?, 8)?,
                    copies: Vec::new(),
                })
            },
        )
        .optional()?;
    let Some(mut sujet) = sujet else { return Ok(None) };
    let mut req = conn.prepare("SELECT id FROM copies WHERE sujet_id = ?1 ORDER BY date DESC")?;
    let ids: Vec<i64> = req.query_map([id], |r| r.get(0))?.collect::<Result<_, _>>()?;
    for c in ids {
        if let Some(copie) = lire_copie(conn, c)? {
            sujet.copies.push(copie);
        }
    }
    Ok(Some(sujet))
}

pub fn lister_sujets(conn: &Connection) -> rusqlite::Result<Vec<ResumeSujet>> {
    let mut req = conn.prepare(
        "SELECT s.id, s.date, s.type, s.matiere, s.chapitres, s.duree_minutes,
                json_extract(s.sujet, '$.titre'), json_array_length(s.sujet, '$.questions'),
                (SELECT count(*) FROM copies c WHERE c.sujet_id = s.id),
                (SELECT c.note FROM copies c WHERE c.sujet_id = s.id ORDER BY c.date DESC LIMIT 1)
         FROM sujets s ORDER BY s.date DESC LIMIT 100",
    )?;
    let lignes = req.query_map([], |r| {
        let type_id: String = r.get(2)?;
        Ok(ResumeSujet {
            id: r.get(0)?,
            date: r.get(1)?,
            type_nom: exercices::nom_du_type(&type_id).into(),
            type_id,
            matiere: r.get(3)?,
            chapitres: lire_json(&r.get::<_, String>(4)?, 4)?,
            duree_minutes: r.get(5)?,
            titre: r.get::<_, Option<String>>(6)?.unwrap_or_default(),
            questions: r.get::<_, Option<i64>>(7)?.unwrap_or(0) as usize,
            copies: r.get(8)?,
            derniere_note: r.get(9)?,
        })
    })?;
    lignes.collect()
}

/// Les questions fausses rejoignent le carnet d'erreurs (genre « examen », sans carte), avec
/// l'extrait de cours qui donne la réponse. `numeros` : questions à prendre en compte.
pub fn noter_erreurs(
    conn: &Connection,
    sujet: &SujetEnregistre,
    reponses: &[String],
    resultats: &[ResultatQuestion],
    numeros: &[u32],
    date: i64,
) -> rusqlite::Result<usize> {
    let mut n = 0;
    for r in resultats.iter().filter(|r| r.verdict == "faux" && !r.retiree && numeros.contains(&r.numero)) {
        let Some(q) = sujet.sujet.questions.get(r.numero as usize - 1) else { continue };
        let extrait = sujet.extraits.iter().find(|e| e.id == q.source);
        let chemin = extrait.map(|e| e.note_chemin.clone()).unwrap_or_default();
        // Matière et chapitre de la note source (un examen peut couvrir plusieurs chapitres).
        let (matiere, chapitre): (Option<String>, Option<String>) = conn
            .query_row("SELECT matiere, chapitre FROM notes WHERE chemin = ?1", [&chemin], |l| Ok((l.get(0)?, l.get(1)?)))
            .optional()?
            .unwrap_or_else(|| {
                let c = sujet.chapitres.first();
                (c.map(|c| c.matiere.clone()), c.and_then(|c| c.chapitre.clone()))
            });
        conn.execute(
            "INSERT INTO erreurs(carte_id, date, genre, matiere, chapitre, note_chemin, section, question,
                                 reponse_attendue, reponse_donnee, extrait)
             VALUES (NULL, ?1, 'examen', ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9)",
            params![
                date,
                matiere,
                chapitre,
                chemin,
                extrait.map(|e| e.section.clone()).unwrap_or_default(),
                q.enonce,
                q.reponse_attendue,
                reponses.get(r.numero as usize - 1).map(|s| s.trim()).unwrap_or(""),
                extrait.map(|e| e.markdown.clone()).unwrap_or_default()
            ],
        )?;
        n += 1;
    }
    Ok(n)
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct NoteCopie {
    pub date: i64,
    pub sujet_id: i64,
    pub titre: String,
    pub type_nom: String,
    pub matiere: String,
    pub note: f64,
}

/// Dernières copies notées (tableau de bord, export de la progression).
pub fn dernieres_notes(conn: &Connection, limite: usize) -> rusqlite::Result<Vec<NoteCopie>> {
    conn.prepare(
        "SELECT c.date, s.id, coalesce(json_extract(s.sujet, '$.titre'), ''), s.type, s.matiere, c.note
         FROM copies c JOIN sujets s ON s.id = c.sujet_id WHERE c.note IS NOT NULL
         ORDER BY c.date DESC LIMIT ?1",
    )?
    .query_map([limite as i64], |r| {
        let type_id: String = r.get(3)?;
        Ok(NoteCopie {
            date: r.get(0)?,
            sujet_id: r.get(1)?,
            titre: r.get(2)?,
            type_nom: exercices::nom_du_type(&type_id).into(),
            matiere: r.get(4)?,
            note: r.get(5)?,
        })
    })?
    .collect()
}

pub fn supprimer_sujet(conn: &Connection, id: i64) -> rusqlite::Result<bool> {
    Ok(conn.execute("DELETE FROM sujets WHERE id = ?1", [id])? > 0)
}

pub fn enregistrer_copie(
    conn: &Connection,
    sujet_id: i64,
    date: i64,
    duree_ms: i64,
    reponses: &[String],
    resultats: &[ResultatQuestion],
) -> rusqlite::Result<Copie> {
    let note = exercices::note_sur_20(resultats);
    conn.execute(
        "INSERT INTO copies(sujet_id, date, duree_ms, reponses, resultats, note) VALUES (?1, ?2, ?3, ?4, ?5, ?6)",
        params![sujet_id, date, duree_ms, json(&reponses), json(&resultats), note],
    )?;
    Ok(Copie {
        id: conn.last_insert_rowid(),
        sujet_id,
        date,
        duree_ms,
        reponses: reponses.to_vec(),
        resultats: resultats.to_vec(),
        appreciation: String::new(),
        note,
    })
}

pub fn lire_copie(conn: &Connection, id: i64) -> rusqlite::Result<Option<Copie>> {
    conn.query_row(
        "SELECT id, sujet_id, date, duree_ms, reponses, resultats, appreciation, note FROM copies WHERE id = ?1",
        [id],
        |r| {
            Ok(Copie {
                id: r.get(0)?,
                sujet_id: r.get(1)?,
                date: r.get(2)?,
                duree_ms: r.get(3)?,
                reponses: lire_json(&r.get::<_, String>(4)?, 4)?,
                resultats: lire_json(&r.get::<_, String>(5)?, 5)?,
                appreciation: r.get(6)?,
                note: r.get(7)?,
            })
        },
    )
    .optional()
}

/// Enregistre les résultats modifiés (correction de l'IA, question retirée) et recalcule la note.
pub fn mettre_a_jour_copie(conn: &Connection, copie: &mut Copie) -> rusqlite::Result<()> {
    copie.note = exercices::note_sur_20(&copie.resultats);
    conn.execute(
        "UPDATE copies SET resultats = ?2, appreciation = ?3, note = ?4 WHERE id = ?1",
        params![copie.id, json(&copie.resultats), copie.appreciation, copie.note],
    )?;
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::db;
    use serde_json::json;

    #[test]
    fn sujet_et_copies_conserves() {
        let conn = db::ouvrir_en_memoire().unwrap();
        let sujet: Sujet = serde_json::from_value(json!({
            "titre": "La méiose", "consigne": "Réponds", "contexte": "", "variables": [], "graphique": null,
            "questions": [
                { "enonce": "Combien de gamètes ?", "type_reponse": "court", "reponse_attendue": "4", "points": 2, "source": "E1" },
                { "enonce": "Expliquer", "type_reponse": "texte", "reponse_attendue": "…", "points": 3, "source": "E1" }
            ]
        }))
        .unwrap();
        let chapitres = vec![ChapitreChoisi { matiere: "Spé SVT".into(), chapitre: Some("Chapitre 3 - La méiose".into()) }];
        let id = enregistrer_sujet(&conn, "definitions", "Spé SVT", &chapitres, "m", 15, &sujet, &[], 1000).unwrap();
        assert_eq!(titres_deja_proposes(&conn, "definitions", &chapitres).unwrap(), ["La méiose"]);
        assert!(titres_deja_proposes(&conn, "reperes", &chapitres).unwrap().is_empty());

        let reponses = vec!["4".to_string(), "Deux divisions".to_string()];
        let resultats = exercices::corriger_automatiquement(&sujet, &reponses, false);
        let mut copie = enregistrer_copie(&conn, id, 2000, 60_000, &reponses, &resultats).unwrap();
        assert_eq!(copie.note, Some(20.0));
        assert_eq!(copie.a_corriger(), [1]);

        copie.resultats[0].retiree = true;
        mettre_a_jour_copie(&conn, &mut copie).unwrap();
        assert_eq!(copie.note, None);

        let relu = lire_sujet(&conn, id).unwrap().unwrap();
        assert_eq!(relu.sujet, sujet);
        assert_eq!(relu.type_nom, "Définitions");
        assert_eq!(relu.copies.len(), 1);
        assert!(relu.copies[0].resultats[0].retiree);

        // La question 1 (fausse) va au carnet d'erreurs ; la rédaction pas encore corrigée, non.
        let faux = vec!["5".to_string(), String::new()];
        let resultats_faux = exercices::corriger_automatiquement(&sujet, &faux, false);
        let relu_sujet = lire_sujet(&conn, id).unwrap().unwrap();
        assert_eq!(noter_erreurs(&conn, &relu_sujet, &faux, &resultats_faux, &[1, 2], 3000).unwrap(), 1);
        let (genre, question, donnee): (String, String, String) = conn
            .query_row("SELECT genre, question, reponse_donnee FROM erreurs", [], |r| Ok((r.get(0)?, r.get(1)?, r.get(2)?)))
            .unwrap();
        assert_eq!((genre.as_str(), question.as_str(), donnee.as_str()), ("examen", "Combien de gamètes ?", "5"));
        let notes = dernieres_notes(&conn, 10).unwrap();
        assert!(notes.is_empty(), "la seule copie n'a plus de note (question retirée)");

        let liste = lister_sujets(&conn).unwrap();
        assert_eq!((liste[0].titre.as_str(), liste[0].questions, liste[0].copies), ("La méiose", 2, 1));
        assert!(supprimer_sujet(&conn, id).unwrap());
        assert!(lire_copie(&conn, copie.id).unwrap().is_none());
    }
}
