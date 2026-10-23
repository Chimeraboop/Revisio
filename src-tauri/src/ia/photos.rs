//! Photos de cours → notes atomiques, rédigées comme celles du vault (Gemini avec vision).
//!
//! Deux appels :
//! 1. repérage : les photos sont-elles lisibles ? quelle matière, quel chapitre, quel dossier
//!    du vault, quelles notions (une notion = une note), lesquelles existent déjà ;
//! 2. rédaction : les nouvelles notes, et pour les notes existantes concernées, une version
//!    complétée (qui ne sera écrite qu'après accord de l'élève).

use std::collections::{BTreeMap, HashSet};
use std::io::Cursor;
use std::path::{Component, Path};

use base64::Engine;
use rusqlite::Connection;
use serde::Deserialize;
use serde_json::{json, Value};

use crate::ecriture::nom_de_fichier;
use crate::vault::dossier_ignore;

/// Formats de photo acceptés.
pub const EXTENSIONS: &[&str] = &["jpg", "jpeg", "png", "webp", "heic", "heif"];
/// Photos au plus par lot (un cours).
pub const PHOTOS_PAR_LOT: usize = 10;
/// Côté le plus long après réduction (assez pour lire une écriture manuscrite).
const COTE_MAX: u32 = 2000;

pub const CONSIGNE_SYSTEME: &str = "Tu aides un lycéen à transformer les photos de ses cours en notes Obsidian. \
Tu transcris fidèlement ce qui est sur les photos : tu n'inventes rien, tu n'ajoutes aucune connaissance extérieure. \
Si un passage est illisible, écris [illisible] plutôt que de deviner. Tu écris en français (ou dans la langue du cours \
pour les langues étrangères), sans emoji.";

/// Vrai si ce fichier a l'extension d'une photo acceptée.
pub fn est_une_photo(chemin: &Path) -> bool {
    chemin
        .extension()
        .map(|e| EXTENSIONS.contains(&e.to_string_lossy().to_lowercase().as_str()))
        .unwrap_or(false)
}

/// Photo prête à envoyer : réduite à 2000 px, tournée dans le bon sens (EXIF), en JPEG.
/// Les photos HEIC (iPhone) sont envoyées telles quelles : Gemini sait les lire.
pub fn preparer_photo(chemin: &Path) -> Result<(String, String), String> {
    let nom = chemin.file_name().map(|n| n.to_string_lossy().into_owned()).unwrap_or_default();
    let octets = std::fs::read(chemin).map_err(|e| format!("Lecture de la photo {nom} : {e}"))?;
    let b64 = |o: &[u8]| base64::engine::general_purpose::STANDARD.encode(o);
    let ext = chemin.extension().map(|e| e.to_string_lossy().to_lowercase()).unwrap_or_default();
    if ext == "heic" || ext == "heif" {
        if octets.len() > 7_000_000 {
            return Err(format!("Photo {nom} trop lourde (HEIC de plus de 7 Mo)."));
        }
        return Ok((format!("image/{ext}"), b64(&octets)));
    }
    let illisible = |e: image::ImageError| format!("Photo {nom} illisible : {e}");
    let lecteur = image::ImageReader::new(Cursor::new(&octets)).with_guessed_format().map_err(|e| e.to_string())?;
    let mut decodeur = lecteur.into_decoder().map_err(illisible)?;
    let orientation = image::ImageDecoder::orientation(&mut decodeur).unwrap_or(image::metadata::Orientation::NoTransforms);
    let mut img = image::DynamicImage::from_decoder(decodeur).map_err(illisible)?;
    img.apply_orientation(orientation);
    if img.width().max(img.height()) > COTE_MAX {
        img = img.resize(COTE_MAX, COTE_MAX, image::imageops::FilterType::Triangle);
    }
    let mut jpeg = Vec::new();
    let encodeur = image::codecs::jpeg::JpegEncoder::new_with_quality(&mut jpeg, 85);
    img.to_rgb8().write_with_encoder(encodeur).map_err(illisible)?;
    Ok(("image/jpeg".into(), b64(&jpeg)))
}

/// Dossiers du vault et noms des notes qu'ils contiennent (d'après l'index).
pub fn arbre(conn: &Connection) -> rusqlite::Result<BTreeMap<String, Vec<String>>> {
    let mut req = conn.prepare("SELECT chemin FROM notes WHERE genre != 'schema' ORDER BY chemin")?;
    let chemins: Vec<String> = req.query_map([], |r| r.get(0))?.collect::<rusqlite::Result<_>>()?;
    let mut dossiers: BTreeMap<String, Vec<String>> = BTreeMap::new();
    for c in chemins {
        let (dossier, nom) = c.rsplit_once('/').unwrap_or(("", c.as_str()));
        dossiers.entry(dossier.to_string()).or_default().push(nom.trim_end_matches(".md").to_string());
        // Les dossiers parents existent aussi (une matière sans note directe, par exemple).
        let mut parent = dossier;
        while let Some((p, _)) = parent.rsplit_once('/') {
            dossiers.entry(p.to_string()).or_default();
            parent = p;
        }
    }
    Ok(dossiers)
}

/// L'arbre en texte pour l'IA (raccourci si le vault est très grand).
pub fn arbre_en_texte(arbre: &BTreeMap<String, Vec<String>>) -> String {
    let mut lignes = Vec::new();
    let par_dossier = if arbre.values().map(Vec::len).sum::<usize>() > 600 { 12 } else { 60 };
    for (dossier, notes) in arbre {
        let affiche = if dossier.is_empty() { "(racine du vault)" } else { dossier };
        let mut liste: Vec<&str> = notes.iter().take(par_dossier).map(String::as_str).collect();
        if notes.len() > par_dossier {
            liste.push("…");
        }
        lignes.push(format!("- {affiche} : {}", liste.join(" ; ")));
    }
    lignes.join("\n")
}

// ---------- 1. Repérage ----------

#[derive(Debug, Clone, Deserialize)]
pub struct Reperage {
    pub lisible: bool,
    #[serde(default)]
    pub probleme: String,
    #[serde(default)]
    pub matiere: String,
    #[serde(default)]
    pub chapitre: String,
    #[serde(default)]
    pub dossier: String,
    #[serde(default)]
    pub notions: Vec<Notion>,
}

#[derive(Debug, Clone, Deserialize)]
pub struct Notion {
    pub titre: String,
    #[serde(default)]
    pub note_existante: String,
}

pub fn consigne_reperage(nombre: usize, arbre: &BTreeMap<String, Vec<String>>) -> String {
    format!(
        "Voici {nombre} photo(s) d'un même cours. Voici l'organisation du vault Obsidian de l'élève \
(dossier : noms des notes, sans « .md ») :\n{}\n\n\
Réponds en JSON :\n\
- lisible : faux si les photos sont illisibles ou ne contiennent pas de cours (par exemple seulement des exercices \
sans notion expliquée) ; explique alors pourquoi dans probleme.\n\
- matiere, chapitre : tels qu'ils apparaissent dans le vault si le cours y est déjà.\n\
- dossier : chemin du dossier où ranger les notes. Reprends le dossier existant du chapitre s'il existe. Sinon, \
crée un dossier de chapitre DANS le dossier existant de la matière, nommé comme les autres chapitres de cette \
matière. N'invente jamais une nouvelle arborescence.\n\
- notions : les notions du cours, une notion = une future note courte (atomique), avec un titre court dans le style \
des titres du vault. Si une notion est déjà traitée par une note existante, mets son chemin complet \
(« dossier/nom.md ») dans note_existante, sinon une chaîne vide. Les exercices sont ignorés, sauf la notion de \
cours qu'ils expliquent. Une liste de vocabulaire est une notion.",
        arbre_en_texte(arbre)
    )
}

pub fn schema_reperage() -> Value {
    json!({
        "type": "object",
        "properties": {
            "lisible": {"type": "boolean"},
            "probleme": {"type": "string"},
            "matiere": {"type": "string"},
            "chapitre": {"type": "string"},
            "dossier": {"type": "string"},
            "notions": {
                "type": "array",
                "maxItems": 15,
                "items": {
                    "type": "object",
                    "properties": {
                        "titre": {"type": "string"},
                        "note_existante": {"type": "string"}
                    },
                    "required": ["titre", "note_existante"]
                }
            }
        },
        "required": ["lisible", "probleme", "matiere", "chapitre", "dossier", "notions"]
    })
}

/// Dossier relatif propre (pas de « .. », pas de dossier réservé) ?
pub fn dossier_valide(dossier: &str) -> bool {
    let p = Path::new(dossier);
    !dossier.is_empty()
        && !dossier.contains('\\')
        && p.components().all(|c| matches!(c, Component::Normal(_)))
        && !dossier.split('/').any(|m| dossier_ignore(m) || nom_de_fichier(m) != m)
}

/// Vérifie le repérage : dossier existant, ou nouveau dossier dans un dossier existant ;
/// notes existantes citées réellement présentes.
pub fn valider_reperage(r: &Reperage, arbre: &BTreeMap<String, Vec<String>>) -> Result<(), String> {
    if !r.lisible {
        return Ok(());
    }
    if r.notions.is_empty() {
        return Err("aucune notion trouvée".into());
    }
    let dossier = r.dossier.trim_matches('/');
    if !dossier_valide(dossier) {
        return Err(format!("dossier invalide : « {} »", r.dossier));
    }
    if !arbre.contains_key(dossier) {
        let parent = dossier.rsplit_once('/').map(|(p, _)| p).unwrap_or("");
        if parent.is_empty() || !arbre.contains_key(parent) {
            return Err(format!("le dossier « {dossier} » doit être placé dans un dossier existant du vault"));
        }
    }
    let notes: HashSet<String> = arbre
        .iter()
        .flat_map(|(d, noms)| noms.iter().map(move |n| if d.is_empty() { format!("{n}.md") } else { format!("{d}/{n}.md") }))
        .collect();
    for n in &r.notions {
        if !n.note_existante.is_empty() && !notes.contains(&n.note_existante) {
            return Err(format!("note existante inconnue : « {} »", n.note_existante));
        }
    }
    Ok(())
}

// ---------- 2. Rédaction ----------

#[derive(Debug, Clone, Deserialize)]
pub struct Redaction {
    pub notes: Vec<NoteRedigee>,
    #[serde(default)]
    pub modifications: Vec<ModificationRedigee>,
}

#[derive(Debug, Clone, Deserialize)]
pub struct NoteRedigee {
    pub titre: String,
    #[serde(default)]
    pub tags: Vec<String>,
    pub contenu: String,
}

#[derive(Debug, Clone, Deserialize)]
pub struct ModificationRedigee {
    pub chemin: String,
    pub contenu: String,
    pub raison: String,
}

/// Ce que l'IA reçoit pour rédiger.
pub struct ContexteRedaction<'a> {
    pub reperage: &'a Reperage,
    pub modele: Option<String>,
    /// (nom, contenu) des notes du dossier choisi (toutes, pour éviter les doublons), ou à défaut
    /// de notes de la même matière, pour imiter le style.
    pub exemples: Vec<(String, String)>,
    /// Vrai si `exemples` contient les notes du dossier choisi (et pas seulement de la matière).
    pub exemples_du_dossier: bool,
    /// (chemin, contenu) des notes existantes qui traitent déjà certaines notions.
    pub existantes: Vec<(String, String)>,
    /// Noms des notes du dossier choisi (pour les [[liens]]).
    pub voisines: Vec<String>,
}

pub fn consigne_redaction(c: &ContexteRedaction) -> String {
    let r = c.reperage;
    let notions: Vec<String> = r
        .notions
        .iter()
        .map(|n| {
            if n.note_existante.is_empty() {
                format!("- {} (nouvelle note)", n.titre)
            } else {
                format!("- {} (déjà traitée dans {})", n.titre, n.note_existante)
            }
        })
        .collect();
    let mut texte = format!(
        "Matière : {}. Chapitre : {}. Dossier : {}.\nNotions repérées sur les photos :\n{}\n\n\
Rédige les notes à partir des photos, en suivant ces règles :\n\
- Pas de doublon : une notion déjà traitée par une note du vault (même avec un autre titre) ne devient JAMAIS une \
nouvelle note. Si les photos n'apportent rien de nouveau, renvoie des listes vides : c'est une bonne réponse.\n\
- notes : une note par notion NOUVELLE, courte et atomique, fidèle aux photos. titre = titre de la note (sans \
« .md »). tags : 2 à 4 mots-clés en minuscules, sans « # », dans le style des exemples. contenu : le corps de la \
note SANS frontmatter (Révisio l'ajoute).\n\
- Mise en forme Obsidian : callouts comme boîte à outils, seulement ceux qui servent (> [!info]+ Définition, \
> [!note]+ Contexte, > [!abstract]+ Propriété, > [!question]+ Démonstration, > [!example]+ Exemple, \
> [!quote]+ Citation, > [!success]+ Application, > [!warning]+ Points de vigilance), tableaux, listes numérotées \
pour des étapes. Formules en LaTeX entre $…$. Mots-clés importants en **gras**. Vocabulaire : une liste \
« * mot = traduction » par partie.\n\
- Termine chaque note par un callout > [!tip]+ Notes liées contenant des [[liens]] vers les notes voisines ou vers \
les autres notes que tu crées (titres exacts).\n\
- modifications : pour une notion déjà traitée dans une note existante, seulement si les photos apportent une \
information qui y manque, renvoie le contenu COMPLET de cette note complétée (frontmatter compris), sans rien \
supprimer ni reformuler de l'existant : uniquement des ajouts. raison : en une phrase, ce que tu ajoutes. Tu peux \
aussi ajouter dans « Notes liées » d'une note existante un lien vers une nouvelle note. Sinon, liste vide.",
        r.matiere,
        r.chapitre,
        r.dossier,
        notions.join("\n")
    );
    if !c.voisines.is_empty() {
        texte.push_str(&format!("\n\nNotes déjà présentes dans ce dossier : {}.", c.voisines.join(" ; ")));
    }
    if let Some(modele) = &c.modele {
        texte.push_str(&format!("\n\nModèle de note de l'élève (structure à suivre) :\n<<<\n{modele}\n>>>"));
    }
    let quoi = if c.exemples_du_dossier {
        "Note déjà présente dans ce dossier (imite son style, ne la recopie pas)"
    } else {
        "Exemple de note de la même matière (imite son style)"
    };
    for (nom, contenu) in &c.exemples {
        texte.push_str(&format!("\n\n{quoi}, « {nom} » :\n<<<\n{contenu}\n>>>"));
    }
    for (chemin, contenu) in &c.existantes {
        texte.push_str(&format!("\n\nNote existante {chemin} :\n<<<\n{contenu}\n>>>"));
    }
    texte
}

pub fn schema_redaction() -> Value {
    json!({
        "type": "object",
        "properties": {
            "notes": {
                "type": "array",
                "maxItems": 15,
                "items": {
                    "type": "object",
                    "properties": {
                        "titre": {"type": "string"},
                        "tags": {"type": "array", "items": {"type": "string"}, "maxItems": 6},
                        "contenu": {"type": "string"}
                    },
                    "required": ["titre", "tags", "contenu"]
                }
            },
            "modifications": {
                "type": "array",
                "maxItems": 8,
                "items": {
                    "type": "object",
                    "properties": {
                        "chemin": {"type": "string"},
                        "contenu": {"type": "string"},
                        "raison": {"type": "string"}
                    },
                    "required": ["chemin", "contenu", "raison"]
                }
            }
        },
        "required": ["notes", "modifications"]
    })
}

/// Vrai si `apres` garde tout ce que contenait `avant` (l'IA n'a fait qu'ajouter).
pub fn conserve_le_contenu(avant: &str, apres: &str) -> bool {
    let lignes_apres: HashSet<&str> = apres.lines().map(str::trim).filter(|l| !l.is_empty()).collect();
    let lignes_avant: Vec<&str> = avant.lines().map(str::trim).filter(|l| !l.is_empty()).collect();
    if lignes_avant.is_empty() {
        return true;
    }
    let gardees = lignes_avant.iter().filter(|l| lignes_apres.contains(*l)).count();
    gardees * 100 >= lignes_avant.len() * 98 && apres.len() > avant.len()
}

pub fn valider_redaction(r: &Redaction, existantes: &[(String, String)]) -> Result<(), String> {
    // Listes vides : les photos n'apportent rien de nouveau, c'est une réponse valable.
    let mut noms = HashSet::new();
    for n in &r.notes {
        let nom = nom_de_fichier(&n.titre).to_lowercase();
        if nom.is_empty() || !noms.insert(nom) {
            return Err(format!("titre vide ou en double : « {} »", n.titre));
        }
        let contenu = retablir_sauts_de_ligne(&n.contenu);
        if contenu.trim().chars().count() < 30 {
            return Err(format!("note trop courte : « {} »", n.titre));
        }
        if contenu.contains("\\n\\n") {
            return Err(format!("retours à la ligne écrits « \\n » dans « {} »", n.titre));
        }
    }
    for m in &r.modifications {
        let Some((_, avant)) = existantes.iter().find(|(c, _)| *c == m.chemin) else {
            return Err(format!("modification d'une note non fournie : {}", m.chemin));
        };
        if !conserve_le_contenu(avant, &retablir_sauts_de_ligne(&m.contenu)) {
            return Err(format!("la modification de {} supprime ou reformule du contenu existant", m.chemin));
        }
    }
    Ok(())
}

/// Il arrive que l'IA écrive les retours à la ligne « \n » en toutes lettres (texte échappé deux fois).
/// Si le texte n'a presque pas de vrais retours à la ligne, on remplace les « \n » qui ne commencent pas une
/// commande LaTeX (\nu, \neq, \nabla…).
pub fn retablir_sauts_de_ligne(texte: &str) -> String {
    let vrais = texte.matches('\n').count();
    let ecrits = texte.matches("\\n").count();
    if ecrits == 0 || vrais * 2 >= ecrits {
        return texte.to_string();
    }
    let mut resultat = String::with_capacity(texte.len());
    let mut reste = texte;
    while let Some(i) = reste.find("\\n") {
        resultat.push_str(&reste[..i]);
        let apres = &reste[i + 2..];
        if apres.chars().next().is_some_and(|c| c.is_ascii_alphabetic()) {
            resultat.push_str("\\n");
        } else {
            resultat.push('\n');
        }
        reste = apres;
    }
    resultat.push_str(reste);
    resultat
}

/// Note complète : frontmatter (tags, date de création) puis le corps rédigé.
pub fn note_complete(note: &NoteRedigee, cree_le: &str) -> String {
    let mut corps = retablir_sauts_de_ligne(note.contenu.trim()).trim().to_string();
    // Frontmatter écrit malgré la consigne : on le retire, Révisio met le sien.
    if corps.starts_with("---") {
        if let Some(fin) = corps[3..].find("\n---") {
            corps = corps[3 + fin + 4..].trim_start().to_string();
        }
    }
    let tags: Vec<String> = note
        .tags
        .iter()
        .map(|t| t.trim().trim_start_matches('#').to_lowercase().replace(' ', "-"))
        .filter(|t| !t.is_empty())
        .collect();
    let mut texte = String::from("---\n");
    if !tags.is_empty() {
        texte.push_str("tags:\n");
        for t in tags {
            texte.push_str(&format!("  - {t}\n"));
        }
    }
    texte.push_str(&format!("created: {cree_le}\n---\n\n{corps}\n"));
    texte
}

#[cfg(test)]
mod tests {
    use super::*;

    fn arbre_exemple() -> BTreeMap<String, Vec<String>> {
        let mut a = BTreeMap::new();
        a.insert("Cours".into(), vec![]);
        a.insert("Cours/Spé SVT".into(), vec!["Accueil".into()]);
        a.insert("Cours/Spé SVT/Chapitre 3 - La méiose".into(), vec!["La méiose".into()]);
        a
    }

    fn reperage(dossier: &str, existante: &str) -> Reperage {
        Reperage {
            lisible: true,
            probleme: String::new(),
            matiere: "Spé SVT".into(),
            chapitre: "Chapitre 4".into(),
            dossier: dossier.into(),
            notions: vec![Notion { titre: "Les mutations".into(), note_existante: existante.into() }],
        }
    }

    #[test]
    fn dossiers_acceptes_ou_refuses() {
        let a = arbre_exemple();
        assert!(valider_reperage(&reperage("Cours/Spé SVT/Chapitre 3 - La méiose", ""), &a).is_ok());
        // Nouveau chapitre dans la matière existante : accepté.
        assert!(valider_reperage(&reperage("Cours/Spé SVT/Chapitre 4 - Mutations", ""), &a).is_ok());
        // Nouvelle arborescence inventée, chemin qui sort, dossier réservé : refusés.
        assert!(valider_reperage(&reperage("Biologie/Chapitre 4", ""), &a).is_err());
        assert!(valider_reperage(&reperage("../Ailleurs", ""), &a).is_err());
        assert!(valider_reperage(&reperage("Cours/.obsidian", ""), &a).is_err());
        assert!(valider_reperage(&reperage("Cours/Spé SVT/Chap: 4", ""), &a).is_err());
        // Note existante citée : doit exister.
        assert!(valider_reperage(&reperage("Cours/Spé SVT/Chapitre 3 - La méiose", "Cours/Spé SVT/Chapitre 3 - La méiose/La méiose.md"), &a).is_ok());
        assert!(valider_reperage(&reperage("Cours/Spé SVT/Chapitre 3 - La méiose", "Cours/Inventée.md"), &a).is_err());
    }

    #[test]
    fn une_modification_ne_doit_qu_ajouter() {
        let avant = "---\ntags:\n  - svt\n---\n\n> [!info]+ Définition\n> La méiose produit 4 cellules.\n";
        let ajout = format!("{avant}\n> [!example]+ Exemple\n> Chez l'humain : 23 chromosomes.\n");
        assert!(conserve_le_contenu(avant, &ajout));
        let reformule = ajout.replace("produit 4 cellules", "donne quatre cellules");
        assert!(!conserve_le_contenu(avant, &reformule));
        assert!(!conserve_le_contenu(avant, avant), "aucun ajout");
    }

    #[test]
    fn frontmatter_ajoute_par_revisio() {
        let note = NoteRedigee {
            titre: "Les mutations".into(),
            tags: vec!["#SVT".into(), "mutation génétique".into()],
            contenu: "---\ntags: [x]\n---\n> [!info]+ Définition\n> Une **mutation** est…".into(),
        };
        assert_eq!(
            note_complete(&note, "2026-10-23T18:00"),
            "---\ntags:\n  - svt\n  - mutation-génétique\ncreated: 2026-10-23T18:00\n---\n\n> [!info]+ Définition\n> Une **mutation** est…\n"
        );
    }

    #[test]
    fn retours_a_la_ligne_ecrits_en_toutes_lettres() {
        let echappe = r"> [!info]+ Définition\n> Le rayon $\nu$ et $a \neq b$\n\n> [!tip]+ Notes liées";
        assert_eq!(
            retablir_sauts_de_ligne(echappe),
            "> [!info]+ Définition\n> Le rayon $\\nu$ et $a \\neq b$\n\n> [!tip]+ Notes liées"
        );
        // Texte normal (avec de vrais retours à la ligne et du LaTeX) : inchangé.
        let normal = "Ligne $\\nabla$\nLigne 2\nLigne 3";
        assert_eq!(retablir_sauts_de_ligne(normal), normal);
    }

    #[test]
    fn rien_de_nouveau_est_une_reponse_valable() {
        assert!(valider_redaction(&Redaction { notes: vec![], modifications: vec![] }, &[]).is_ok());
    }

    #[test]
    fn redaction_validee() {
        let existantes = vec![("a/La méiose.md".to_string(), "Ligne 1\nLigne 2".to_string())];
        let note = |t: &str| NoteRedigee { titre: t.into(), tags: vec![], contenu: "x".repeat(40) };
        let ok = Redaction { notes: vec![note("A"), note("B")], modifications: vec![] };
        assert!(valider_redaction(&ok, &existantes).is_ok());
        let doublon = Redaction { notes: vec![note("A"), note("a")], modifications: vec![] };
        assert!(valider_redaction(&doublon, &existantes).is_err());
        let modif = |c: &str, contenu: &str| ModificationRedigee { chemin: c.into(), contenu: contenu.into(), raison: "r".into() };
        let bonne = Redaction { notes: vec![], modifications: vec![modif("a/La méiose.md", "Ligne 1\nLigne 2\nLigne 3")] };
        assert!(valider_redaction(&bonne, &existantes).is_ok());
        let inconnue = Redaction { notes: vec![], modifications: vec![modif("b.md", "x")] };
        assert!(valider_redaction(&inconnue, &existantes).is_err());
        let destructrice = Redaction { notes: vec![], modifications: vec![modif("a/La méiose.md", "Ligne 1\nAutre chose longue")] };
        assert!(valider_redaction(&destructrice, &existantes).is_err());
    }

    #[test]
    fn photo_reduite_et_en_jpeg() {
        let dossier = std::env::temp_dir().join(format!("revisio-photo-{}", std::process::id()));
        std::fs::create_dir_all(&dossier).unwrap();
        let chemin = dossier.join("cours.png");
        image::RgbImage::from_pixel(3000, 1000, image::Rgb([200, 200, 200])).save(&chemin).unwrap();
        let (mime, b64) = preparer_photo(&chemin).unwrap();
        assert_eq!(mime, "image/jpeg");
        let octets = base64::engine::general_purpose::STANDARD.decode(b64).unwrap();
        let img = image::load_from_memory(&octets).unwrap();
        assert_eq!((img.width(), img.height()), (2000, 667));
        assert!(est_une_photo(&chemin) && est_une_photo(Path::new("IMG_1.HEIC")) && !est_une_photo(Path::new("a.pdf")));
        std::fs::remove_dir_all(&dossier).unwrap();
    }
}
