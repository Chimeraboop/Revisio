//! Requêtes de navigation sur l'index : arbre des matières, fiche d'une note,
//! résolution des liens `[[...]]`, rétroliens. Aucune écriture.

use rusqlite::{params, Connection, OptionalExtension};
use serde::Serialize;
use serde_json::Value;

#[derive(Debug, Clone, Serialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct NoteResume {
    pub chemin: String,
    pub titre: String,
    pub genre: String,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Groupe {
    pub nom: String,
    pub notes: Vec<NoteResume>,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Matiere {
    pub nom: String,
    /// Notes rangées directement dans le dossier de la matière (Accueil, méthodologie…).
    pub notes: Vec<NoteResume>,
    pub chapitres: Vec<Groupe>,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Arbre {
    pub matieres: Vec<Matiere>,
    /// Dossiers spéciaux (_Révisions, Web clip…) et notes à la racine du vault.
    pub autres: Vec<Groupe>,
}

/// Arbre matière → chapitre → notes (l'ordre d'affichage est décidé par l'interface).
pub fn arbre(conn: &Connection) -> rusqlite::Result<Arbre> {
    let mut req = conn.prepare("SELECT chemin, titre, genre, matiere, chapitre FROM notes ORDER BY chemin")?;
    let lignes = req.query_map([], |r| {
        Ok((
            NoteResume { chemin: r.get(0)?, titre: r.get(1)?, genre: r.get(2)? },
            r.get::<_, Option<String>>(3)?,
            r.get::<_, Option<String>>(4)?,
        ))
    })?;

    let mut arbre = Arbre { matieres: vec![], autres: vec![] };
    for ligne in lignes {
        let (note, matiere, chapitre) = ligne?;
        match matiere {
            Some(m) => {
                let mat = match arbre.matieres.iter().position(|x| x.nom == m) {
                    Some(i) => &mut arbre.matieres[i],
                    None => {
                        arbre.matieres.push(Matiere { nom: m, notes: vec![], chapitres: vec![] });
                        arbre.matieres.last_mut().unwrap()
                    }
                };
                match chapitre {
                    Some(c) => ajouter(&mut mat.chapitres, c, note),
                    None => mat.notes.push(note),
                }
            }
            None => ajouter(&mut arbre.autres, chapitre.unwrap_or_else(|| "Racine du vault".into()), note),
        }
    }
    Ok(arbre)
}

fn ajouter(groupes: &mut Vec<Groupe>, nom: String, note: NoteResume) {
    match groupes.iter_mut().find(|g| g.nom == nom) {
        Some(g) => g.notes.push(note),
        None => groupes.push(Groupe { nom, notes: vec![note] }),
    }
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct FicheNote {
    pub chemin: String,
    pub nom: String,
    pub titre: String,
    pub matiere: Option<String>,
    pub chapitre: Option<String>,
    pub genre: String,
    pub tags: Vec<String>,
    pub frontmatter: Value,
    /// Pour un schéma : ses légendes ; pour un cours : la note dont il est le schéma, etc.
    pub legendes: Vec<String>,
    pub retroliens: Vec<NoteResume>,
}

/// Informations indexées sur une note (le contenu Markdown est lu à part, dans le vault).
pub fn fiche(conn: &Connection, chemin: &str) -> rusqlite::Result<Option<FicheNote>> {
    let ligne = conn
        .query_row(
            "SELECT id, nom, titre, matiere, chapitre, genre, tags, frontmatter FROM notes WHERE chemin = ?1",
            [chemin],
            |r| {
                Ok((
                    r.get::<_, i64>(0)?,
                    r.get::<_, String>(1)?,
                    r.get::<_, String>(2)?,
                    r.get::<_, Option<String>>(3)?,
                    r.get::<_, Option<String>>(4)?,
                    r.get::<_, String>(5)?,
                    r.get::<_, String>(6)?,
                    r.get::<_, String>(7)?,
                ))
            },
        )
        .optional()?;
    let Some((id, nom, titre, matiere, chapitre, genre, tags, frontmatter)) = ligne else {
        return Ok(None);
    };

    let legendes = if genre == "schema" {
        conn.query_row("SELECT texte FROM sections WHERE note_id = ?1 AND genre = 'legendes'", [id], |r| {
            r.get::<_, String>(0)
        })
        .optional()?
        .map(|t| t.lines().map(str::to_owned).collect())
        .unwrap_or_default()
    } else {
        vec![]
    };

    let sans_md = chemin.strip_suffix(".md").unwrap_or(chemin);
    let mut req = conn.prepare(
        "SELECT DISTINCT n.chemin, n.titre, n.genre FROM liens l JOIN notes n ON n.id = l.source_id
         WHERE (l.cible = ?1 COLLATE NOCASE OR l.cible = ?2 COLLATE NOCASE)
           AND n.id != ?3 AND n.genre != 'accueil'
         ORDER BY n.titre",
    )?;
    let retroliens = req
        .query_map(params![nom, sans_md, id], |r| {
            Ok(NoteResume { chemin: r.get(0)?, titre: r.get(1)?, genre: r.get(2)? })
        })?
        .collect::<rusqlite::Result<Vec<_>>>()?;

    Ok(Some(FicheNote {
        chemin: chemin.to_string(),
        nom,
        titre,
        matiere,
        chapitre,
        genre,
        tags: serde_json::from_str(&tags).unwrap_or_default(),
        frontmatter: serde_json::from_str(&frontmatter).unwrap_or(Value::Null),
        legendes,
        retroliens,
    }))
}

#[derive(Debug, Serialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct Cible {
    pub chemin: String,
    /// "note" ou "fichier" (image…).
    pub genre: String,
}

/// Résout la cible d'un lien `[[...]]` comme Obsidian : par chemin s'il y en a un,
/// sinon par nom de fichier ; en cas d'homonymes, on préfère le même dossier que la note source.
pub fn resoudre(conn: &Connection, cible: &str, source: Option<&str>) -> rusqlite::Result<Option<Cible>> {
    let cible = cible.trim().trim_start_matches('/');
    let sans_md = cible.strip_suffix(".md").unwrap_or(cible);
    let dossier_source = source.and_then(|s| s.rsplit_once('/').map(|(d, _)| d)).unwrap_or("");

    // 1. Chemin complet (liens HTML des pages d'accueil, ou [[Dossier/Note]]).
    if cible.contains('/') {
        let trouve: Option<String> = conn
            .query_row(
                "SELECT chemin FROM notes WHERE chemin = ?1 COLLATE NOCASE",
                [format!("{sans_md}.md")],
                |r| r.get(0),
            )
            .optional()?;
        if let Some(chemin) = trouve {
            return Ok(Some(Cible { chemin, genre: "note".into() }));
        }
        let trouve: Option<String> = conn
            .query_row("SELECT chemin FROM fichiers WHERE chemin = ?1 COLLATE NOCASE", [cible], |r| r.get(0))
            .optional()?;
        if let Some(chemin) = trouve {
            return Ok(Some(Cible { chemin, genre: "fichier".into() }));
        }
    }

    // 2. Par nom (dernier morceau du chemin).
    let nom = sans_md.rsplit('/').next().unwrap_or(sans_md);
    let mut req = conn.prepare("SELECT chemin FROM notes WHERE nom = ?1 COLLATE NOCASE")?;
    let candidats: Vec<String> = req.query_map([nom], |r| r.get(0))?.collect::<rusqlite::Result<_>>()?;
    if let Some(chemin) = meilleur(candidats, dossier_source) {
        return Ok(Some(Cible { chemin, genre: "note".into() }));
    }
    let nom_fichier = cible.rsplit('/').next().unwrap_or(cible);
    let mut req = conn.prepare("SELECT chemin FROM fichiers WHERE nom = ?1 COLLATE NOCASE")?;
    let candidats: Vec<String> = req.query_map([nom_fichier], |r| r.get(0))?.collect::<rusqlite::Result<_>>()?;
    Ok(meilleur(candidats, dossier_source).map(|chemin| Cible { chemin, genre: "fichier".into() }))
}

/// Parmi des homonymes, celui qui partage le plus long début de chemin avec la note source.
fn meilleur(candidats: Vec<String>, dossier_source: &str) -> Option<String> {
    candidats.into_iter().max_by_key(|c| {
        let commun = c
            .split('/')
            .zip(dossier_source.split('/'))
            .take_while(|(a, b)| a == b)
            .count();
        (commun, std::cmp::Reverse(c.len()))
    })
}

/// Lien `obsidian://open?vault=...&file=...` (chemin sans « .md », tout encodé).
pub fn url_obsidian(nom_vault: &str, chemin: &str) -> String {
    let fichier = chemin.strip_suffix(".md").unwrap_or(chemin);
    format!("obsidian://open?vault={}&file={}", encoder(nom_vault), encoder(fichier))
}

fn encoder(texte: &str) -> String {
    texte
        .bytes()
        .map(|b| match b {
            b'A'..=b'Z' | b'a'..=b'z' | b'0'..=b'9' | b'-' | b'_' | b'.' | b'~' => (b as char).to_string(),
            _ => format!("%{b:02X}"),
        })
        .collect()
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
    fn arbre_par_matiere_et_chapitre() {
        let a = arbre(&base_exemple()).unwrap();
        let svt = a.matieres.iter().find(|m| m.nom == "Spé SVT").unwrap();
        assert_eq!(svt.notes[0].titre, "Accueil");
        assert_eq!(svt.chapitres[0].nom, "Chapitre 3 - La méiose");
        assert_eq!(svt.chapitres[0].notes.len(), 3);
        assert!(a.autres.iter().any(|g| g.nom == "_Révisions"));
    }

    #[test]
    fn resolution_des_liens() {
        let conn = base_exemple();
        let c = resoudre(&conn, "La méiose dans le cycle reproductif", None).unwrap().unwrap();
        assert_eq!(c.chemin, "Spé SVT/Chapitre 3 - La méiose/La méiose dans le cycle reproductif.md");
        let c = resoudre(&conn, "la méiose (schéma).excalidraw", None).unwrap().unwrap();
        assert!(c.chemin.ends_with("La méiose (schéma).excalidraw.md"));
        let c = resoudre(&conn, "Spé SVT/Chapitre 3 - La méiose/Fiche de synthèse - Chapitre 3 La méiose.md", None)
            .unwrap()
            .unwrap();
        assert_eq!(c.genre, "note");
        let c = resoudre(&conn, "pixel.png", None).unwrap().unwrap();
        assert_eq!((c.chemin.as_str(), c.genre.as_str()), ("_Assets/pixel.png", "fichier"));
        assert!(resoudre(&conn, "Note inexistante", None).unwrap().is_none());
    }

    #[test]
    fn fiche_avec_retroliens_et_legendes() {
        let conn = base_exemple();
        let f = fiche(&conn, "Spé SVT/Chapitre 3 - La méiose/La méiose dans le cycle reproductif.md")
            .unwrap()
            .unwrap();
        let sources: Vec<_> = f.retroliens.iter().map(|n| n.titre.as_str()).collect();
        assert_eq!(sources, vec!["Fiche de synthèse - Chapitre 3 La méiose", "Semaine du 20 au 26 septembre"]);
        assert_eq!(f.tags, vec!["svt", "méiose"]);
        let s = fiche(&conn, "Spé SVT/Chapitre 3 - La méiose/La méiose (schéma).excalidraw.md").unwrap().unwrap();
        assert_eq!(s.legendes, vec!["Cellule mère (2n)", "Gamètes (n)"]);
    }

    #[test]
    fn homonymes_meme_dossier_prefere() {
        let candidats = vec!["A/x/Note.md".to_string(), "B/y/Note.md".to_string()];
        assert_eq!(meilleur(candidats, "B/y").as_deref(), Some("B/y/Note.md"));
    }

    #[test]
    fn lien_obsidian_encode() {
        assert_eq!(
            url_obsidian("mon-vault", "Spé SVT/La méiose.md"),
            "obsidian://open?vault=mon-vault&file=Sp%C3%A9%20SVT%2FLa%20m%C3%A9iose"
        );
    }
}
