//! Textes à trous : un mot en **gras** est caché dans sa phrase.
//!
//! Sources : toutes les sections des fiches de synthèse (prioritaires), puis les
//! callouts « Définition » des notes de cours.

use std::collections::HashMap;
use std::sync::LazyLock;

use regex::Regex;
use serde_json::json;

use super::{normaliser_cle, sans_chevrons, titre_lisible, CarteGeneree, NoteSource, SectionSource};

/// Ce qui remplace le mot caché dans la question (affiché surligné par l'interface).
pub const TROU: &str = "==……==";

static RE_GRAS: LazyLock<Regex> = LazyLock::new(|| Regex::new(r"\*\*([^*\n]+?)\*\*").unwrap());
static RE_WIKILIEN: LazyLock<Regex> = LazyLock::new(|| Regex::new(r"!?\[\[([^\[\]|]+)(?:\|([^\[\]]+))?\]\]").unwrap());
static RE_PUCE: LazyLock<Regex> = LazyLock::new(|| Regex::new(r"^(?:[-*+]|\d+[.)])\s+").unwrap());

/// Abréviations suivies d'un point qui ne terminent pas une phrase.
const ABREVIATIONS: &[&str] = &["ex", "cf", "etc", "env", "av", "apr", "chap", "p", "fig", "doc", "n°", "vol", "éd", "mme", "m", "dr", "st", "ste"];

pub fn est_source(note: &NoteSource) -> bool {
    !matches!(note.genre.as_str(), "vocabulaire" | "schema" | "accueil" | "revision")
}

pub fn cartes(note: &NoteSource, sections: &[SectionSource]) -> Vec<CarteGeneree> {
    let fiche = note.genre == "fiche_synthese";
    let mut occurrences: HashMap<String, usize> = HashMap::new();
    let mut cartes = Vec::new();
    for s in sections {
        let definition = s.genre == "callout" && s.titre.to_lowercase().starts_with("définition");
        if !fiche && !definition {
            continue;
        }
        for phrase in phrases_de_section(&s.texte) {
            for (question, terme) in trous_de_phrase(&phrase) {
                let cle_terme = normaliser_cle(&terme);
                let n = occurrences.entry(cle_terme.clone()).or_default();
                *n += 1;
                cartes.push(CarteGeneree {
                    cle: format!("trou|{}|{}|{}", note.chemin, cle_terme, n),
                    genre: "trou",
                    section: texte_simple(&s.titre),
                    question,
                    reponse: terme,
                    donnees: json!({ "source": if fiche { "fiche" } else { "definition" } }),
                    extrait: s.texte.clone(),
                });
            }
        }
    }
    cartes
}

/// Phrases d'une section (hors titres, en-têtes de callout, code et ligne « Repère cours »).
fn phrases_de_section(texte: &str) -> Vec<String> {
    let mut resultat = Vec::new();
    let mut dans_code = false;
    for ligne in texte.lines() {
        let l = sans_chevrons(ligne).trim();
        if l.starts_with("```") {
            dans_code = !dans_code;
            continue;
        }
        if dans_code || l.is_empty() || l.starts_with('#') || l.starts_with("[!") || l.starts_with("*Repère") || l.starts_with('|') {
            continue;
        }
        let l = RE_PUCE.replace(l, "");
        resultat.extend(decouper_phrases(&l).into_iter().map(str::to_string));
    }
    resultat
}

/// Découpe une ligne en phrases, sans couper dans le gras, les formules ou les parenthèses.
pub fn decouper_phrases(ligne: &str) -> Vec<&str> {
    let mut phrases = Vec::new();
    let mut debut = 0;
    let mut gras = false;
    let mut maths = false;
    let mut parentheses = 0i32;
    let caracteres: Vec<(usize, char)> = ligne.char_indices().collect();
    for (k, &(i, c)) in caracteres.iter().enumerate() {
        match c {
            '*' if ligne[i..].starts_with("**") && !ligne[..i].ends_with('*') => gras = !gras,
            '$' => maths = !maths,
            '(' => parentheses += 1,
            ')' => parentheses -= 1,
            '.' | '!' | '?' if !gras && !maths && parentheses <= 0 => {
                let suivant = caracteres.get(k + 1).map(|x| x.1);
                let apres = caracteres.get(k + 2).map(|x| x.1);
                let fin_de_phrase = suivant == Some(' ')
                    && apres.is_some_and(|a| a.is_uppercase() || a == '«' || a.is_ascii_digit());
                let mot = ligne[debut..i].rsplit([' ', '(']).next().unwrap_or("").to_lowercase();
                let abreviation = ABREVIATIONS.contains(&mot.as_str()) || mot.chars().count() == 1;
                if fin_de_phrase && !(c == '.' && abreviation) {
                    phrases.push(ligne[debut..=i].trim());
                    debut = i + c.len_utf8();
                }
            }
            _ => {}
        }
    }
    let reste = ligne[debut..].trim();
    if !reste.is_empty() {
        phrases.push(reste);
    }
    phrases
}

/// Pour chaque mot en gras utilisable : (phrase avec le trou, mot caché).
pub fn trous_de_phrase(phrase: &str) -> Vec<(String, String)> {
    let phrase = RE_WIKILIEN.replace_all(phrase, |c: &regex::Captures| {
        c.get(2).map_or(c[1].trim(), |a| a.as_str().trim()).to_string()
    });
    let nb_mots = texte_simple(&phrase).split_whitespace().count();
    let longueur = phrase.chars().count();
    let mut resultat = Vec::new();
    for m in RE_GRAS.find_iter(&phrase) {
        let terme = m.as_str().trim_matches('*').trim().trim_end_matches([':', ',', ';']).trim();
        let mots = terme.split_whitespace().count();
        let utilisable = (2..=60).contains(&terme.chars().count())
            && mots <= 6
            && !terme.contains('$')
            && nb_mots >= 6
            && (m.as_str().chars().count() * 10) < longueur * 6; // le gras ne doit pas être toute la phrase
        if utilisable {
            let question = format!("{}{}{}", &phrase[..m.start()], TROU, &phrase[m.end()..]);
            resultat.push((question, terme.to_string()));
        }
    }
    resultat
}

fn texte_simple(t: &str) -> String {
    titre_lisible(t)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn decoupage_en_phrases() {
        assert_eq!(
            decouper_phrases("La **méiose** produit 4 gamètes (ex. chez l'Homme). Elle a lieu dans les gonades. Fin"),
            vec!["La **méiose** produit 4 gamètes (ex. chez l'Homme).", "Elle a lieu dans les gonades.", "Fin"]
        );
        assert_eq!(decouper_phrases("Voir le chap. 2 pour la suite."), vec!["Voir le chap. 2 pour la suite."]);
        assert_eq!(decouper_phrases("Si $x. Y$ alors ok. Puis"), vec!["Si $x. Y$ alors ok.", "Puis"]);
    }

    #[test]
    fn un_trou_par_mot_en_gras() {
        let t = trous_de_phrase(
            "La **méiose** permet de passer d'une cellule **diploïde** (2n) à 4 gamètes haploïdes, voir [[La méiose dans le cycle reproductif|cycle]].",
        );
        assert_eq!(t.len(), 2);
        assert_eq!(t[0].1, "méiose");
        assert!(t[0].0.starts_with("La ==……== permet"));
        assert!(t[0].0.contains("**diploïde**"));
        assert!(t[0].0.ends_with("voir cycle."));
        assert_eq!(t[1].1, "diploïde");
    }

    #[test]
    fn gras_inutilisable_ignore() {
        // Phrase entièrement en gras, phrase trop courte, formule.
        assert!(trous_de_phrase("**Méiose et fécondation assurent ensemble la stabilité du caryotype.**").is_empty());
        assert!(trous_de_phrase("Le **discriminant** Δ.").is_empty());
        assert!(trous_de_phrase("On calcule le nombre **$\\Delta$** puis on conclut sur les racines.").is_empty());
    }

    #[test]
    fn sources_fiche_et_definitions() {
        let note = |genre: &str| NoteSource {
            id: 1,
            chemin: "SVT/n.md".into(),
            titre: "n".into(),
            matiere: Some("Spé SVT".into()),
            chapitre: None,
            genre: genre.into(),
            tags: vec![],
        };
        let def = SectionSource {
            titre: "Définition".into(),
            genre: "callout".into(),
            type_callout: Some("info".into()),
            texte: "> [!info]+ Définition\n> Une **mutation** est un changement dans la séquence d'un gène.".into(),
        };
        let exemple = SectionSource {
            titre: "Exemple".into(),
            genre: "callout".into(),
            type_callout: Some("example".into()),
            texte: "> [!example]+ Exemple\n> Le gène **AP3** forme les pétales de la fleur chez Arabidopsis.".into(),
        };
        let c = cartes(&note("cours"), &[def.clone(), exemple.clone()]);
        assert_eq!(c.iter().map(|x| x.reponse.as_str()).collect::<Vec<_>>(), vec!["mutation"]);
        assert_eq!(c[0].section, "Définition");
        // Dans une fiche de synthèse, toutes les sections comptent.
        assert_eq!(cartes(&note("fiche_synthese"), &[def, exemple]).len(), 2);
    }
}
