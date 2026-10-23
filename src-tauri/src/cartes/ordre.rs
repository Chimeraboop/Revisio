//! Remise dans l'ordre : étapes d'une liste numérotée, ou événements d'une chronologie.
//!
//! - Liste numérotée `1. … 2. … 3. …` (3 à 8 éléments) : les étapes sont mélangées.
//! - Chronologie `- **5 mai** : …` (3 à 8 éléments) : les dates sont cachées pendant
//!   l'exercice et réapparaissent à la correction.

use std::sync::LazyLock;

use regex::Regex;
use serde_json::json;

use super::{normaliser_cle, sans_chevrons, titre_lisible, CarteGeneree, NoteSource, SectionSource};

static RE_NUMERO: LazyLock<Regex> = LazyLock::new(|| Regex::new(r"^(\d{1,2})[.)]\s+(.+)$").unwrap());
static RE_DATE: LazyLock<Regex> =
    LazyLock::new(|| Regex::new(r"^[-*+]\s+\*\*([^*]*\d[^*]*)\*\*\s*[:—–-]\s*(.+)$").unwrap());

const MIN: usize = 3;
const MAX: usize = 8;
const LONGUEUR_MAX: usize = 300;

#[derive(Debug, Clone, PartialEq)]
pub struct Suite {
    /// "etapes" ou "chronologie".
    pub genre: &'static str,
    pub elements: Vec<String>,
    /// Pour une chronologie : la date de chaque élément.
    pub dates: Vec<String>,
    /// La ligne de texte juste avant la liste (ex. « **Étapes de réalisation d'un caryotype :** »).
    pub intro: String,
}

/// Une liste numérotée n'est un vrai ordre à retenir que si son contexte le dit
/// (sinon ce sont souvent des phrases d'exercice ou une simple énumération).
const MOTS_ORDRE: &[&str] = &[
    "étape", "methode", "méthode", "protocole", "plan", "chronolog", "déroulement", "procédure", "cycle", "phase",
    "ordre", "réalisation", "succession", "démarche", "algorithme",
];

fn contexte_ordonne(textes: &[&str]) -> bool {
    textes.iter().any(|t| {
        let t = t.to_lowercase();
        MOTS_ORDRE.iter().any(|m| t.contains(m))
    })
}

static RE_ENTETE_CALLOUT: LazyLock<Regex> = LazyLock::new(|| Regex::new(r"^\[![\w-]+\][+-]?\s*").unwrap());

fn nettoyer_intro(intro: &str) -> String {
    let sans_entete = RE_ENTETE_CALLOUT.replace(intro.trim(), "");
    sans_entete.replace("**", "").trim().trim_end_matches(':').trim().to_string()
}

pub fn est_source(note: &NoteSource) -> bool {
    !matches!(note.genre.as_str(), "vocabulaire" | "schema" | "accueil" | "revision")
}

pub fn cartes(note: &NoteSource, sections: &[SectionSource]) -> Vec<CarteGeneree> {
    let mut cartes = Vec::new();
    for s in sections {
        let titre_section = if s.titre.trim().is_empty() { note.titre.clone() } else { titre_lisible(&s.titre) };
        for (i, suite) in suites(&s.texte).into_iter().enumerate() {
            if suite.genre == "etapes" && !contexte_ordonne(&[&note.titre, &s.titre, &suite.intro]) {
                continue;
            }
            // Titre affiché : la phrase d'introduction si elle annonce la liste, sinon le titre de section.
            let intro = nettoyer_intro(&suite.intro);
            let courte = !intro.is_empty() && intro.chars().count() <= 80;
            let titre = if courte && contexte_ordonne(&[&intro]) { intro } else { titre_section.clone() };
            let consigne = if suite.genre == "chronologie" {
                "Remets ces événements dans l'ordre chronologique"
            } else {
                "Remets ces étapes dans l'ordre"
            };
            let reponse = if suite.genre == "chronologie" {
                suite.dates.iter().zip(&suite.elements).map(|(d, e)| format!("{d} : {e}")).collect::<Vec<_>>().join(" → ")
            } else {
                suite.elements.join(" → ")
            };
            cartes.push(CarteGeneree {
                cle: format!("ordre|{}|{}|{}", note.chemin, normaliser_cle(&titre), i),
                genre: "ordre",
                section: titre.clone(),
                question: consigne.to_string(),
                reponse,
                donnees: json!({ "type": suite.genre, "elements": suite.elements, "dates": suite.dates }),
                extrait: s.texte.clone(),
            });
        }
    }
    cartes
}

/// Les listes ordonnées (numérotées ou datées) d'un texte.
pub fn suites(texte: &str) -> Vec<Suite> {
    let mut resultat = Vec::new();
    let mut courante: Option<Suite> = None;
    let mut dans_code = false;
    let mut derniere_ligne = String::new();

    // Une liste terminée n'est gardée que si elle a 3 à 8 éléments pas trop longs.
    fn clore(s: Option<Suite>, resultat: &mut Vec<Suite>) {
        if let Some(s) = s {
            if (MIN..=MAX).contains(&s.elements.len()) && s.elements.iter().all(|e| e.chars().count() <= LONGUEUR_MAX) {
                resultat.push(s);
            }
        }
    }

    for ligne in texte.lines() {
        let brute = sans_chevrons(ligne);
        if brute.trim_start().starts_with("```") {
            dans_code = !dans_code;
            continue;
        }
        if dans_code {
            continue;
        }
        if brute.trim().is_empty() {
            continue; // une ligne vide n'interrompt pas une liste
        }
        if brute.starts_with([' ', '\t']) {
            continue; // sous-élément ou suite d'un élément
        }
        let l = brute.trim();
        if let Some(c) = RE_NUMERO.captures(l) {
            let n: usize = c[1].parse().unwrap_or(0);
            match courante.as_mut() {
                Some(s) if s.genre == "etapes" && n == s.elements.len() + 1 => s.elements.push(c[2].trim().to_string()),
                _ => {
                    clore(courante.take(), &mut resultat);
                    if n == 1 {
                        courante = Some(Suite {
                            genre: "etapes",
                            elements: vec![c[2].trim().to_string()],
                            dates: vec![],
                            intro: derniere_ligne.clone(),
                        });
                    }
                }
            }
            continue;
        }
        if let Some(c) = RE_DATE.captures(l) {
            let date = c[1].trim().to_string();
            let evenement = c[2].trim().to_string();
            match courante.as_mut() {
                Some(s) if s.genre == "chronologie" => {
                    s.dates.push(date);
                    s.elements.push(evenement);
                }
                _ => {
                    clore(courante.take(), &mut resultat);
                    courante = Some(Suite {
                        genre: "chronologie",
                        elements: vec![evenement],
                        dates: vec![date],
                        intro: derniere_ligne.clone(),
                    });
                }
            }
            continue;
        }
        clore(courante.take(), &mut resultat);
        derniere_ligne = l.to_string();
    }
    clore(courante.take(), &mut resultat);
    resultat
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn liste_numerotee() {
        let s = suites("> [!info]+ Les 5 étapes\n> 1. **Inventio** : trouver\n> 2. Dispositio\n>    - détail\n> 3. Elocutio\n> 4. Memoria\n> 5. Actio\n");
        assert_eq!(s.len(), 1);
        assert_eq!(s[0].genre, "etapes");
        assert_eq!(s[0].elements, vec!["**Inventio** : trouver", "Dispositio", "Elocutio", "Memoria", "Actio"]);
    }

    #[test]
    fn chronologie_avec_dates() {
        let s = suites("Chronologie clé :\n- **5 mai** : ouverture des États généraux.\n- **17 juin** : Assemblée nationale.\n- **20 juin** : **Serment du Jeu de paume**.\n\nLe roi reconnaît…");
        assert_eq!(s.len(), 1);
        assert_eq!(s[0].genre, "chronologie");
        assert_eq!(s[0].dates, vec!["5 mai", "17 juin", "20 juin"]);
        assert_eq!(s[0].elements[2], "**Serment du Jeu de paume**.");
    }

    #[test]
    fn listes_trop_courtes_ou_trop_longues_ignorees() {
        assert!(suites("1. a\n2. b\n").is_empty());
        let longue: String = (1..=12).map(|i| format!("{i}. étape {i}\n")).collect();
        assert!(suites(&longue).is_empty());
        // Une numérotation qui ne commence pas à 1 n'est pas une liste d'étapes.
        assert!(suites("3. c\n4. d\n5. e\n").is_empty());
    }

    #[test]
    fn seules_les_listes_annoncees_comme_ordonnees() {
        let note = NoteSource {
            id: 1,
            chemin: "n.md".into(),
            titre: "Le caryotype".into(),
            matiere: None,
            chapitre: None,
            genre: "cours".into(),
            tags: vec![],
        };
        let section = |titre: &str, texte: &str| SectionSource {
            titre: titre.into(),
            genre: "titre".into(),
            type_callout: None,
            texte: texte.into(),
        };
        let etapes = section("Le caryotype", "**Étapes de réalisation d'un caryotype :**\n1. Prélèvement.\n2. Arrêt.\n3. Coloration.");
        let exercice = section("Consigne", "Complète :\n1. Viele Deutsche ___.\n2. Sie ___.\n3. Manche ___.");
        let c = cartes(&note, &[etapes, exercice]);
        assert_eq!(c.len(), 1);
        assert_eq!(c[0].section, "Étapes de réalisation d'un caryotype");
        assert_eq!(nettoyer_intro("[!abstract]+ Plan du passage :"), "Plan du passage");
    }

    #[test]
    fn deux_listes_dans_une_section() {
        let s = suites("1. a\n2. b\n3. c\nTexte entre deux.\n1. x\n2. y\n3. z\n4. w\n");
        assert_eq!(s.iter().map(|x| x.elements.len()).collect::<Vec<_>>(), vec![3, 4]);
    }
}
