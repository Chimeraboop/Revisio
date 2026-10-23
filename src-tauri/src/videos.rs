//! Vidéos de révision : diaporama narré.
//! 1. Gemini écrit les diapositives (titre, points clés, schéma éventuel) et leur narration (1 appel) ;
//! 2. chaque narration est enregistrée par la synthèse vocale (1 appel par diapositive, sauvegardé aussitôt) ;
//! 3. l'interface dessine les diapositives et assemble le MP4 (encodeur intégré à WebView2) ;
//!    Révisio range le fichier dans `%APPDATA%\Revisio\videos` (jamais dans le vault).

use std::path::{Path, PathBuf};

use rusqlite::{params, Connection, OptionalExtension};
use serde::{Deserialize, Serialize};
use serde_json::{json, Value};

use crate::audio::{self, Pcm};
use crate::ia::ancrage::{extrait_par_id, Extrait};

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct OptionsVideo {
    pub diapos: u32,
    /// « francais », « allemand » ou « anglais ».
    pub langue: String,
}

impl OptionsVideo {
    pub fn valider(&self) -> Result<(), String> {
        if !(4..=12).contains(&self.diapos) {
            return Err("Une vidéo compte de 4 à 12 diapositives.".into());
        }
        if !["francais", "allemand", "anglais"].contains(&self.langue.as_str()) {
            return Err(format!("Langue inconnue : {}", self.langue));
        }
        Ok(())
    }
}

/// Un schéma du chapitre que l'IA peut placer sur une diapositive (S1, S2…).
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct SchemaPropose {
    pub id: String,
    pub chemin: String,
    pub titre: String,
}

pub fn schemas_du_chapitre(conn: &Connection, matiere: &str, chapitre: Option<&str>) -> rusqlite::Result<Vec<SchemaPropose>> {
    let lignes: Vec<(String, String)> = conn
        .prepare("SELECT chemin, titre FROM notes WHERE genre = 'schema' AND matiere = ?1 AND chapitre IS ?2 ORDER BY titre")?
        .query_map(params![matiere, chapitre], |r| Ok((r.get(0)?, r.get(1)?)))?
        .collect::<rusqlite::Result<_>>()?;
    Ok(lignes
        .into_iter()
        .enumerate()
        .map(|(i, (chemin, titre))| SchemaPropose { id: format!("S{}", i + 1), chemin, titre })
        .collect())
}

// ---------- Le diaporama ----------

#[derive(Debug, Clone, Deserialize)]
pub struct DiapoIa {
    pub titre: String,
    #[serde(default)]
    pub points: Vec<String>,
    #[serde(default)]
    pub schema: String,
    pub narration: String,
    #[serde(default)]
    pub source: String,
}

#[derive(Debug, Clone, Deserialize)]
pub struct ScriptVideo {
    pub titre: String,
    pub diapositives: Vec<DiapoIa>,
}

pub const CONSIGNE_SYSTEME: &str = "Tu prépares de courtes vidéos de révision (diaporama commenté) pour un élève de Première (France), à partir de ses notes de cours.
Règles absolues :
- Utilise UNIQUEMENT les informations des extraits fournis. N'invente rien.
- Pour chaque diapositive, indique dans « source » l'identifiant de l'extrait utilisé (ex. « E3 »).
- Les points des diapositives sont très courts (mots-clés, 12 mots au plus) ; les formules peuvent être en LaTeX entre $…$.
- La narration est lue telle quelle par une voix de synthèse : pas de LaTeX, pas de Markdown, formules et symboles écrits en toutes lettres.";

pub fn consigne(nom_chapitre: &str, o: &OptionsVideo, schemas: &[SchemaPropose]) -> String {
    let langue = match o.langue.as_str() {
        "allemand" => "en allemand (niveau B1, phrases claires)",
        "anglais" => "en anglais (niveau B1, phrases claires)",
        _ => "en français",
    };
    let mut c = format!(
        "Prépare une vidéo de révision sur « {nom_chapitre} », {langue} : exactement {n} diapositives. La première présente \
         le chapitre et son plan, les suivantes expliquent les notions dans un ordre logique, la dernière récapitule \
         l'essentiel. Chaque diapositive : un titre court, 2 à 5 points clés, et une narration de 40 à 90 mots qui \
         explique la diapositive (sans la lire mot pour mot).",
        n = o.diapos
    );
    if schemas.is_empty() {
        c += " « schema » : toujours vide.";
    } else {
        c += &format!(
            " Schémas disponibles (mets son identifiant dans « schema » quand il illustre bien la diapositive, sinon vide ; \
             2 diapositives au plus avec un schéma) :\n{}",
            schemas.iter().map(|s| format!("- {} : {}", s.id, s.titre)).collect::<Vec<_>>().join("\n")
        );
    }
    c
}

pub fn schema_json() -> Value {
    json!({
        "type": "object",
        "properties": {
            "titre": { "type": "string" },
            "diapositives": {
                "type": "array",
                "items": {
                    "type": "object",
                    "properties": {
                        "titre": { "type": "string" },
                        "points": { "type": "array", "items": { "type": "string" } },
                        "schema": { "type": "string", "description": "Identifiant d'un schéma disponible (S1…) ou vide." },
                        "narration": { "type": "string" },
                        "source": { "type": "string" }
                    },
                    "required": ["titre", "points", "schema", "narration", "source"]
                }
            }
        },
        "required": ["titre", "diapositives"]
    })
}

fn mots(t: &str) -> usize {
    t.split_whitespace().count()
}

pub fn valider(s: &ScriptVideo, o: &OptionsVideo, extraits: &[Extrait], schemas: &[SchemaPropose]) -> Result<(), String> {
    let n = s.diapositives.len() as u32;
    if n + 1 < o.diapos || n > o.diapos + 1 {
        return Err(format!("{n} diapositives au lieu de {}", o.diapos));
    }
    for (i, d) in s.diapositives.iter().enumerate() {
        let k = i + 1;
        if d.titre.trim().is_empty() || d.points.is_empty() || d.points.len() > 6 {
            return Err(format!("diapositive {k} : titre et 1 à 6 points attendus"));
        }
        if !(15..=160).contains(&mots(&d.narration)) {
            return Err(format!("diapositive {k} : narration de {} mots (40 à 90 attendus)", mots(&d.narration)));
        }
        if d.narration.contains('$') || d.narration.contains("**") {
            return Err(format!("diapositive {k} : pas de LaTeX ni de Markdown dans la narration"));
        }
        if !d.schema.trim().is_empty() && !schemas.iter().any(|x| x.id.eq_ignore_ascii_case(d.schema.trim())) {
            return Err(format!("diapositive {k} : schéma « {} » inconnu", d.schema));
        }
    }
    let sourcees = s.diapositives.iter().filter(|d| extrait_par_id(extraits, &d.source).is_some()).count();
    if sourcees * 2 < s.diapositives.len() {
        return Err("trop peu de diapositives indiquent leur extrait source".into());
    }
    Ok(())
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct Diapo {
    pub titre: String,
    pub points: Vec<String>,
    /// Chemin du schéma Excalidraw à afficher (ou aucun).
    pub schema: Option<String>,
    pub narration: String,
    pub source: String,
    /// Durée de la narration enregistrée.
    pub duree_ms: Option<u64>,
}

pub fn preparer(s: ScriptVideo, extraits: &[Extrait], schemas: &[SchemaPropose]) -> (String, Vec<Diapo>) {
    let diapos = s
        .diapositives
        .into_iter()
        .map(|d| Diapo {
            titre: d.titre.trim().into(),
            points: d.points.into_iter().map(|p| p.trim().to_string()).filter(|p| !p.is_empty()).collect(),
            schema: schemas.iter().find(|x| x.id.eq_ignore_ascii_case(d.schema.trim())).map(|x| x.chemin.clone()),
            narration: d.narration.trim().into(),
            source: extrait_par_id(extraits, &d.source).map(|e| e.id.clone()).unwrap_or_default(),
            duree_ms: None,
        })
        .collect();
    (s.titre.trim().to_string(), diapos)
}

// ---------- Stockage ----------

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Video {
    pub id: i64,
    pub date: i64,
    pub matiere: String,
    pub chapitre: Option<String>,
    pub titre: String,
    pub options: OptionsVideo,
    pub modele_script: String,
    pub modele_tts: String,
    pub voix: String,
    pub diapos: Vec<Diapo>,
    pub extraits: Vec<Extrait>,
    pub faites: usize,
    pub fichier: Option<String>,
    pub duree_ms: Option<i64>,
}

impl Video {
    pub fn fichier_narration(&self, k: usize) -> String {
        format!("video-{}-{k}.wav", self.id)
    }
}

pub fn dossier_videos(dossier_donnees: &Path) -> Result<PathBuf, String> {
    let d = dossier_donnees.join("videos");
    std::fs::create_dir_all(&d).map_err(|e| format!("Dossier des vidéos : {e}"))?;
    Ok(d)
}

fn lire_json<T: for<'de> Deserialize<'de>>(t: &str, colonne: usize) -> rusqlite::Result<T> {
    serde_json::from_str(t).map_err(|e| rusqlite::Error::FromSqlConversionFailure(colonne, rusqlite::types::Type::Text, Box::new(e)))
}

#[allow(clippy::too_many_arguments)]
pub fn creer(
    conn: &Connection,
    matiere: &str,
    chapitre: Option<&str>,
    titre: &str,
    options: &OptionsVideo,
    modele_script: &str,
    modele_tts: &str,
    voix: &str,
    diapos: &[Diapo],
    extraits: &[Extrait],
    date: i64,
) -> rusqlite::Result<i64> {
    conn.execute(
        "INSERT INTO videos(date, matiere, chapitre, titre, options, modele_script, modele_tts, voix, diapos, extraits)
         VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10)",
        params![
            date,
            matiere,
            chapitre,
            titre,
            serde_json::to_string(options).unwrap_or_default(),
            modele_script,
            modele_tts,
            voix,
            serde_json::to_string(diapos).unwrap_or_default(),
            serde_json::to_string(extraits).unwrap_or_default()
        ],
    )?;
    Ok(conn.last_insert_rowid())
}

const COLONNES: &str =
    "id, date, matiere, chapitre, titre, options, modele_script, modele_tts, voix, diapos, extraits, faites, fichier, duree_ms";

fn lire(r: &rusqlite::Row) -> rusqlite::Result<Video> {
    Ok(Video {
        id: r.get(0)?,
        date: r.get(1)?,
        matiere: r.get(2)?,
        chapitre: r.get(3)?,
        titre: r.get(4)?,
        options: lire_json(&r.get::<_, String>(5)?, 5)?,
        modele_script: r.get(6)?,
        modele_tts: r.get(7)?,
        voix: r.get(8)?,
        diapos: lire_json(&r.get::<_, String>(9)?, 9)?,
        extraits: lire_json(&r.get::<_, String>(10)?, 10)?,
        faites: r.get::<_, i64>(11)? as usize,
        fichier: r.get(12)?,
        duree_ms: r.get(13)?,
    })
}

pub fn lire_video(conn: &Connection, id: i64) -> rusqlite::Result<Option<Video>> {
    conn.query_row(&format!("SELECT {COLONNES} FROM videos WHERE id = ?1"), [id], lire).optional()
}

pub fn lister(conn: &Connection) -> rusqlite::Result<Vec<Video>> {
    conn.prepare(&format!("SELECT {COLONNES} FROM videos ORDER BY date DESC"))?.query_map([], lire)?.collect()
}

/// La narration de la diapositive `k` est enregistrée (avec sa durée).
pub fn narration_enregistree(conn: &Connection, v: &mut Video, k: usize, duree_ms: u64) -> rusqlite::Result<()> {
    if let Some(d) = v.diapos.get_mut(k) {
        d.duree_ms = Some(duree_ms);
    }
    v.faites = k + 1;
    conn.execute(
        "UPDATE videos SET diapos = ?2, faites = ?3 WHERE id = ?1",
        params![v.id, serde_json::to_string(&v.diapos).unwrap_or_default(), v.faites as i64],
    )?;
    Ok(())
}

pub fn definir_fichier(conn: &Connection, id: i64, fichier: &str, duree_ms: i64) -> rusqlite::Result<()> {
    conn.execute("UPDATE videos SET fichier = ?2, duree_ms = ?3 WHERE id = ?1", params![id, fichier, duree_ms])?;
    Ok(())
}

/// Silence avant et après chaque narration : une diapositive dure `PAUSE_AVANT_MS + durée + PAUSE_APRES_MS`
/// (mêmes valeurs dans `src/videos/montage.ts`).
pub const PAUSE_AVANT_MS: u64 = 500;
pub const PAUSE_APRES_MS: u64 = 700;

/// Toutes les narrations bout à bout, séparées par des silences (une diapositive = un segment).
pub fn narration_complete(dossier_audio: &Path, v: &Video) -> Result<Vec<u8>, String> {
    let mut total: Option<Pcm> = None;
    for (k, d) in v.diapos.iter().enumerate() {
        let octets =
            std::fs::read(dossier_audio.join(v.fichier_narration(k))).map_err(|e| format!("Narration {} : {e}", k + 1))?;
        let pcm = audio::lire_wav(&octets)?;
        let par_echantillon = (pcm.canaux as u64 * (pcm.bits as u64 / 8)).max(1);
        let octets_pour = |ms: u64| (ms * pcm.frequence as u64 / 1000 * par_echantillon) as usize;
        // La narration est coupée à sa durée en millisecondes entières (moins d'1 ms) : le segment dure exactement
        // ce que l'interface calcule.
        let duree = d.duree_ms.unwrap_or_else(|| pcm.duree_ms());
        let mut donnees = vec![0u8; octets_pour(PAUSE_AVANT_MS)];
        donnees.extend_from_slice(&pcm.donnees[..octets_pour(duree).min(pcm.donnees.len())]);
        donnees.resize(octets_pour(PAUSE_AVANT_MS + duree + PAUSE_APRES_MS), 0);
        let segment = Pcm { donnees, ..pcm };
        match total.as_mut() {
            Some(t) => t.ajouter(&segment)?,
            None => total = Some(segment),
        }
    }
    Ok(total.ok_or("Aucune narration enregistrée.")?.en_wav())
}

/// Supprime la vidéo, ses narrations et son fichier.
pub fn supprimer(conn: &Connection, dossier_audio: &Path, dossier_videos: &Path, id: i64) -> Result<(), String> {
    if let Some(v) = lire_video(conn, id).map_err(|e| e.to_string())? {
        for k in 0..v.diapos.len() {
            let _ = std::fs::remove_file(dossier_audio.join(v.fichier_narration(k)));
        }
        if let Some(f) = &v.fichier {
            let _ = std::fs::remove_file(dossier_videos.join(f));
        }
    }
    conn.execute("DELETE FROM videos WHERE id = ?1", [id]).map_err(|e| e.to_string())?;
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::db;

    fn options() -> OptionsVideo {
        OptionsVideo { diapos: 4, langue: "francais".into() }
    }

    fn extraits() -> Vec<Extrait> {
        vec![Extrait {
            id: "E1".into(),
            note_chemin: "n.md".into(),
            titre_note: "Note".into(),
            section: String::new(),
            texte: "t".into(),
            markdown: "t".into(),
        }]
    }

    fn schemas() -> Vec<SchemaPropose> {
        vec![SchemaPropose { id: "S1".into(), chemin: "SVT/méiose (schéma).excalidraw.md".into(), titre: "Méiose (schéma)".into() }]
    }

    fn script(n: usize) -> ScriptVideo {
        ScriptVideo {
            titre: " La méiose ".into(),
            diapositives: (0..n)
                .map(|i| DiapoIa {
                    titre: format!("Diapo {i}"),
                    points: vec!["$2n$ → $n$".into(), "4 gamètes".into()],
                    schema: if i == 1 { "S1".into() } else { String::new() },
                    narration: vec!["mot"; 50].join(" "),
                    source: "E1".into(),
                })
                .collect(),
        }
    }

    #[test]
    fn validation_et_preparation() {
        let (o, e, s) = (options(), extraits(), schemas());
        assert!(o.valider().is_ok());
        assert!(OptionsVideo { diapos: 20, ..options() }.valider().is_err());
        assert!(valider(&script(4), &o, &e, &s).is_ok());
        assert!(valider(&script(1), &o, &e, &s).is_err(), "trop peu de diapositives");
        let mut latex = script(4);
        latex.diapositives[2].narration = format!("on a $x^2$ {}", vec!["mot"; 30].join(" "));
        assert!(valider(&latex, &o, &e, &s).is_err());
        let mut inconnu = script(4);
        inconnu.diapositives[0].schema = "S9".into();
        assert!(valider(&inconnu, &o, &e, &s).is_err());

        let (titre, diapos) = preparer(script(4), &e, &s);
        assert_eq!(titre, "La méiose");
        assert_eq!(diapos[1].schema.as_deref(), Some("SVT/méiose (schéma).excalidraw.md"));
        assert_eq!(diapos[0].schema, None);
        assert_eq!(diapos[0].source, "E1");
    }

    #[test]
    fn stockage() {
        let conn = db::ouvrir_en_memoire().unwrap();
        let (titre, diapos) = preparer(script(4), &extraits(), &schemas());
        let id = creer(&conn, "SVT", Some("Méiose"), &titre, &options(), "m", "tts", "Puck", &diapos, &extraits(), 1).unwrap();
        let mut v = lire_video(&conn, id).unwrap().unwrap();
        assert_eq!((v.faites, v.diapos.len(), v.fichier.as_deref()), (0, 4, None));
        narration_enregistree(&conn, &mut v, 0, 12_345).unwrap();
        let relu = lire_video(&conn, id).unwrap().unwrap();
        assert_eq!((relu.faites, relu.diapos[0].duree_ms), (1, Some(12_345)));
        definir_fichier(&conn, id, "video-1.mp4", 60_000).unwrap();
        assert_eq!(lister(&conn).unwrap()[0].fichier.as_deref(), Some("video-1.mp4"));
        let dossier = std::env::temp_dir();
        supprimer(&conn, &dossier, &dossier, id).unwrap();
        assert!(lire_video(&conn, id).unwrap().is_none());
    }

    #[test]
    fn narration_avec_pauses() {
        let conn = db::ouvrir_en_memoire().unwrap();
        let dossier = std::env::temp_dir().join(format!("revisio-test-video-{}", std::process::id()));
        std::fs::create_dir_all(&dossier).unwrap();
        let (titre, diapos) = preparer(script(2), &extraits(), &schemas());
        let opts = OptionsVideo { diapos: 2, ..options() };
        let id = creer(&conn, "SVT", None, &titre, &opts, "m", "tts", "Charon", &diapos, &extraits(), 1).unwrap();
        let mut v = lire_video(&conn, id).unwrap().unwrap();
        assert!(narration_complete(&dossier, &v).is_err(), "rien n'est encore enregistré");
        // Narrations de 1 s et de 0,5 s (+ un demi-échantillon de trop, coupé).
        for (k, n) in [48_000usize, 24_002].into_iter().enumerate() {
            let pcm = Pcm { frequence: 24_000, canaux: 1, bits: 16, donnees: vec![9; n] };
            std::fs::write(dossier.join(v.fichier_narration(k)), pcm.en_wav()).unwrap();
            narration_enregistree(&conn, &mut v, k, pcm.duree_ms()).unwrap();
        }
        let total = audio::lire_wav(&narration_complete(&dossier, &v).unwrap()).unwrap();
        let attendu = (PAUSE_AVANT_MS + PAUSE_APRES_MS) * 2 + 1000 + 500;
        assert_eq!(total.duree_ms(), attendu);
        // Le premier segment commence par un silence, puis la voix.
        let debut_voix = (PAUSE_AVANT_MS * 48) as usize;
        assert!(total.donnees[..debut_voix].iter().all(|&o| o == 0));
        assert_eq!(total.donnees[debut_voix], 9);
        supprimer(&conn, &dossier, &dossier, id).unwrap();
        assert!(std::fs::read_dir(&dossier).unwrap().next().is_none(), "narrations supprimées");
        std::fs::remove_dir(&dossier).unwrap();
    }
}
