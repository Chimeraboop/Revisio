//! Après un renommage ou un déplacement : la base suit les fichiers (sans toucher au disque).
//!
//! Chaque carte de révision a un identifiant qui contient le chemin de sa note
//! (`trou|Matière/Chapitre/Note.md|…`) : sans cette mise à jour, renommer une note recréerait
//! ses cartes de zéro et effacerait la progression de l'élève.
//! Ce module s'occupe aussi des liens `[[Note]]` à mettre à jour quand une note change de nom.

use regex::Regex;
use rusqlite::{params, Connection};
use std::sync::LazyLock;

/// Nouveau chemin si `chemin` est `ancien` ou se trouve dans le dossier `ancien`.
pub fn chemin_deplace(chemin: &str, ancien: &str, nouveau: &str) -> Option<String> {
    if chemin == ancien {
        Some(nouveau.to_string())
    } else {
        chemin.strip_prefix(&format!("{ancien}/")).map(|reste| format!("{nouveau}/{reste}"))
    }
}

/// Met à jour toutes les références à `ancien` (fichier ou dossier) pour qu'elles pointent vers `nouveau`.
pub fn migrer_chemins(conn: &Connection, ancien: &str, nouveau: &str) -> rusqlite::Result<()> {
    // Cartes : chemin de la note et identifiant.
    let cartes: Vec<(i64, String, String)> = {
        let mut req = conn.prepare("SELECT id, cle, note_chemin FROM cartes")?;
        let lignes = req.query_map([], |r| Ok((r.get(0)?, r.get(1)?, r.get(2)?)))?;
        lignes.collect::<rusqlite::Result<_>>()?
    };
    for (id, cle, note) in cartes {
        let Some(nouveau_chemin) = chemin_deplace(&note, ancien, nouveau) else { continue };
        let ancienne_partie = format!("|{note}");
        let nouvelle_cle = if cle.contains(&ancienne_partie) {
            cle.replacen(&ancienne_partie, &format!("|{nouveau_chemin}"), 1)
        } else {
            cle.clone()
        };
        // Identifiant déjà pris (cas très rare) : on garde l'ancien, la carte reste rattachée à la note.
        let ok = conn.execute(
            "UPDATE cartes SET cle = ?2, note_chemin = ?3 WHERE id = ?1",
            params![id, nouvelle_cle, nouveau_chemin],
        );
        if ok.is_err() {
            conn.execute("UPDATE cartes SET note_chemin = ?2 WHERE id = ?1", params![id, nouveau_chemin])?;
        }
    }
    // Carnet d'erreurs, propositions de l'IA, historique (pour pouvoir encore annuler après un renommage).
    for (table, colonne, condition) in [
        ("erreurs", "note_chemin", ""),
        ("propositions_vault", "chemin", " AND statut = 'attente'"),
        ("historique_vault", "chemin", " AND genre IN ('creation', 'modification') AND annulee_le IS NULL"),
    ] {
        let lignes: Vec<(i64, String)> = {
            let mut req = conn.prepare(&format!("SELECT id, {colonne} FROM {table} WHERE 1 = 1{condition}"))?;
            let lignes = req.query_map([], |r| Ok((r.get(0)?, r.get(1)?)))?;
            lignes.collect::<rusqlite::Result<_>>()?
        };
        for (id, chemin) in lignes {
            if let Some(n) = chemin_deplace(&chemin, ancien, nouveau) {
                conn.execute(&format!("UPDATE {table} SET {colonne} = ?2 WHERE id = ?1"), params![id, n])?;
            }
        }
    }
    migrer_evaluations(conn, ancien, nouveau)
}

/// Le planning des évaluations désigne matières et chapitres par leur nom (= nom du dossier).
fn migrer_evaluations(conn: &Connection, ancien: &str, nouveau: &str) -> rusqlite::Result<()> {
    let a: Vec<&str> = ancien.split('/').collect();
    let n: Vec<&str> = nouveau.split('/').collect();
    match (a.as_slice(), n.as_slice()) {
        // Dossier de matière renommé.
        ([matiere], [nouvelle]) => {
            conn.execute("UPDATE evaluations SET matiere = ?2 WHERE matiere = ?1", params![matiere, nouvelle])?;
        }
        // Dossier de chapitre renommé dans la même matière.
        ([matiere, chapitre], [meme, nouveau_chapitre]) if matiere == meme => {
            let evals: Vec<(i64, String)> = {
                let mut req = conn.prepare("SELECT id, chapitres FROM evaluations WHERE matiere = ?1")?;
                let lignes = req.query_map([matiere], |r| Ok((r.get(0)?, r.get(1)?)))?;
                lignes.collect::<rusqlite::Result<_>>()?
            };
            for (id, json) in evals {
                let Ok(mut chapitres) = serde_json::from_str::<Vec<Option<String>>>(&json) else { continue };
                let mut change = false;
                for c in chapitres.iter_mut().flatten() {
                    if c == chapitre {
                        *c = nouveau_chapitre.to_string();
                        change = true;
                    }
                }
                if change {
                    conn.execute(
                        "UPDATE evaluations SET chapitres = ?2 WHERE id = ?1",
                        params![id, serde_json::to_string(&chapitres).unwrap()],
                    )?;
                }
            }
        }
        _ => {}
    }
    Ok(())
}

static RE_LIEN: LazyLock<Regex> = LazyLock::new(|| Regex::new(r"(!?\[\[)([^\[\]|#^\n]+?)(\s*[\]|#^])").unwrap());

/// Remplace dans `texte` les liens vers la note `ancien_nom` par `nouveau_nom` (noms sans « .md »).
/// Garde le chemin éventuel (`[[Dossier/Note]]`), l'ancre et l'alias. `None` si rien ne change.
pub fn remplacer_liens(texte: &str, ancien_nom: &str, nouveau_nom: &str) -> Option<String> {
    let ancien = ancien_nom.to_lowercase();
    let mut change = false;
    let resultat = RE_LIEN.replace_all(texte, |c: &regex::Captures| {
        let cible = c[2].trim();
        let (prefixe, nom) = match cible.rsplit_once('/') {
            Some((p, n)) => (format!("{p}/"), n),
            None => (String::new(), cible),
        };
        let nom_sans_md = nom.strip_suffix(".md").unwrap_or(nom);
        if nom_sans_md.to_lowercase() == ancien {
            change = true;
            format!("{}{prefixe}{nouveau_nom}{}", &c[1], &c[3])
        } else {
            c[0].to_string()
        }
    });
    change.then(|| resultat.into_owned())
}

/// Notes qui contiennent un lien vers `nom` (d'après l'index).
pub fn notes_qui_lient(conn: &Connection, nom: &str) -> rusqlite::Result<Vec<String>> {
    let mut req = conn.prepare("SELECT DISTINCT n.chemin, l.cible FROM liens l JOIN notes n ON n.id = l.source_id")?;
    let lignes = req.query_map([], |r| Ok((r.get::<_, String>(0)?, r.get::<_, String>(1)?)))?;
    let nom = nom.to_lowercase();
    let mut chemins: Vec<String> = Vec::new();
    for ligne in lignes {
        let (chemin, cible) = ligne?;
        let dernier = cible.rsplit('/').next().unwrap_or(&cible);
        let dernier = dernier.strip_suffix(".md").unwrap_or(dernier);
        if dernier.to_lowercase() == nom && !chemins.contains(&chemin) {
            chemins.push(chemin);
        }
    }
    Ok(chemins)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::db;

    #[test]
    fn liens_mis_a_jour() {
        let texte = "Voir [[La méiose]], [[Spé SVT/La méiose|la méiose]], [[la méiose#Étapes]] et ![[La méiose]].\n\
                     Pas [[La méiose 2]] ni [[Autre]].";
        assert_eq!(
            remplacer_liens(texte, "La méiose", "Méiose").unwrap(),
            "Voir [[Méiose]], [[Spé SVT/Méiose|la méiose]], [[Méiose#Étapes]] et ![[Méiose]].\n\
             Pas [[La méiose 2]] ni [[Autre]]."
        );
        assert_eq!(remplacer_liens("Rien à voir [[Autre]]", "La méiose", "Méiose"), None);
    }

    #[test]
    fn chemins_deplaces() {
        assert_eq!(chemin_deplace("SVT/Chap/Note.md", "SVT/Chap/Note.md", "SVT/Note.md").as_deref(), Some("SVT/Note.md"));
        assert_eq!(chemin_deplace("SVT/Chap/Note.md", "SVT/Chap", "SVT/Chapitre 1").as_deref(), Some("SVT/Chapitre 1/Note.md"));
        assert_eq!(chemin_deplace("SVT/Chapitre/Note.md", "SVT/Chap", "X"), None, "même début de nom, autre dossier");
    }

    #[test]
    fn la_progression_suit_la_note_renommee() {
        let conn = db::ouvrir_en_memoire().unwrap();
        conn.execute(
            "INSERT INTO cartes(cle, genre, note_chemin, matiere, chapitre, question, reponse, creee_le, etat_fsrs)
             VALUES ('trou|SVT/Chap/Méiose.md|haploïde|0', 'trou', 'SVT/Chap/Méiose.md', 'SVT', 'Chap', 'q', 'r', 0, '{\"x\":1}'),
                    ('ia|42|0', 'qcm', 'SVT/Chap/Méiose.md', 'SVT', 'Chap', 'q', 'r', 0, NULL),
                    ('trou|SVT/Autre.md|x|0', 'trou', 'SVT/Autre.md', 'SVT', NULL, 'q', 'r', 0, NULL)",
            [],
        )
        .unwrap();
        conn.execute(
            "INSERT INTO evaluations(jour, matiere, titre, genre, chapitres, creee_le)
             VALUES ('2026-11-02', 'SVT', 'Contrôle', 'controle', '[\"Chap\", null]', 0)",
            [],
        )
        .unwrap();
        // Le chapitre est renommé : la carte garde son état FSRS, sous son nouvel identifiant.
        migrer_chemins(&conn, "SVT/Chap", "SVT/Chapitre 4").unwrap();
        let (cle, etat): (String, Option<String>) = conn
            .query_row("SELECT cle, etat_fsrs FROM cartes WHERE genre = 'trou' AND note_chemin LIKE 'SVT/Chapitre 4/%'", [], |r| {
                Ok((r.get(0)?, r.get(1)?))
            })
            .unwrap();
        assert_eq!(cle, "trou|SVT/Chapitre 4/Méiose.md|haploïde|0");
        assert_eq!(etat.as_deref(), Some("{\"x\":1}"));
        let ia: String = conn.query_row("SELECT note_chemin FROM cartes WHERE cle = 'ia|42|0'", [], |r| r.get(0)).unwrap();
        assert_eq!(ia, "SVT/Chapitre 4/Méiose.md");
        let autre: String = conn.query_row("SELECT cle FROM cartes WHERE note_chemin = 'SVT/Autre.md'", [], |r| r.get(0)).unwrap();
        assert_eq!(autre, "trou|SVT/Autre.md|x|0");
        let chapitres: String = conn.query_row("SELECT chapitres FROM evaluations", [], |r| r.get(0)).unwrap();
        assert_eq!(chapitres, "[\"Chapitre 4\",null]");
        // La matière est renommée.
        migrer_chemins(&conn, "SVT", "Spé SVT").unwrap();
        let matiere: String = conn.query_row("SELECT matiere FROM evaluations", [], |r| r.get(0)).unwrap();
        assert_eq!(matiere, "Spé SVT");
    }
}
