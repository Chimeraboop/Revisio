//! Podcasts de révision à deux voix (façon NotebookLM) :
//! 1. Gemini écrit le dialogue à partir des extraits des notes (1 appel) ;
//! 2. la voix est enregistrée par morceaux d'environ 3 minutes (1 appel de synthèse vocale par morceau),
//!    chaque morceau étant sauvegardé aussitôt : si le quota coupe, on reprend plus tard ;
//! 3. les morceaux sont assemblés en un fichier, puis l'interface le compresse (Opus) pour l'écoute hors ligne.
//!
//! Tout est rangé dans `%APPDATA%\Revisio\audio` (jamais dans le vault).

use std::path::{Path, PathBuf};

use rusqlite::{params, Connection, OptionalExtension};
use serde::{Deserialize, Serialize};
use serde_json::{json, Value};

use crate::audio::{self, Pcm};
use crate::ia::ancrage::{extrait_par_id, Extrait};

/// Débit de parole visé (mots par minute) et taille d'un morceau enregistré en un appel.
pub const MOTS_PAR_MINUTE: usize = 150;
pub const MOTS_PAR_MORCEAU: usize = 450;

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct OptionsPodcast {
    pub duree_minutes: u32,
    /// « detendu », « scolaire » ou « dynamique ».
    pub ton: String,
    /// « essentiel » ou « approfondi ».
    pub detail: String,
    /// « francais », « allemand » ou « anglais ».
    pub langue: String,
}

impl OptionsPodcast {
    pub fn valider(&self) -> Result<(), String> {
        if !(3..=20).contains(&self.duree_minutes) {
            return Err("La durée d'un podcast doit être entre 3 et 20 minutes.".into());
        }
        if !["detendu", "scolaire", "dynamique"].contains(&self.ton.as_str()) {
            return Err(format!("Ton inconnu : {}", self.ton));
        }
        if !["essentiel", "approfondi"].contains(&self.detail.as_str()) {
            return Err(format!("Niveau de détail inconnu : {}", self.detail));
        }
        if !["francais", "allemand", "anglais"].contains(&self.langue.as_str()) {
            return Err(format!("Langue inconnue : {}", self.langue));
        }
        Ok(())
    }

    pub fn mots_cibles(&self) -> usize {
        self.duree_minutes as usize * MOTS_PAR_MINUTE
    }

    /// Prénoms des deux personnes du podcast (A pose les questions, B explique).
    pub fn prenoms(&self) -> (&'static str, &'static str) {
        match self.langue.as_str() {
            "allemand" => ("Anna", "Jonas"),
            "anglais" => ("Emma", "Jack"),
            _ => ("Camille", "Théo"),
        }
    }
}

/// Appels nécessaires : 1 pour le script, puis 1 par morceau de voix.
pub fn appels_estimes(duree_minutes: u32) -> (usize, usize) {
    (1, (duree_minutes as usize * MOTS_PAR_MINUTE).div_ceil(MOTS_PAR_MORCEAU).max(1))
}

// ---------- Le script ----------

#[derive(Debug, Clone, Deserialize)]
pub struct RepliqueIa {
    pub voix: String,
    pub texte: String,
    #[serde(default)]
    pub style: String,
    #[serde(default)]
    pub source: String,
}

#[derive(Debug, Clone, Deserialize)]
pub struct ScriptIa {
    pub titre: String,
    pub repliques: Vec<RepliqueIa>,
}

pub const CONSIGNE_SYSTEME: &str = "Tu écris des podcasts de révision pour un élève de Première (France), à partir de ses notes de cours.
Règles absolues :
- Utilise UNIQUEMENT les informations des extraits fournis. N'ajoute aucune connaissance extérieure, aucun chiffre ni exemple inventé.
- Pour chaque réplique qui transmet une information du cours, indique dans « source » l'identifiant de l'extrait (ex. « E3 »).
- Le texte de chaque réplique est prononcé tel quel par une voix de synthèse : pas de didascalies, pas de Markdown, pas de LaTeX, pas d'abréviations ambiguës. Écris les formules et les symboles en toutes lettres.";

pub fn consigne(nom_chapitre: &str, o: &OptionsPodcast) -> String {
    let (a, b) = o.prenoms();
    let langue = match o.langue.as_str() {
        "allemand" => "en allemand (niveau B1, phrases claires)",
        "anglais" => "en anglais (niveau B1, phrases claires)",
        _ => "en français",
    };
    let ton = match o.ton.as_str() {
        "scolaire" => "clair et posé, comme un bon cours",
        "dynamique" => "énergique et rythmé, avec de l'enthousiasme",
        _ => "détendu et chaleureux, comme une discussion entre amis",
    };
    let detail = if o.detail == "approfondi" {
        "approfondi : toutes les notions, avec les exemples et les nuances des notes"
    } else {
        "l'essentiel : les notions clés à retenir pour un contrôle"
    };
    format!(
        "Écris le script d'un podcast de révision sur « {nom_chapitre} », {langue}, d'environ {mots} mots \
         (≈ {duree} minutes). C'est un dialogue entre {a} (voix « A » : pose les questions que se pose un élève, \
         reformule, fait des liens) et {b} (voix « B » : explique les notions à partir des notes). Ton : {ton}. \
         Niveau de détail : {detail}.\n\
         Structure : une accroche, l'annonce du plan, les notions dans un ordre logique (définitions, exemples \
         des notes, erreurs fréquentes à éviter), de petites questions de {a} pour vérifier la compréhension, \
         puis un récapitulatif des points essentiels. Chaque réplique fait 1 à 4 phrases. « voix » : « A » ou « B ». \
         « style » : courte indication de ton pour la voix (ex. « curieuse », « enthousiaste », « posé »). \
         « source » : identifiant de l'extrait utilisé, vide pour l'accroche, les transitions et la conclusion.",
        mots = o.mots_cibles(),
        duree = o.duree_minutes,
    )
}

pub fn schema() -> Value {
    json!({
        "type": "object",
        "properties": {
            "titre": { "type": "string", "description": "Titre court et accrocheur du podcast." },
            "repliques": {
                "type": "array",
                "items": {
                    "type": "object",
                    "properties": {
                        "voix": { "type": "string", "enum": ["A", "B"] },
                        "texte": { "type": "string" },
                        "style": { "type": "string" },
                        "source": { "type": "string" }
                    },
                    "required": ["voix", "texte", "style", "source"]
                }
            }
        },
        "required": ["titre", "repliques"]
    })
}

fn mots(t: &str) -> usize {
    t.split_whitespace().count()
}

/// Refuse un script trop court ou trop long, à une seule voix, avec du LaTeX, ou trop peu ancré dans les notes.
pub fn valider(s: &ScriptIa, o: &OptionsPodcast, extraits: &[Extrait]) -> Result<(), String> {
    if s.repliques.len() < 6 {
        return Err("au moins 6 répliques".into());
    }
    if !s.repliques.iter().any(|r| r.voix == "A") || !s.repliques.iter().any(|r| r.voix == "B") {
        return Err("les deux voix A et B doivent parler".into());
    }
    if let Some(r) = s.repliques.iter().find(|r| r.texte.trim().is_empty() || !["A", "B"].contains(&r.voix.as_str())) {
        return Err(format!("réplique invalide : « {} »", r.texte));
    }
    if s.repliques.iter().any(|r| r.texte.contains('$') || r.texte.contains("**")) {
        return Err("pas de LaTeX ni de Markdown : le texte est lu à voix haute".into());
    }
    let total: usize = s.repliques.iter().map(|r| mots(&r.texte)).sum();
    let cible = o.mots_cibles();
    if total * 10 < cible * 6 || total * 10 > cible * 16 {
        return Err(format!("{total} mots au lieu d'environ {cible}"));
    }
    let ancrees = s.repliques.iter().filter(|r| extrait_par_id(extraits, &r.source).is_some()).count();
    if ancrees * 3 < s.repliques.len() {
        return Err("trop peu de répliques citent un extrait des notes dans « source »".into());
    }
    Ok(())
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct Replique {
    /// « A » ou « B ».
    pub voix: String,
    pub nom: String,
    pub texte: String,
    pub style: String,
    /// Extrait source (vide si aucun).
    pub source: String,
    /// Morceau audio auquel appartient la réplique.
    pub morceau: usize,
    /// Début approximatif dans l'audio final (pour suivre la transcription).
    pub debut_ms: Option<u64>,
}

/// Répliques prêtes à enregistrer, regroupées en morceaux d'environ `MOTS_PAR_MORCEAU` mots.
pub fn preparer(s: ScriptIa, o: &OptionsPodcast, extraits: &[Extrait]) -> (String, Vec<Replique>, usize) {
    let (a, b) = o.prenoms();
    let mut repliques = Vec::new();
    let (mut morceau, mut taille) = (0, 0);
    for r in s.repliques {
        let n = mots(&r.texte);
        if taille > 0 && taille + n > MOTS_PAR_MORCEAU {
            morceau += 1;
            taille = 0;
        }
        taille += n;
        let source = extrait_par_id(extraits, &r.source).map(|e| e.id.clone()).unwrap_or_default();
        repliques.push(Replique {
            nom: if r.voix == "A" { a } else { b }.into(),
            voix: r.voix,
            texte: r.texte.trim().into(),
            style: r.style.trim().into(),
            source,
            morceau,
            debut_ms: None,
        });
    }
    (s.titre.trim().to_string(), repliques, morceau + 1)
}

/// Début de chaque réplique, réparti dans chaque morceau selon la longueur du texte.
pub fn minuter(repliques: &mut [Replique], durees_morceaux: &[u64]) {
    let mut debut_morceau = 0u64;
    for (k, &duree) in durees_morceaux.iter().enumerate() {
        let du_morceau: Vec<usize> = (0..repliques.len()).filter(|&i| repliques[i].morceau == k).collect();
        let total: u64 = du_morceau.iter().map(|&i| repliques[i].texte.chars().count() as u64).sum::<u64>().max(1);
        let mut t = debut_morceau;
        for i in du_morceau {
            repliques[i].debut_ms = Some(t);
            t += duree * repliques[i].texte.chars().count() as u64 / total;
        }
        debut_morceau += duree;
    }
}

// ---------- Stockage ----------

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct Orateur {
    pub nom: String,
    pub voix: String,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Podcast {
    pub id: i64,
    pub date: i64,
    pub matiere: String,
    pub chapitre: Option<String>,
    pub titre: String,
    pub options: OptionsPodcast,
    pub modele_script: String,
    pub modele_tts: String,
    pub orateurs: Vec<Orateur>,
    pub repliques: Vec<Replique>,
    pub extraits: Vec<Extrait>,
    pub morceaux: usize,
    pub morceaux_faits: usize,
    /// Nom du fichier audio final dans le dossier `audio` (absent tant que ce n'est pas assemblé).
    pub fichier: Option<String>,
    pub duree_ms: Option<i64>,
    pub position_ms: i64,
}

impl Podcast {
    pub fn fichier_morceau(&self, k: usize) -> String {
        format!("podcast-{}-{k}.wav", self.id)
    }
}

pub fn dossier_audio(dossier_donnees: &Path) -> Result<PathBuf, String> {
    let d = dossier_donnees.join("audio");
    std::fs::create_dir_all(&d).map_err(|e| format!("Dossier audio : {e}"))?;
    Ok(d)
}

fn json_texte<T: Serialize>(v: &T) -> String {
    serde_json::to_string(v).unwrap_or_default()
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
    options: &OptionsPodcast,
    modele_script: &str,
    modele_tts: &str,
    orateurs: &[Orateur],
    repliques: &[Replique],
    extraits: &[Extrait],
    morceaux: usize,
    date: i64,
) -> rusqlite::Result<i64> {
    conn.execute(
        "INSERT INTO podcasts(date, matiere, chapitre, titre, options, modele_script, modele_tts, orateurs, repliques,
                              extraits, morceaux)
         VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10, ?11)",
        params![
            date,
            matiere,
            chapitre,
            titre,
            json_texte(options),
            modele_script,
            modele_tts,
            json_texte(&orateurs),
            json_texte(&repliques),
            json_texte(&extraits),
            morceaux as i64
        ],
    )?;
    Ok(conn.last_insert_rowid())
}

const COLONNES: &str = "id, date, matiere, chapitre, titre, options, modele_script, modele_tts, orateurs, repliques, extraits,
                        morceaux, morceaux_faits, fichier, duree_ms, position_ms";

fn lire(r: &rusqlite::Row) -> rusqlite::Result<Podcast> {
    Ok(Podcast {
        id: r.get(0)?,
        date: r.get(1)?,
        matiere: r.get(2)?,
        chapitre: r.get(3)?,
        titre: r.get(4)?,
        options: lire_json(&r.get::<_, String>(5)?, 5)?,
        modele_script: r.get(6)?,
        modele_tts: r.get(7)?,
        orateurs: lire_json(&r.get::<_, String>(8)?, 8)?,
        repliques: lire_json(&r.get::<_, String>(9)?, 9)?,
        extraits: lire_json(&r.get::<_, String>(10)?, 10)?,
        morceaux: r.get::<_, i64>(11)? as usize,
        morceaux_faits: r.get::<_, i64>(12)? as usize,
        fichier: r.get(13)?,
        duree_ms: r.get(14)?,
        position_ms: r.get(15)?,
    })
}

pub fn lire_podcast(conn: &Connection, id: i64) -> rusqlite::Result<Option<Podcast>> {
    conn.query_row(&format!("SELECT {COLONNES} FROM podcasts WHERE id = ?1"), [id], lire).optional()
}

pub fn lister(conn: &Connection) -> rusqlite::Result<Vec<Podcast>> {
    conn.prepare(&format!("SELECT {COLONNES} FROM podcasts ORDER BY date DESC"))?.query_map([], lire)?.collect()
}

pub fn morceau_enregistre(conn: &Connection, id: i64, faits: usize) -> rusqlite::Result<()> {
    conn.execute("UPDATE podcasts SET morceaux_faits = ?2 WHERE id = ?1", params![id, faits as i64])?;
    Ok(())
}

pub fn definir_fichier(
    conn: &Connection,
    id: i64,
    fichier: &str,
    duree_ms: i64,
    repliques: &[Replique],
) -> rusqlite::Result<()> {
    conn.execute(
        "UPDATE podcasts SET fichier = ?2, duree_ms = ?3, repliques = ?4 WHERE id = ?1",
        params![id, fichier, duree_ms, json_texte(&repliques)],
    )?;
    Ok(())
}

pub fn definir_position(conn: &Connection, id: i64, position_ms: i64) -> rusqlite::Result<()> {
    conn.execute("UPDATE podcasts SET position_ms = ?2 WHERE id = ?1", params![id, position_ms.max(0)])?;
    Ok(())
}

/// Supprime le podcast et ses fichiers audio.
pub fn supprimer(conn: &Connection, dossier: &Path, id: i64) -> Result<(), String> {
    if let Some(p) = lire_podcast(conn, id).map_err(|e| e.to_string())? {
        for k in 0..p.morceaux {
            let _ = std::fs::remove_file(dossier.join(p.fichier_morceau(k)));
        }
        if let Some(f) = &p.fichier {
            let _ = std::fs::remove_file(dossier.join(f));
        }
    }
    conn.execute("DELETE FROM podcasts WHERE id = ?1", [id]).map_err(|e| e.to_string())?;
    Ok(())
}

/// Assemble les morceaux enregistrés en un seul WAV ; renvoie la durée totale et celle de chaque morceau.
pub fn assembler(dossier: &Path, p: &Podcast) -> Result<(Vec<u8>, u64, Vec<u64>), String> {
    let mut total: Option<Pcm> = None;
    let mut durees = Vec::new();
    for k in 0..p.morceaux {
        let octets = std::fs::read(dossier.join(p.fichier_morceau(k))).map_err(|e| format!("Morceau {} : {e}", k + 1))?;
        let pcm = audio::lire_wav(&octets)?;
        durees.push(pcm.duree_ms());
        match total.as_mut() {
            Some(t) => t.ajouter(&pcm)?,
            None => total = Some(pcm),
        }
    }
    let total = total.ok_or("Aucun morceau enregistré.")?;
    Ok((total.en_wav(), total.duree_ms(), durees))
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::db;

    fn options(duree: u32) -> OptionsPodcast {
        OptionsPodcast { duree_minutes: duree, ton: "detendu".into(), detail: "essentiel".into(), langue: "francais".into() }
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

    /// Script de `n` répliques de `m` mots, sources alternées.
    fn script(n: usize, m: usize) -> ScriptIa {
        ScriptIa {
            titre: " La méiose ".into(),
            repliques: (0..n)
                .map(|i| RepliqueIa {
                    voix: if i % 2 == 0 { "A" } else { "B" }.into(),
                    texte: vec!["mot"; m].join(" "),
                    style: "posé".into(),
                    source: if i % 2 == 0 { "E1".into() } else { String::new() },
                })
                .collect(),
        }
    }

    #[test]
    fn options_et_estimation() {
        assert!(options(10).valider().is_ok());
        assert!(options(30).valider().is_err());
        assert!(OptionsPodcast { langue: "latin".into(), ..options(5) }.valider().is_err());
        assert_eq!(appels_estimes(10), (1, 4));
        assert_eq!(appels_estimes(3), (1, 1));
        assert_eq!(OptionsPodcast { langue: "allemand".into(), ..options(5) }.prenoms(), ("Anna", "Jonas"));
    }

    #[test]
    fn validation_du_script() {
        let o = options(5); // 750 mots visés
        assert!(valider(&script(20, 40), &o, &extraits()).is_ok());
        assert!(valider(&script(4, 200), &o, &extraits()).is_err(), "trop peu de répliques");
        assert!(valider(&script(20, 10), &o, &extraits()).is_err(), "trop court");
        let mut une_voix = script(20, 40);
        une_voix.repliques.iter_mut().for_each(|r| r.voix = "A".into());
        assert!(valider(&une_voix, &o, &extraits()).is_err());
        let mut latex = script(20, 40);
        latex.repliques[3].texte = "on a $\\Delta = b^2 - 4ac$".into();
        assert!(valider(&latex, &o, &extraits()).is_err());
        let mut sans_source = script(20, 40);
        sans_source.repliques.iter_mut().for_each(|r| r.source = "E9".into());
        assert!(valider(&sans_source, &o, &extraits()).is_err());
    }

    #[test]
    fn morceaux_et_minutage() {
        let (titre, mut r, morceaux) = preparer(script(12, 100), &options(8), &extraits());
        assert_eq!(titre, "La méiose");
        // 4 répliques de 100 mots par morceau de 450 mots au plus.
        assert_eq!(morceaux, 3);
        assert_eq!(r.iter().map(|x| x.morceau).collect::<Vec<_>>(), [0, 0, 0, 0, 1, 1, 1, 1, 2, 2, 2, 2]);
        assert_eq!((r[0].nom.as_str(), r[1].nom.as_str()), ("Camille", "Théo"));
        assert_eq!((r[0].source.as_str(), r[1].source.as_str()), ("E1", ""));
        minuter(&mut r, &[40_000, 40_000, 20_000]);
        let debuts: Vec<u64> = r.iter().map(|x| x.debut_ms.unwrap()).collect();
        assert_eq!(&debuts[..5], &[0, 10_000, 20_000, 30_000, 40_000]);
        assert_eq!(debuts[8], 80_000);
        assert!(debuts.windows(2).all(|w| w[0] <= w[1]));
    }

    #[test]
    fn stockage_assemblage_suppression() {
        let conn = db::ouvrir_en_memoire().unwrap();
        let dossier = std::env::temp_dir().join(format!("revisio-test-podcast-{}", std::process::id()));
        std::fs::create_dir_all(&dossier).unwrap();
        let (titre, repliques, morceaux) = preparer(script(8, 100), &options(5), &extraits());
        let orateurs = vec![Orateur { nom: "Camille".into(), voix: "Kore".into() }, Orateur { nom: "Théo".into(), voix: "Puck".into() }];
        let id = creer(&conn, "SVT", Some("Méiose"), &titre, &options(5), "m", "tts", &orateurs, &repliques, &extraits(), morceaux, 1)
            .unwrap();
        let p = lire_podcast(&conn, id).unwrap().unwrap();
        assert_eq!((p.morceaux, p.morceaux_faits, p.fichier.as_deref()), (2, 0, None));
        assert!(assembler(&dossier, &p).is_err(), "rien n'est encore enregistré");

        for k in 0..p.morceaux {
            let pcm = Pcm { frequence: 24_000, canaux: 1, bits: 16, donnees: vec![0; 48_000 * (k + 1)] };
            std::fs::write(dossier.join(p.fichier_morceau(k)), pcm.en_wav()).unwrap();
            morceau_enregistre(&conn, id, k + 1).unwrap();
        }
        let (wav, duree, durees) = assembler(&dossier, &p).unwrap();
        assert_eq!((duree, durees), (3000, vec![1000, 2000]));
        std::fs::write(dossier.join("podcast-final.wav"), &wav).unwrap();
        definir_fichier(&conn, id, "podcast-final.wav", duree as i64, &p.repliques).unwrap();
        definir_position(&conn, id, 1234).unwrap();
        let p = lister(&conn).unwrap().remove(0);
        assert_eq!((p.morceaux_faits, p.fichier.as_deref(), p.position_ms), (2, Some("podcast-final.wav"), 1234));

        supprimer(&conn, &dossier, id).unwrap();
        assert!(lire_podcast(&conn, id).unwrap().is_none());
        assert!(std::fs::read_dir(&dossier).unwrap().next().is_none(), "fichiers supprimés");
        std::fs::remove_dir(&dossier).unwrap();
    }
}
