//! Fabrication des cartes de révision à partir de l'index, sans IA et hors ligne.
//!
//! Chaque générateur (vocabulaire, textes à trous…) lit les sections d'une note
//! et propose des cartes. Chaque carte a une clé stable : quand la note change,
//! la carte est mise à jour sans perdre sa progression (état FSRS).

pub mod ordre;
pub mod schemas;
pub mod trous;
pub mod vocabulaire;

use std::collections::{BTreeMap, HashMap, HashSet};

use rusqlite::{params, Connection};
use serde::Serialize;
use serde_json::Value;

/// Note telle que lue dans l'index.
#[derive(Debug, Clone)]
pub struct NoteSource {
    pub id: i64,
    pub chemin: String,
    pub titre: String,
    pub matiere: Option<String>,
    pub chapitre: Option<String>,
    pub genre: String,
    pub tags: Vec<String>,
}

#[derive(Debug, Clone)]
pub struct SectionSource {
    pub titre: String,
    pub genre: String,
    pub type_callout: Option<String>,
    pub texte: String,
}

#[derive(Debug, Clone, PartialEq)]
pub struct CarteGeneree {
    pub cle: String,
    pub genre: &'static str,
    pub section: String,
    pub question: String,
    pub reponse: String,
    pub donnees: Value,
    pub extrait: String,
}

#[derive(Debug, Default, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct RapportCartes {
    pub actives: usize,
    pub nouvelles: usize,
    pub desactivees: usize,
    pub par_genre: BTreeMap<String, usize>,
}

/// Toutes les cartes qu'une note peut produire.
pub fn cartes_de_note(note: &NoteSource, sections: &[SectionSource]) -> Vec<CarteGeneree> {
    if note.genre == "accueil" || note.genre == "revision" {
        return vec![];
    }
    let mut cartes = Vec::new();
    if note.genre == "schema" {
        cartes.extend(schemas::cartes(note, sections));
    } else if vocabulaire::est_source(note) {
        cartes.extend(vocabulaire::cartes(note, sections));
    } else if trous::est_source(note) {
        cartes.extend(trous::cartes(note, sections));
    }
    if ordre::est_source(note) {
        cartes.extend(ordre::cartes(note, sections));
    }
    // Une même clé ne doit apparaître qu'une fois.
    let mut vues = HashSet::new();
    cartes.retain(|c| vues.insert(c.cle.clone()));
    cartes
}

/// Recalcule les cartes de tout le vault et met à jour la table `cartes`.
/// Les cartes qui ne sont plus produites sont désactivées (jamais supprimées : on garde l'historique).
pub fn synchroniser(conn: &mut Connection, maintenant: i64) -> Result<RapportCartes, String> {
    let (notes, sections) = charger_sources(conn).map_err(|e| e.to_string())?;
    let mut generees: Vec<(&NoteSource, CarteGeneree)> = Vec::new();
    for note in &notes {
        let secs = sections.get(&note.id).map(Vec::as_slice).unwrap_or(&[]);
        for c in cartes_de_note(note, secs) {
            generees.push((note, c));
        }
    }

    let tx = conn.transaction().map_err(|e| e.to_string())?;
    let existantes: HashMap<String, (i64, bool)> = {
        let mut req = tx.prepare("SELECT cle, id, active FROM cartes").map_err(|e| e.to_string())?;
        let lignes = req
            .query_map([], |r| Ok((r.get::<_, String>(0)?, (r.get::<_, i64>(1)?, r.get::<_, bool>(2)?))))
            .map_err(|e| e.to_string())?;
        lignes.filter_map(Result::ok).collect()
    };

    let mut rapport = RapportCartes::default();
    let mut produites = HashSet::new();
    for (note, c) in &generees {
        produites.insert(c.cle.as_str());
        *rapport.par_genre.entry(c.genre.to_string()).or_default() += 1;
        let donnees = c.donnees.to_string();
        match existantes.get(&c.cle) {
            Some((id, _)) => {
                tx.execute(
                    "UPDATE cartes SET genre=?2, note_chemin=?3, matiere=?4, chapitre=?5, section=?6,
                            question=?7, reponse=?8, donnees=?9, extrait=?10, active=1
                     WHERE id=?1",
                    params![id, c.genre, note.chemin, note.matiere, note.chapitre, c.section, c.question, c.reponse, donnees, c.extrait],
                )
                .map_err(|e| e.to_string())?;
            }
            None => {
                tx.execute(
                    "INSERT INTO cartes(cle, genre, note_chemin, matiere, chapitre, section, question, reponse, donnees, extrait, creee_le)
                     VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10, ?11)",
                    params![c.cle, c.genre, note.chemin, note.matiere, note.chapitre, c.section, c.question, c.reponse, donnees, c.extrait, maintenant],
                )
                .map_err(|e| e.to_string())?;
                rapport.nouvelles += 1;
            }
        }
    }
    for (cle, (id, active)) in &existantes {
        // Les cartes créées par l'IA (« ia|… ») ne viennent pas des générateurs : on n'y touche pas.
        if *active && !cle.starts_with("ia|") && !produites.contains(cle.as_str()) {
            tx.execute("UPDATE cartes SET active = 0 WHERE id = ?1", [id]).map_err(|e| e.to_string())?;
            rapport.desactivees += 1;
        }
    }
    tx.commit().map_err(|e| e.to_string())?;
    rapport.actives = produites.len();
    Ok(rapport)
}

type Sources = (Vec<NoteSource>, HashMap<i64, Vec<SectionSource>>);

fn charger_sources(conn: &Connection) -> rusqlite::Result<Sources> {
    let mut req = conn.prepare("SELECT id, chemin, titre, matiere, chapitre, genre, tags FROM notes ORDER BY chemin")?;
    let notes = req
        .query_map([], |r| {
            Ok(NoteSource {
                id: r.get(0)?,
                chemin: r.get(1)?,
                titre: r.get(2)?,
                matiere: r.get(3)?,
                chapitre: r.get(4)?,
                genre: r.get(5)?,
                tags: serde_json::from_str(&r.get::<_, String>(6)?).unwrap_or_default(),
            })
        })?
        .collect::<rusqlite::Result<Vec<_>>>()?;
    let mut req = conn.prepare("SELECT note_id, titre, genre, type_callout, texte FROM sections ORDER BY note_id, ordre")?;
    let mut sections: HashMap<i64, Vec<SectionSource>> = HashMap::new();
    for ligne in req.query_map([], |r| {
        Ok((
            r.get::<_, i64>(0)?,
            SectionSource { titre: r.get(1)?, genre: r.get(2)?, type_callout: r.get(3)?, texte: r.get(4)? },
        ))
    })? {
        let (id, s) = ligne?;
        sections.entry(id).or_default().push(s);
    }
    Ok((notes, sections))
}

/// Retire les chevrons de citation `> ` en début de ligne (contenu des callouts).
pub fn sans_chevrons(ligne: &str) -> &str {
    let mut l = ligne.trim_start();
    while let Some(reste) = l.strip_prefix('>') {
        l = reste.strip_prefix(' ').unwrap_or(reste);
    }
    l
}

/// Découpe `texte` sur `separateur`, en ignorant ceux qui sont entre parenthèses.
pub fn decouper_hors_parentheses<'a>(texte: &'a str, separateur: &str) -> Vec<&'a str> {
    let mut morceaux = Vec::new();
    let mut profondeur = 0i32;
    let mut debut = 0;
    let mut caracteres = texte.char_indices().peekable();
    while let Some((i, c)) = caracteres.next() {
        match c {
            '(' | '[' => profondeur += 1,
            ')' | ']' => profondeur -= 1,
            _ => {}
        }
        if profondeur == 0 && texte[i..].starts_with(separateur) {
            morceaux.push(texte[debut..i].trim());
            debut = i + separateur.len();
            // On saute les caractères du séparateur.
            while caracteres.peek().is_some_and(|&(j, _)| j < debut) {
                caracteres.next();
            }
        }
    }
    morceaux.push(texte[debut..].trim());
    morceaux
}

/// Titre lisible : sans gras ni italique, liens `[[cible|alias]]` remplacés par leur texte.
pub fn titre_lisible(t: &str) -> String {
    static RE_LIEN: std::sync::LazyLock<regex::Regex> =
        std::sync::LazyLock::new(|| regex::Regex::new(r"!?\[\[([^\[\]|]+)(?:\|([^\[\]]+))?\]\]").unwrap());
    let t = RE_LIEN.replace_all(t, |c: &regex::Captures| c.get(2).map_or(c[1].to_string(), |a| a.as_str().to_string()));
    t.replace("**", "").replace('*', "").trim().to_string()
}

/// Clé de comparaison : minuscules, espaces simplifiés.
pub fn normaliser_cle(texte: &str) -> String {
    texte.split_whitespace().collect::<Vec<_>>().join(" ").to_lowercase()
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{db, indexer};
    use std::path::Path;

    #[test]
    fn decoupage_hors_parentheses() {
        assert_eq!(
            decouper_hors_parentheses("überschreiten (überschritt, hat überschritten), franchir", ","),
            vec!["überschreiten (überschritt, hat überschritten)", "franchir"]
        );
        assert_eq!(decouper_hors_parentheses("a ≠ b", "≠"), vec!["a", "b"]);
        assert_eq!(sans_chevrons("> > texte"), "texte");
    }

    #[test]
    fn synchronisation_stable() {
        let mut conn = db::ouvrir_en_memoire().unwrap();
        let racine = Path::new(env!("CARGO_MANIFEST_DIR")).join("tests").join("vault_exemple");
        indexer::indexer(&mut conn, &racine).unwrap();
        let r1 = synchroniser(&mut conn, 1).unwrap();
        assert!(r1.nouvelles > 0);
        let r2 = synchroniser(&mut conn, 2).unwrap();
        assert_eq!((r2.nouvelles, r2.desactivees, r2.actives), (0, 0, r1.actives));
    }
}
