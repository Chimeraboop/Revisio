//! Cartes de vocabulaire (allemand / anglais ↔ français), dans les deux sens.
//!
//! Formats reconnus (observés dans le vault) :
//! - `* die Verfolgung = la persécution` (Wortschatz), avec `≠` ou `;` pour plusieurs paires ;
//! - `die Grenze (n), la frontière` (notes « (Quizlet) ») : virgule hors parenthèses ;
//! - bloc de code `because,parce que` (export Quizlet dans une note de cours).

use serde_json::json;

use super::{decouper_hors_parentheses, normaliser_cle, sans_chevrons, CarteGeneree, NoteSource, SectionSource};

#[derive(Debug, Clone, PartialEq)]
pub struct Paire {
    pub etranger: String,
    pub francais: String,
    pub section: String,
    pub ligne: String,
}

/// Note qui contient du vocabulaire à apprendre.
pub fn est_source(note: &NoteSource) -> bool {
    note.genre == "vocabulaire"
        || note
            .tags
            .iter()
            .any(|t| matches!(t.to_lowercase().as_str(), "vocabulaire" | "wortschatz" | "quizlet"))
}

/// (code, nom) de la langue étrangère, d'après la matière.
pub fn langue(note: &NoteSource) -> (&'static str, &'static str) {
    let m = note.matiere.as_deref().unwrap_or("").to_lowercase();
    if m.contains("anglais") || note.tags.iter().any(|t| t == "anglais") {
        ("en", "Anglais")
    } else {
        ("de", "Allemand")
    }
}

/// Matière où l'on apprend une langue étrangère (ou une DNL enseignée dans cette langue) : le vocabulaire s'y
/// révise seulement en cartes recto / verso, comme dans Quizlet (demandé le 2026-11-01).
pub fn matiere_de_langue(matiere: Option<&str>) -> bool {
    let m = matiere.unwrap_or("").to_lowercase();
    m.contains("allemand") || m.contains("anglais") || m.contains("dnl")
}

/// Note de cours d'une langue dont les lignes « mot = traduction » deviennent aussi des cartes recto / verso.
pub fn est_cours_de_langue(note: &NoteSource) -> bool {
    !est_source(note) && matiere_de_langue(note.matiere.as_deref())
}

pub fn cartes(note: &NoteSource, sections: &[SectionSource]) -> Vec<CarteGeneree> {
    cartes_depuis(note, paires(&note.titre, sections))
}

/// Cartes du vocabulaire glissé dans une note de cours de langue.
pub fn cartes_de_cours(note: &NoteSource, sections: &[SectionSource]) -> Vec<CarteGeneree> {
    cartes_depuis(note, paires_de_cours(sections))
}

fn cartes_depuis(note: &NoteSource, paires: Vec<Paire>) -> Vec<CarteGeneree> {
    let (code, nom) = langue(note);
    let mut cartes = Vec::new();
    for p in paires {
        let base = normaliser_cle(&p.etranger);
        for (sens, question, reponse) in [
            ("vers_fr", &p.etranger, &p.francais),
            ("depuis_fr", &p.francais, &p.etranger),
        ] {
            cartes.push(CarteGeneree {
                cle: format!("vocab|{}|{}|{}", note.chemin, base, sens),
                genre: "vocabulaire",
                section: p.section.clone(),
                question: question.clone(),
                reponse: reponse.clone(),
                donnees: json!({ "sens": sens, "langue": code, "nomLangue": nom }),
                extrait: p.ligne.clone(),
            });
        }
    }
    cartes
}

/// Toutes les paires mot / traduction d'une note.
pub fn paires(titre_note: &str, sections: &[SectionSource]) -> Vec<Paire> {
    let mut code = Vec::new();
    let mut texte = Vec::new();
    for s in sections {
        let mut dans_code = false;
        for ligne in s.texte.lines() {
            let l = sans_chevrons(ligne);
            if l.trim_start().starts_with("```") {
                dans_code = !dans_code;
                continue;
            }
            if dans_code {
                code.push((s.titre.as_str(), l));
            } else {
                texte.push((s.titre.as_str(), l));
            }
        }
    }

    // Un export Quizlet en bloc de code est la source la plus fiable : on ne prend alors que lui.
    let depuis_code: Vec<Paire> = code
        .iter()
        .filter_map(|(sec, l)| paire_virgule(l).map(|(e, f)| paire(e, f, sec, l)))
        .collect();
    if !depuis_code.is_empty() {
        return depuis_code;
    }

    let format_quizlet = titre_note.contains("(Quizlet)");
    let mut paires = Vec::new();
    for (sec, l) in texte {
        if let Some(liste) = paires_egal(l) {
            paires.extend(liste.into_iter().map(|(e, f)| paire(&e, &f, sec, l)));
        } else if format_quizlet {
            if let Some((e, f)) = paire_virgule(l) {
                paires.push(paire(e, f, sec, l));
            }
        }
    }
    paires
}

/// Paires d'une note de cours, plus prudemment que dans une liste de vocabulaire : seulement les éléments de
/// liste (`* mot = traduction`) et les lignes qui commencent par le mot en gras ou en italique
/// (`**die Amtssprache** = la langue officielle ; **die Minderheit** = la minorité`). Les phrases du cours qui
/// contiennent un « = » au milieu (« L'allemand (die Amtssprache = langue officielle) est… ») sont ignorées.
pub fn paires_de_cours(sections: &[SectionSource]) -> Vec<Paire> {
    let mut paires = Vec::new();
    for s in sections {
        let mut dans_code = false;
        for ligne in s.texte.lines() {
            let l = sans_chevrons(ligne).trim();
            if l.starts_with("```") {
                dans_code = !dans_code;
                continue;
            }
            let element = ["* ", "- ", "+ "].iter().find_map(|p| l.strip_prefix(p));
            let l = element.unwrap_or(l).trim();
            if dans_code || (element.is_none() && !l.starts_with('*')) || l.contains('$') || l.contains('→') {
                continue;
            }
            // Plusieurs paires sur la ligne, séparées par « ; » : chacune a son « = ».
            let morceaux = decouper_hors_parentheses(l, ";");
            let morceaux = if morceaux.len() > 1 && morceaux.iter().all(|m| m.contains(" = ")) { morceaux } else { vec![l] };
            for m in morceaux {
                let Some((gauche, droite)) = m.split_once(" = ") else { continue };
                if gauche.contains(" : ") || droite.contains(" = ") {
                    continue; // « **Développement** : … (x = y) » : une phrase du cours, pas du vocabulaire
                }
                // Point final retiré, mais pas les points de suspension (« pensé que... »).
                let droite = match droite.strip_suffix('.') {
                    Some(d) if !d.ends_with('.') => d,
                    _ => droite,
                };
                // Remarque en italique à la fin (« quitter son pays *(attention : haben)* ») : pas à apprendre.
                let droite = match droite.rfind(" *(") {
                    Some(i) if droite.ends_with(")*") => &droite[..i],
                    _ => droite,
                };
                let texte = format!("{} = {}", sans_emphase(gauche), sans_emphase(droite));
                for (e, f) in paires_egal(&texte).unwrap_or_default() {
                    paires.push(paire(&e, &f, &s.titre, l));
                }
            }
        }
    }
    paires
}

/// `**mot**` ou `*expression*` → le texte seul. Un astérisque isolé (`verlassen*`, verbe fort) est gardé.
fn sans_emphase(texte: &str) -> &str {
    let t = texte.trim();
    for marque in ["**", "*", "_"] {
        if let Some(sans) = t.strip_prefix(marque).and_then(|x| x.strip_suffix(marque)) {
            if !sans.is_empty() && !sans.contains(marque) {
                return sans.trim();
            }
        }
    }
    t
}

fn paire(etranger: &str, francais: &str, section: &str, ligne: &str) -> Paire {
    Paire {
        etranger: etranger.trim().to_string(),
        francais: francais.trim().to_string(),
        section: section.to_string(),
        ligne: ligne.trim().to_string(),
    }
}

/// `mot, traduction` : exactement une virgule hors parenthèses, pas de « : ».
fn paire_virgule(ligne: &str) -> Option<(&str, &str)> {
    let l = ligne.trim();
    if l.is_empty() || l.contains(':') || l.starts_with(['#', '*', '[', '!', '|', '-']) {
        return None;
    }
    match decouper_hors_parentheses(l, ",").as_slice() {
        [e, f] if !e.is_empty() && !f.is_empty() && e.chars().count() <= 80 => Some((e, f)),
        _ => None,
    }
}

/// `* mot = traduction`, éventuellement plusieurs paires : `a; b = x; y`, ou `a ≠ b = x ≠ y`
/// quand les éléments opposés ont une longueur comparable. Les virgules ne sont jamais
/// découpées : « mit dem Schiff, Zug, Auto fahren = partir en bateau, train, voiture »
/// est une seule expression.
fn paires_egal(ligne: &str) -> Option<Vec<(String, String)>> {
    let l = ligne.trim();
    let l = l
        .strip_prefix("* ")
        .or_else(|| l.strip_prefix("- "))
        .or_else(|| l.strip_prefix("+ "))
        .unwrap_or(l)
        .trim();
    if l.contains('$') || l.starts_with('[') {
        return None; // formule ou en-tête de callout
    }
    let (gauche, droite) = l.split_once(" = ")?;
    let (gauche, droite) = (gauche.trim(), droite.trim());
    if gauche.is_empty() || droite.is_empty() || gauche.chars().count() > 80 {
        return None;
    }
    for separateur in [";", "≠"] {
        let g = decouper_hors_parentheses(gauche, separateur);
        let d = decouper_hors_parentheses(droite, separateur);
        let symetrique = separateur == ";" || (longueurs_comparables(&g) && longueurs_comparables(&d));
        if g.len() >= 2 && g.len() == d.len() && symetrique && g.iter().chain(&d).all(|x| !x.is_empty()) {
            return Some(g.into_iter().zip(d).map(|(a, b)| (a.to_string(), b.to_string())).collect());
        }
    }
    Some(vec![(gauche.to_string(), droite.to_string())])
}

/// « friedliche ≠ kriegerische Beziehungen haben » : 1 mot contre 3, le nom est sous-entendu
/// d'un côté → on ne découpe pas.
fn longueurs_comparables(morceaux: &[&str]) -> bool {
    let mots: Vec<usize> = morceaux.iter().map(|m| m.split_whitespace().count()).collect();
    mots.iter().max().unwrap_or(&0) - mots.iter().min().unwrap_or(&0) <= 1
}

#[cfg(test)]
mod tests {
    use super::*;

    fn section(texte: &str) -> SectionSource {
        SectionSource { titre: "S".into(), genre: "titre".into(), type_callout: None, texte: texte.into() }
    }

    fn liste(titre: &str, texte: &str) -> Vec<(String, String)> {
        paires(titre, &[section(texte)]).into_iter().map(|p| (p.etranger, p.francais)).collect()
    }

    #[test]
    fn format_wortschatz() {
        let p = liste(
            "Wortschatz",
            "### A2-B1\n* weg/fahren* = s'en aller, partir\n* aus/wandern, emigrieren ≠ ein/wandern, immigrieren = émigrer ≠ immigrer\n* jm begegnen; die Begegnung = rencontrer qn ; la rencontre\n* seine Heimat, seine Verwandten verlassen* = quitter son pays, sa famille",
        );
        assert_eq!(
            p,
            vec![
                ("weg/fahren*".into(), "s'en aller, partir".into()),
                ("aus/wandern, emigrieren".into(), "émigrer".into()),
                ("ein/wandern, immigrieren".into(), "immigrer".into()),
                ("jm begegnen".into(), "rencontrer qn".into()),
                ("die Begegnung".into(), "la rencontre".into()),
                ("seine Heimat, seine Verwandten verlassen*".into(), "quitter son pays, sa famille".into()),
            ]
        );
    }

    #[test]
    fn format_quizlet() {
        let p = liste(
            "Vocabulaire allemand - Fuite et frontières (Quizlet)",
            "Note pour Quizlet : dans l'import Quizlet, choisis \"virgule\", puis colle la liste.\n\ndie Grenze (n), la frontière\nüberschreiten (überschritt, hat überschritten), franchir\n> [!tip]+ Notes liées\n> [[Max Ernst]]",
        );
        assert_eq!(
            p,
            vec![
                ("die Grenze (n)".into(), "la frontière".into()),
                ("überschreiten (überschritt, hat überschritten)".into(), "franchir".into()),
            ]
        );
    }

    #[test]
    fn bloc_quizlet_prioritaire() {
        let p = liste(
            "Linking words",
            "> because, owing to, for = parce que, en raison de, car\n> ```\n> because,parce que\n> owing to,en raison de\n> ```",
        );
        assert_eq!(p, vec![("because".into(), "parce que".into()), ("owing to".into(), "en raison de".into())]);
    }

    #[test]
    fn expressions_factorisees_non_decoupees() {
        let p = liste(
            "Wortschatz",
            "* mit dem Schiff, Zug, Auto fahren* = partir en bateau, train, voiture\n* friedliche ≠ kriegerische Beziehungen haben = avoir des relations pacifiques ≠ conflictuelles\n* bestimmt, sicher(lich) = sûrement",
        );
        assert_eq!(
            p,
            vec![
                ("mit dem Schiff, Zug, Auto fahren*".into(), "partir en bateau, train, voiture".into()),
                (
                    "friedliche ≠ kriegerische Beziehungen haben".into(),
                    "avoir des relations pacifiques ≠ conflictuelles".into()
                ),
                ("bestimmt, sicher(lich)".into(), "sûrement".into()),
            ]
        );
    }

    #[test]
    fn vocabulaire_glisse_dans_un_cours() {
        let p: Vec<(String, String)> = paires_de_cours(&[section(
            "> **die Amtssprache** = la langue officielle ; **die Minderheit** = la minorité.\n\
             > **Deutschland: ein Auswanderungsland** = l'Allemagne, un pays d'émigration.\n\
             > * *Ich hätte nie gedacht, dass...* = je n'aurais jamais pensé que...\n\
             > * die Heimat verlassen* = quitter son pays *(attention : haben)*\n\
             > * aus/wandern ≠ ein/wandern = émigrer ≠ immigrer\n\
             L'allemand (die Amtssprache = langue officielle) est langue officielle dans 6 pays.\n\
             > * **Raisons économiques** (aus Gründen) : Sie fanden keine Arbeit = ils ne trouvaient pas de travail\n\
             > * **Développement** : connecteurs (*einerseits* = d'une part)\n\
             >   → \"on parle allemand\" = *dass Deutsch gesprochen wird.*\n\
             > * $E = mc^2$",
        )])
        .into_iter()
        .map(|p| (p.etranger, p.francais))
        .collect();
        assert_eq!(
            p,
            vec![
                ("die Amtssprache".into(), "la langue officielle".into()),
                ("die Minderheit".into(), "la minorité".into()),
                ("Deutschland: ein Auswanderungsland".into(), "l'Allemagne, un pays d'émigration".into()),
                ("Ich hätte nie gedacht, dass...".into(), "je n'aurais jamais pensé que...".into()),
                ("die Heimat verlassen*".into(), "quitter son pays".into()),
                ("aus/wandern".into(), "émigrer".into()),
                ("ein/wandern".into(), "immigrer".into()),
            ]
        );
    }

    #[test]
    fn matieres_de_langue() {
        assert!(matiere_de_langue(Some("Allemand")));
        assert!(matiere_de_langue(Some("DNL histoire géo")));
        assert!(!matiere_de_langue(Some("Spé SVT")));
        assert!(!matiere_de_langue(None));
    }

    #[test]
    fn deux_cartes_par_paire() {
        let note = NoteSource {
            id: 1,
            chemin: "Allemand/W.md".into(),
            titre: "Wortschatz - Test".into(),
            matiere: Some("Allemand".into()),
            chapitre: None,
            genre: "vocabulaire".into(),
            tags: vec![],
        };
        let c = cartes(&note, &[section("* die Verfolgung = la persécution")]);
        assert_eq!(c.len(), 2);
        assert_eq!((c[0].question.as_str(), c[0].reponse.as_str()), ("die Verfolgung", "la persécution"));
        assert_eq!((c[1].question.as_str(), c[1].reponse.as_str()), ("la persécution", "die Verfolgung"));
        assert_eq!(c[1].donnees["sens"], "depuis_fr");
        assert_eq!(c[0].cle, "vocab|Allemand/W.md|die verfolgung|vers_fr");
    }
}
