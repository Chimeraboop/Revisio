//! Analyse d'une note Obsidian (texte → structure). Aucune lecture de fichier ici :
//! ce module reçoit du texte et renvoie des données, ce qui le rend facile à tester.
//!
//! Formats observés dans le vault (octobre 2026) :
//! - frontmatter YAML (`tags` en liste ou `[a, b]`, `created`, `repere`, `banner`) ;
//! - une ligne en italique « *Repère cours : …* » en tête ;
//! - des callouts `> [!info]+ Titre` (définitions, propriétés, exemples…), souvent sans titres `#` ;
//! - LaTeX `$…$` / `$$…$$`, liens `[[cible|alias]]`, intégrations `![[schéma.excalidraw|800]]` ;
//! - pages « Accueil » en HTML (`<a data-href="….md">`) ;
//! - schémas `.excalidraw.md` : légendes en clair dans « ## Text Elements », dessin compressé entre `%%`.

use std::sync::LazyLock;

use regex::Regex;
use serde::Serialize;
use serde_json::{Map, Value};

#[derive(Debug, Clone, Serialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct Section {
    pub ordre: usize,
    /// "intro", "titre", "callout" ou "legendes" (schéma Excalidraw).
    pub genre: String,
    /// Niveau du titre `#` (0 pour intro / callout).
    pub niveau: u8,
    pub type_callout: Option<String>,
    pub titre: String,
    /// Contenu Markdown brut de la section.
    pub texte: String,
    /// Numéro de la première ligne (à partir de 1) dans le fichier.
    pub ligne: usize,
}

#[derive(Debug, Clone, Serialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct Lien {
    /// Cible telle qu'écrite (sans alias ni ancre), ex. `Forme canonique d'un polynôme du second degré`.
    pub cible: String,
    pub ancre: Option<String>,
    pub alias: Option<String>,
    /// `![[...]]` (intégration) plutôt que `[[...]]`.
    pub integre: bool,
}

#[derive(Debug, Clone, Default)]
pub struct NoteAnalysee {
    pub frontmatter: Map<String, Value>,
    pub tags: Vec<String>,
    pub sections: Vec<Section>,
    pub liens: Vec<Lien>,
}

static RE_TITRE: LazyLock<Regex> = LazyLock::new(|| Regex::new(r"^(#{1,6})\s+(.+?)\s*#*\s*$").unwrap());
static RE_CALLOUT: LazyLock<Regex> = LazyLock::new(|| Regex::new(r"^>\s*\[!([\w-]+)\]([+-]?)\s*(.*)$").unwrap());
static RE_WIKILIEN: LazyLock<Regex> = LazyLock::new(|| Regex::new(r"(!?)\[\[([^\[\]]+?)\]\]").unwrap());
static RE_DATA_HREF: LazyLock<Regex> = LazyLock::new(|| Regex::new(r#"data-href="([^"]+)""#).unwrap());
static RE_TAG: LazyLock<Regex> = LazyLock::new(|| Regex::new(r"(?:^|\s)#([\p{L}\p{N}_][\p{L}\p{N}_/-]*)").unwrap());
static RE_ID_BLOC: LazyLock<Regex> = LazyLock::new(|| Regex::new(r"\s*\^[\w-]+\s*$").unwrap());

/// Analyse complète d'une note.
pub fn analyser(texte: &str, est_schema: bool) -> NoteAnalysee {
    let texte = texte.replace("\r\n", "\n");
    let (frontmatter, corps, decalage) = separer_frontmatter(&texte);
    let mut tags = tags_du_frontmatter(&frontmatter);
    let sections = if est_schema {
        sections_schema(corps, decalage)
    } else {
        decouper_sections(corps, decalage)
    };
    for s in &sections {
        for t in tags_en_ligne(&texte_brut(&s.texte)) {
            if !tags.contains(&t) {
                tags.push(t);
            }
        }
    }
    let liens = extraire_liens(corps);
    NoteAnalysee { frontmatter, tags, sections, liens }
}

/// Sépare le frontmatter YAML du corps. Renvoie aussi le nombre de lignes du frontmatter.
pub fn separer_frontmatter(texte: &str) -> (Map<String, Value>, &str, usize) {
    if let Some(reste) = texte.strip_prefix("---\n") {
        if let Some(fin) = reste.find("\n---") {
            let yaml = &reste[..fin];
            let apres = &reste[fin + 4..];
            let corps = apres.strip_prefix('\n').unwrap_or(apres);
            let lignes = texte[..texte.len() - corps.len()].lines().count();
            return (yaml_vers_json(yaml), corps, lignes);
        }
    }
    (Map::new(), texte, 0)
}

fn yaml_vers_json(yaml: &str) -> Map<String, Value> {
    use yaml_rust2::{Yaml, YamlLoader};
    fn convertir(y: &Yaml) -> Value {
        match y {
            Yaml::String(s) | Yaml::Real(s) => Value::String(s.clone()),
            Yaml::Integer(i) => Value::from(*i),
            Yaml::Boolean(b) => Value::Bool(*b),
            Yaml::Array(a) => Value::Array(a.iter().map(convertir).collect()),
            Yaml::Hash(h) => Value::Object(
                h.iter()
                    .filter_map(|(k, v)| k.as_str().map(|k| (k.to_string(), convertir(v))))
                    .collect(),
            ),
            _ => Value::Null,
        }
    }
    match YamlLoader::load_from_str(yaml) {
        Ok(docs) => match docs.first().map(convertir) {
            Some(Value::Object(m)) => m,
            _ => Map::new(),
        },
        Err(_) => Map::new(), // frontmatter mal formé : on l'ignore sans planter
    }
}

fn tags_du_frontmatter(fm: &Map<String, Value>) -> Vec<String> {
    let brut: Vec<String> = match fm.get("tags") {
        Some(Value::Array(a)) => a.iter().filter_map(|v| v.as_str().map(str::to_owned)).collect(),
        Some(Value::String(s)) => s.split([',', ' ']).map(str::to_owned).collect(),
        _ => vec![],
    };
    let mut tags = Vec::new();
    for t in brut {
        let t = t.trim().trim_start_matches('#').to_string();
        if !t.is_empty() && !tags.contains(&t) {
            tags.push(t);
        }
    }
    tags
}

fn tags_en_ligne(texte: &str) -> Vec<String> {
    let sans_liens = RE_WIKILIEN.replace_all(texte, " ");
    RE_TAG
        .captures_iter(&sans_liens)
        .map(|c| c[1].to_string())
        .filter(|t| !t.chars().all(|c| c.is_ascii_digit())) // « #1 » n'est pas un tag
        .collect()
}

/// Liens `[[...]]`, `![[...]]` et liens HTML `data-href="....md"` des pages d'accueil.
pub fn extraire_liens(corps: &str) -> Vec<Lien> {
    let mut liens = Vec::new();
    for c in RE_WIKILIEN.captures_iter(corps) {
        let integre = &c[1] == "!";
        let interieur = &c[2];
        let (cible_ancre, alias) = match interieur.split_once('|') {
            Some((a, b)) => (a, Some(b.trim().to_string())),
            None => (interieur, None),
        };
        let (cible, ancre) = match cible_ancre.find(['#', '^']) {
            Some(i) => (&cible_ancre[..i], Some(cible_ancre[i..].trim_start_matches('#').to_string())),
            None => (cible_ancre, None),
        };
        let cible = cible.trim();
        if cible.is_empty() {
            continue; // lien interne à la note ([[#Titre]])
        }
        liens.push(Lien { cible: cible.to_string(), ancre, alias, integre });
    }
    for c in RE_DATA_HREF.captures_iter(corps) {
        let cible = c[1].trim_end_matches(".md").to_string();
        if !liens.iter().any(|l| l.cible == cible) {
            liens.push(Lien { cible, ancre: None, alias: None, integre: false });
        }
    }
    liens
}

/// Découpe le corps en sections : texte d'introduction, titres `#`, callouts `> [!type]`.
pub fn decouper_sections(corps: &str, decalage: usize) -> Vec<Section> {
    let mut sections: Vec<Section> = Vec::new();
    let mut courante = Section {
        ordre: 0,
        genre: "intro".into(),
        niveau: 0,
        type_callout: None,
        titre: String::new(),
        texte: String::new(),
        ligne: decalage + 1,
    };
    let mut dernier_titre: Option<(u8, String)> = None;
    let mut dans_code: Option<&str> = None;
    let mut dans_commentaire = false;
    let mut dans_callout = false;

    // Une section vide n'est gardée que si c'est un vrai titre (il structure la note).
    fn pousser(s: &mut Section, sections: &mut Vec<Section>) {
        if !s.texte.trim().is_empty() || s.genre == "titre" {
            s.ordre = sections.len();
            s.texte = s.texte.trim_end().to_string();
            sections.push(s.clone());
        }
    }

    for (i, ligne) in corps.lines().enumerate() {
        let numero = decalage + i + 1;
        let nette = ligne.trim_start();

        // Blocs de code ``` / ~~~ : on ne cherche ni titres ni callouts dedans.
        if let Some(clotureur) = dans_code {
            courante.texte.push_str(ligne);
            courante.texte.push('\n');
            if nette.starts_with(clotureur) {
                dans_code = None;
            }
            continue;
        }
        // Commentaires Obsidian %% ... %% : ignorés.
        if dans_commentaire || nette.starts_with("%%") {
            let nb = ligne.matches("%%").count();
            if dans_commentaire {
                if nb % 2 == 1 {
                    dans_commentaire = false;
                }
            } else if nb % 2 == 1 {
                dans_commentaire = true;
            }
            continue;
        }

        if dans_callout && !nette.starts_with('>') {
            // Fin du callout : la suite revient sous le dernier titre.
            pousser(&mut courante, &mut sections);
            dans_callout = false;
            // « suite » = reprise du dernier titre ; écartée si elle reste vide.
            let (niveau, titre) = dernier_titre.clone().unwrap_or((0, String::new()));
            courante = Section {
                ordre: 0,
                genre: if niveau > 0 { "suite".into() } else { "intro".into() },
                niveau,
                type_callout: None,
                titre,
                texte: String::new(),
                ligne: numero,
            };
        }

        if let Some(c) = RE_TITRE.captures(ligne) {
            pousser(&mut courante, &mut sections);
            let niveau = c[1].len() as u8;
            let titre = c[2].to_string();
            dernier_titre = Some((niveau, titre.clone()));
            courante = Section {
                ordre: 0,
                genre: "titre".into(),
                niveau,
                type_callout: None,
                titre,
                texte: String::new(),
                ligne: numero,
            };
            continue;
        }

        if let Some(c) = RE_CALLOUT.captures(ligne) {
            pousser(&mut courante, &mut sections);
            let type_callout = c[1].to_lowercase();
            let titre = if c[3].trim().is_empty() { titre_par_defaut(&type_callout) } else { c[3].trim().to_string() };
            courante = Section {
                ordre: 0,
                genre: "callout".into(),
                niveau: 0,
                type_callout: Some(type_callout),
                titre,
                texte: String::new(),
                ligne: numero,
            };
            courante.texte.push_str(ligne);
            courante.texte.push('\n');
            dans_callout = true;
            continue;
        }

        if nette.starts_with("```") || nette.starts_with("~~~") {
            dans_code = Some(if nette.starts_with("```") { "```" } else { "~~~" });
        }
        courante.texte.push_str(ligne);
        courante.texte.push('\n');
    }
    pousser(&mut courante, &mut sections);

    // Les sections « suite » non vides redeviennent des sections de titre.
    for s in &mut sections {
        if s.genre == "suite" {
            s.genre = "titre".into();
        }
    }
    sections
}

/// Schémas Excalidraw : on ne garde que les légendes (section « ## Text Elements »).
pub fn sections_schema(corps: &str, decalage: usize) -> Vec<Section> {
    let legendes = legendes_excalidraw(corps);
    if legendes.is_empty() {
        return vec![];
    }
    let ligne = corps
        .lines()
        .position(|l| l.trim() == "## Text Elements")
        .map(|p| decalage + p + 1)
        .unwrap_or(decalage + 1);
    vec![Section {
        ordre: 0,
        genre: "legendes".into(),
        niveau: 0,
        type_callout: None,
        titre: "Légendes du schéma".into(),
        texte: legendes.join("\n"),
        ligne,
    }]
}

/// Légendes d'un schéma Excalidraw. Deux formats existent dans le vault :
/// - section « ## Text Elements » en clair (dessin compressé en `compressed-json`) ;
/// - dessin en JSON non compressé (bloc ```json) sans section « Text Elements ».
pub fn legendes_excalidraw(corps: &str) -> Vec<String> {
    let legendes = legendes_section_texte(corps);
    if legendes.is_empty() { legendes_json(corps) } else { legendes }
}

/// Textes des éléments `"type": "text"` d'un dessin Excalidraw en JSON non compressé.
fn legendes_json(corps: &str) -> Vec<String> {
    let Some(debut) = corps.find("```json") else { return vec![] };
    let apres = &corps[debut + 7..];
    let Some(fin) = apres.find("\n```") else { return vec![] };
    let Ok(dessin) = serde_json::from_str::<Value>(apres[..fin].trim()) else { return vec![] };
    dessin["elements"]
        .as_array()
        .map(|elements| {
            elements
                .iter()
                .filter(|e| e["type"] == "text" && e["isDeleted"] != true)
                .filter_map(|e| e["originalText"].as_str().or_else(|| e["text"].as_str()))
                .map(|t| t.split_whitespace().collect::<Vec<_>>().join(" "))
                .filter(|t| !t.is_empty())
                .collect()
        })
        .unwrap_or_default()
}

/// Section « ## Text Elements », sans les identifiants `^abc` de fin de bloc.
fn legendes_section_texte(corps: &str) -> Vec<String> {
    let mut legendes = Vec::new();
    let mut dans_textes = false;
    let mut bloc = String::new();
    for ligne in corps.lines() {
        let t = ligne.trim();
        if t == "## Text Elements" {
            dans_textes = true;
            continue;
        }
        if !dans_textes {
            continue;
        }
        if t.starts_with("## ") || t.starts_with("%%") || t.starts_with("# ") {
            break;
        }
        if t.is_empty() {
            if !bloc.trim().is_empty() {
                legendes.push(bloc.trim().to_string());
            }
            bloc.clear();
            continue;
        }
        if !bloc.is_empty() {
            bloc.push(' ');
        }
        bloc.push_str(&RE_ID_BLOC.replace(t, ""));
    }
    if !bloc.trim().is_empty() {
        legendes.push(bloc.trim().to_string());
    }
    legendes
}

fn titre_par_defaut(type_callout: &str) -> String {
    match type_callout {
        "info" => "Info",
        "abstract" | "summary" | "tldr" => "Résumé",
        "tip" | "hint" => "Astuce",
        "example" => "Exemple",
        "note" => "Remarque",
        "question" | "faq" => "Question",
        "success" | "check" | "done" => "Réussite",
        "warning" | "caution" | "attention" => "Attention",
        "danger" | "error" => "Danger",
        "quote" | "cite" => "Citation",
        autre => autre,
    }
    .to_string()
}

/// Texte « nu » pour la recherche : sans syntaxe Markdown, HTML ni LaTeX superflu.
pub fn texte_brut(markdown: &str) -> String {
    static RE_HTML: LazyLock<Regex> = LazyLock::new(|| Regex::new(r"(?s)<!--.*?-->|<[^>]+>").unwrap());
    static RE_CALLOUT_ENTETE: LazyLock<Regex> = LazyLock::new(|| Regex::new(r"\[![\w-]+\][+-]?").unwrap());
    static RE_MARQUES: LazyLock<Regex> = LazyLock::new(|| Regex::new(r"\*{1,3}|__|==|~~|`").unwrap());
    static RE_TITRE_DEBUT: LazyLock<Regex> = LazyLock::new(|| Regex::new(r"^#{1,6}\s+").unwrap());
    static RE_ESPACES: LazyLock<Regex> = LazyLock::new(|| Regex::new(r"[ \t]{2,}").unwrap());
    let sans_html = RE_HTML.replace_all(markdown, " ");
    let sans_liens = RE_WIKILIEN.replace_all(&sans_html, |c: &regex::Captures| {
        let interieur = &c[2];
        let affiche = interieur.split_once('|').map(|(_, a)| a).unwrap_or(interieur);
        affiche.split(['#', '^']).next().unwrap_or("").to_string()
    });
    let mut sortie = String::with_capacity(sans_liens.len());
    for ligne in sans_liens.lines() {
        let l = ligne.trim_start().trim_start_matches('>').trim_start();
        let l = RE_TITRE_DEBUT.replace(l, "");
        let l = l.strip_prefix("* ").or_else(|| l.strip_prefix("- ")).unwrap_or(&l);
        let l = RE_CALLOUT_ENTETE.replace(l, "");
        let l = RE_MARQUES.replace_all(&l, "");
        let l = RE_ESPACES.replace_all(&l, " ");
        let l = l.trim();
        if !l.is_empty() {
            sortie.push_str(l);
            sortie.push('\n');
        }
    }
    sortie
}

#[cfg(test)]
mod tests {
    use super::*;

    const NOTE_COURS: &str = "---\ntags:\n  - maths\n  - polynôme\ncreated: 2026-10-12T14:05\nrepere: \"Chap. 1\"\n---\n\n*Repère cours : Chap. 1*\n\n> [!info]+ Définition 9 — Discriminant\n> Le réel $\\Delta = b^2-4ac$ est le **discriminant**.\n\n> [!tip]+ Notes liées\n> [[Forme canonique d'un polynôme du second degré|forme canonique]], [[Autre note#Section]]\n";

    #[test]
    fn frontmatter_et_tags() {
        let n = analyser(NOTE_COURS, false);
        assert_eq!(n.tags, vec!["maths", "polynôme"]);
        assert_eq!(n.frontmatter["repere"], "Chap. 1");
        let n2 = analyser("---\ntags: [synthese]\n---\n# T\n", false);
        assert_eq!(n2.tags, vec!["synthese"]);
    }

    #[test]
    fn sections_par_callout() {
        let n = analyser(NOTE_COURS, false);
        let genres: Vec<_> = n.sections.iter().map(|s| (s.genre.as_str(), s.titre.as_str())).collect();
        assert_eq!(
            genres,
            vec![("intro", ""), ("callout", "Définition 9 — Discriminant"), ("callout", "Notes liées")]
        );
        assert_eq!(n.sections[1].type_callout.as_deref(), Some("info"));
        assert_eq!(n.sections[1].ligne, 11);
    }

    #[test]
    fn sections_par_titre() {
        let texte = "# Fiche\n\n## 1. [[La méiose]]\nLa **méiose** permet…\n\n## 2. Étapes\nDeux divisions.\n```\n# pas un titre\n```\n";
        let n = analyser(texte, false);
        let titres: Vec<_> = n.sections.iter().map(|s| s.titre.as_str()).collect();
        assert_eq!(titres, vec!["Fiche", "1. [[La méiose]]", "2. Étapes"]);
        assert!(n.sections[2].texte.contains("# pas un titre"));
    }

    #[test]
    fn texte_apres_un_callout_reste_sous_son_titre() {
        let texte = "## Partie\n> [!note]+ Remarque\n> dedans\n\nDehors.\n";
        let n = analyser(texte, false);
        assert_eq!(n.sections.len(), 3);
        assert_eq!(n.sections[2].titre, "Partie");
        assert!(n.sections[2].texte.contains("Dehors."));
    }

    #[test]
    fn liens_wiki_et_html() {
        let n = analyser(NOTE_COURS, false);
        assert_eq!(n.liens[0].cible, "Forme canonique d'un polynôme du second degré");
        assert_eq!(n.liens[0].alias.as_deref(), Some("forme canonique"));
        assert_eq!(n.liens[1].cible, "Autre note");
        assert_eq!(n.liens[1].ancre.as_deref(), Some("Section"));
        let accueil = extraire_liens(r#"<a href="Spé SVT/Chap/Note.md" data-href="Spé SVT/Chap/Note.md">x</a> ![[La mutation (schéma).excalidraw|800]]"#);
        assert!(accueil.iter().any(|l| l.cible == "Spé SVT/Chap/Note" && !l.integre));
        assert!(accueil.iter().any(|l| l.cible == "La mutation (schéma).excalidraw" && l.integre));
    }

    #[test]
    fn legendes_de_schema() {
        let schema = "---\nexcalidraw-plugin: parsed\n---\n# Excalidraw Data\n\n## Text Elements\nSéquence initiale\n(allèle sauvage)\n...cytosine... ^t2\n\nmutation ^t8\n\n%%\n## Drawing\n```compressed-json\nN4Ke\n```\n%%";
        let n = analyser(schema, true);
        assert_eq!(n.sections.len(), 1);
        assert_eq!(n.sections[0].texte, "Séquence initiale (allèle sauvage) ...cytosine...\nmutation");
    }

    #[test]
    fn legendes_de_schema_en_json() {
        let schema = "---\nexcalidraw-plugin: parsed\n---\n# Excalidraw Data\n\n## Drawing\n```json\n{\"type\": \"excalidraw\", \"elements\": [{\"type\": \"rectangle\"}, {\"type\": \"text\", \"text\": \"Prophase I\\n(appariement)\"}, {\"type\": \"text\", \"text\": \"supprimé\", \"isDeleted\": true}]}\n```\n%%";
        let n = analyser(schema, true);
        assert_eq!(n.sections[0].texte, "Prophase I (appariement)");
        assert_eq!(n.sections[0].ligne, 4); // première ligne après le frontmatter
    }

    #[test]
    fn commentaires_obsidian_ignores() {
        let n = analyser("Avant\n%%\ncaché\n%%\nAprès\n", false);
        assert!(!n.sections[0].texte.contains("caché"));
        assert!(n.sections[0].texte.contains("Après"));
    }

    #[test]
    fn texte_brut_pour_la_recherche() {
        let t = texte_brut("> [!info]+ Définition\n> Une **mutation** est un [[Le gène et l'allèle|changement]] <b>x</b>\n");
        assert_eq!(t, "Définition\nUne mutation est un changement x\n");
    }
}
