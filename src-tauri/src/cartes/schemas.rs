//! Schémas muets : les légendes d'un schéma Excalidraw sont cachées et numérotées,
//! il faut les retrouver. Le dessin lui-même est affiché par l'interface.

use serde_json::json;

use super::{normaliser_cle, CarteGeneree, NoteSource, SectionSource};

/// Une légende à retrouver : courte (les titres et phrases longues restent visibles comme repères).
pub fn est_legende_a_retrouver(texte: &str) -> bool {
    let mots = texte.split_whitespace().count();
    (1..=6).contains(&mots) && texte.chars().count() <= 60 && texte.chars().any(char::is_alphabetic)
}

pub fn cartes(note: &NoteSource, sections: &[SectionSource]) -> Vec<CarteGeneree> {
    if note.genre != "schema" {
        return vec![];
    }
    let Some(section) = sections.iter().find(|s| s.genre == "legendes") else { return vec![] };
    let mut legendes: Vec<String> = Vec::new();
    for l in section.texte.lines().map(str::trim).filter(|l| est_legende_a_retrouver(l)) {
        if !legendes.iter().any(|x| normaliser_cle(x) == normaliser_cle(l)) {
            legendes.push(l.to_string());
        }
    }
    if legendes.len() < 2 {
        return vec![];
    }
    vec![CarteGeneree {
        cle: format!("schema|{}", note.chemin),
        genre: "schema",
        section: String::new(),
        question: note.titre.clone(),
        reponse: legendes.join(" · "),
        donnees: json!({ "legendes": legendes }),
        extrait: section.texte.clone(),
    }]
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn legendes_courtes_seulement() {
        assert!(est_legende_a_retrouver("Prophase I"));
        assert!(est_legende_a_retrouver("appariement des homologues (chiasma)"));
        assert!(!est_legende_a_retrouver("Les étapes de la méiose — reproduction des croquis de l'élève"));
        assert!(!est_legende_a_retrouver("(1)"));
    }

    #[test]
    fn une_carte_par_schema() {
        let note = NoteSource {
            id: 1,
            chemin: "SVT/La méiose (schéma).excalidraw.md".into(),
            titre: "La méiose (schéma)".into(),
            matiere: Some("Spé SVT".into()),
            chapitre: None,
            genre: "schema".into(),
            tags: vec![],
        };
        let s = SectionSource {
            titre: "Légendes du schéma".into(),
            genre: "legendes".into(),
            type_callout: None,
            texte: "Les étapes de la méiose — reproduction des croquis de l'élève\nProphase I\nMétaphase I\nProphase I".into(),
        };
        let c = cartes(&note, &[s]);
        assert_eq!(c.len(), 1);
        assert_eq!(c[0].donnees["legendes"], json!(["Prophase I", "Métaphase I"]));
        // Moins de 2 légendes : pas de carte.
        let s1 = SectionSource { titre: String::new(), genre: "legendes".into(), type_callout: None, texte: "Seule".into() };
        assert!(cartes(&note, &[s1]).is_empty());
    }
}
