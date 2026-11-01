//! Cartes des cours créées par l'IA (demandé le 2026-11-01). Dès qu'une note de cours arrive ou s'allonge
//! nettement, l'IA en tire des cartes question / réponse et quelques QCM, ancrées sur la note (citation
//! vérifiée, comme dans l'Atelier IA). Les notes déjà là sont traitées aussi, en arrière-plan, les chapitres
//! des prochaines évaluations d'abord.
//!
//! Une fois une note traitée, ses textes à trous et listes à remettre dans l'ordre (cartes faites sans IA,
//! souvent hors contexte) sont retirés des révisions ; leur historique est gardé.

use std::collections::{BTreeMap, HashMap, HashSet};
use std::sync::Mutex;
use std::time::Duration;

use rusqlite::{params, Connection};
use serde::Serialize;
use tauri::{AppHandle, Emitter, Manager};

use crate::ia::ancrage::{extrait_cite, Extrait};
use crate::ia::gemini::{ErreurIa, Partie, Requete};
use crate::ia::questions::{self, LotQuestions};
use crate::{ia, maintenant_ms, Etat};

/// Genres de notes qui reçoivent des cartes (le vocabulaire a les siennes, sans IA).
pub const GENRES_DE_COURS: [&str; 5] = ["cours", "fiche_synthese", "methodologie", "reference", "etude_lineaire"];
/// Extraits envoyés en un appel : assez petit pour que l'IA couvre tout le texte.
const CARACTERES_PAR_LOT: usize = 4_500;
/// Une carte pour environ 250 caractères de cours (350 laissait de côté la moitié des notions, essai du 01/11).
const CARACTERES_PAR_CARTE: usize = 250;
/// Au-delà d'une vingtaine de cartes par appel, Gemini répond parfois « Request contains an invalid argument »
/// au bout de 2 à 3 minutes (constaté le 01/11 avec 28 cartes).
const CARTES_PAR_LOT: usize = 18;
/// En dessous, la note est laissée telle quelle (ses anciennes cartes restent).
const TAILLE_MINIMALE: usize = 150;
/// Une note qui échoue est réessayée une heure plus tard, trois fois au plus (ensuite : bouton « Réessayer »).
const ESSAIS_MAX: i64 = 3;
const PAUSE_APRES_ERREUR_MS: i64 = 3_600_000;

/// Note de cours qui attend des cartes.
#[derive(Debug, Clone, PartialEq)]
pub struct NoteAFaire {
    pub chemin: String,
    pub matiere: String,
    pub chapitre: Option<String>,
    pub genre: String,
    pub modifie: i64,
    /// Caractères du texte de la note aujourd'hui.
    pub taille: usize,
    /// Caractères déjà couverts par des cartes (0 : note jamais traitée).
    pub deja: usize,
}

impl NoteAFaire {
    /// Une carte pour ~250 caractères nouveaux ; 2 au moins pour une nouvelle note.
    fn cartes_visees(&self) -> usize {
        let n = self.taille.saturating_sub(self.deja) / CARACTERES_PAR_CARTE;
        if self.deja == 0 { n.max(2) } else { n.max(1) }
    }
}

/// Suivi d'une note dans `cartes_ia_notes`.
#[derive(Debug, Clone)]
struct SuiviNote {
    statut: String,
    taille: usize,
    essais: i64,
    maj: i64,
    en_erreur: bool,
}

/// La note a-t-elle besoin de (nouvelles) cartes ?
fn a_faire(suivi: Option<&SuiviNote>, taille: usize, maintenant: i64) -> bool {
    if taille < TAILLE_MINIMALE {
        return false;
    }
    let Some(s) = suivi else { return true };
    if s.en_erreur && (s.essais >= ESSAIS_MAX || maintenant - s.maj < PAUSE_APRES_ERREUR_MS) {
        return false;
    }
    // Jamais réussie, ou bien allongée depuis (au moins 30 % et 400 caractères de plus).
    s.statut == "erreur" || (taille >= s.taille + 400 && taille * 10 >= s.taille * 13)
}

/// Notes de cours qui attendent des cartes.
pub fn notes_a_faire(conn: &Connection, maintenant: i64) -> rusqlite::Result<Vec<NoteAFaire>> {
    let genres = GENRES_DE_COURS.map(|g| format!("'{g}'")).join(", ");
    let mut req = conn.prepare(&format!(
        "SELECT n.chemin, n.matiere, n.chapitre, n.genre, n.modifie,
                coalesce((SELECT sum(length(s.texte)) FROM sections s WHERE s.note_id = n.id), 0),
                t.statut, t.taille, t.essais, t.maj, t.erreur IS NOT NULL
         FROM notes n LEFT JOIN cartes_ia_notes t ON t.chemin = n.chemin
         WHERE n.matiere IS NOT NULL AND n.genre IN ({genres})
         ORDER BY n.chemin"
    ))?;
    let lignes = req.query_map([], |r| {
        let suivi = match r.get::<_, Option<String>>(6)? {
            Some(statut) => Some(SuiviNote {
                statut,
                taille: r.get::<_, i64>(7)? as usize,
                essais: r.get(8)?,
                maj: r.get(9)?,
                en_erreur: r.get(10)?,
            }),
            None => None,
        };
        let note = NoteAFaire {
            chemin: r.get(0)?,
            matiere: r.get(1)?,
            chapitre: r.get(2)?,
            genre: r.get(3)?,
            modifie: r.get(4)?,
            taille: r.get::<_, i64>(5)? as usize,
            deja: suivi.as_ref().filter(|s| s.statut != "erreur").map_or(0, |s| s.taille),
        };
        Ok((note, suivi))
    })?;
    let mut notes = Vec::new();
    for l in lignes {
        let (note, suivi) = l?;
        if a_faire(suivi.as_ref(), note.taille, maintenant) {
            notes.push(note);
        }
    }
    Ok(notes)
}

/// Un chapitre à traiter, avec ses notes.
#[derive(Debug, Clone, PartialEq)]
pub struct Chapitre {
    pub matiere: String,
    pub chapitre: Option<String>,
    pub notes: Vec<NoteAFaire>,
    /// Jour (AAAA-MM-JJ) de la prochaine évaluation qui porte sur ce chapitre.
    pub evaluation: Option<String>,
}

impl Chapitre {
    pub fn nom(&self) -> String {
        format!("{} › {}", self.matiere, self.chapitre.as_deref().unwrap_or("notes générales"))
    }
}

/// Évaluation à venir : jour, matière, chapitres (liste vide = toute la matière).
pub type Evaluation = (String, String, Vec<Option<String>>);

pub fn evaluations_a_venir(conn: &Connection, aujourdhui: &str) -> rusqlite::Result<Vec<Evaluation>> {
    let mut req = conn.prepare("SELECT jour, matiere, chapitres FROM evaluations WHERE jour >= ?1 ORDER BY jour")?;
    let lignes = req.query_map([aujourdhui], |r| {
        let chapitres: String = r.get(2)?;
        Ok((r.get(0)?, r.get(1)?, serde_json::from_str(&chapitres).unwrap_or_default()))
    })?;
    lignes.collect()
}

/// Chapitres dans l'ordre de traitement : ceux des prochaines évaluations (la plus proche d'abord), puis les
/// notes modifiées le plus récemment. Dans un chapitre, la fiche de synthèse passe après les notes de cours.
pub fn par_priorite(notes: Vec<NoteAFaire>, evaluations: &[Evaluation]) -> Vec<Chapitre> {
    let mut groupes: BTreeMap<(String, Option<String>), Vec<NoteAFaire>> = BTreeMap::new();
    for n in notes {
        groupes.entry((n.matiere.clone(), n.chapitre.clone())).or_default().push(n);
    }
    let mut chapitres: Vec<Chapitre> = groupes
        .into_iter()
        .map(|((matiere, chapitre), mut notes)| {
            notes.sort_by(|a, b| (a.genre == "fiche_synthese", &a.chemin).cmp(&(b.genre == "fiche_synthese", &b.chemin)));
            let evaluation = evaluations
                .iter()
                .find(|(_, m, chs)| *m == matiere && (chs.is_empty() || chs.contains(&chapitre)))
                .map(|(jour, _, _)| jour.clone());
            Chapitre { matiere, chapitre, notes, evaluation }
        })
        .collect();
    let recent = |c: &Chapitre| c.notes.iter().map(|n| n.modifie).max().unwrap_or(0);
    chapitres.sort_by(|a, b| match (&a.evaluation, &b.evaluation) {
        (Some(x), Some(y)) => x.cmp(y),
        (Some(_), None) => std::cmp::Ordering::Less,
        (None, Some(_)) => std::cmp::Ordering::Greater,
        (None, None) => recent(b).cmp(&recent(a)),
    });
    chapitres
}

/// Un appel à l'IA : des extraits (numérotés E1… dans le lot) et le nombre de cartes voulu.
#[derive(Debug, Clone, PartialEq)]
pub struct Lot {
    pub extraits: Vec<Extrait>,
    pub nombre: usize,
    /// Notes entièrement couvertes une fois ce lot fait, avec leur taille.
    pub finies: Vec<(String, usize)>,
}

/// Regroupe les notes d'un chapitre en lots d'au plus ~4 500 caractères. Une note n'est coupée que si elle
/// est trop longue à elle seule ; ses cartes sont alors réparties sur ses morceaux.
pub fn lots(notes: &[(NoteAFaire, Vec<Extrait>)]) -> Vec<Lot> {
    let mut lots: Vec<Lot> = Vec::new();
    let mut taille_lot = 0;
    for (note, extraits) in notes {
        // Morceaux de la note (extraits consécutifs).
        let mut morceaux: Vec<Vec<Extrait>> = Vec::new();
        let mut t = 0;
        for e in extraits {
            if morceaux.is_empty() || t + e.texte.len() > CARACTERES_PAR_LOT {
                morceaux.push(Vec::new());
                t = 0;
            }
            t += e.texte.len();
            morceaux.last_mut().unwrap().push(e.clone());
        }
        let total: usize = extraits.iter().map(|e| e.texte.len()).sum::<usize>().max(1);
        let visees = note.cartes_visees();
        let dernier = morceaux.len().saturating_sub(1);
        for (i, m) in morceaux.into_iter().enumerate() {
            let taille: usize = m.iter().map(|e| e.texte.len()).sum();
            if lots.is_empty() || taille_lot + taille > CARACTERES_PAR_LOT {
                lots.push(Lot { extraits: Vec::new(), nombre: 0, finies: Vec::new() });
                taille_lot = 0;
            }
            taille_lot += taille;
            let lot = lots.last_mut().unwrap();
            lot.nombre += (visees * taille).div_ceil(total).max(1);
            lot.extraits.extend(m);
            if i == dernier {
                lot.finies.push((note.chemin.clone(), note.taille));
            }
        }
    }
    for lot in &mut lots {
        lot.nombre = lot.nombre.clamp(2, CARTES_PAR_LOT);
        for (i, e) in lot.extraits.iter_mut().enumerate() {
            e.id = format!("E{}", i + 1);
        }
    }
    lots
}

/// Consigne : des cartes comme de bonnes fiches Quizlet, sur l'essentiel du cours.
pub fn consigne(nombre: usize, deja_posees: &[String]) -> String {
    let mut c = format!(
        "Crée {nombre} cartes de révision sur ces extraits de cours, comme de bonnes fiches Quizlet.\n\
         Environ trois quarts de cartes de type « question » : au recto une question précise, au verso une réponse \
         courte (un mot, une définition, une formule, une date, ou une à deux phrases). Le reste en QCM (4 propositions \
         affirmatives, une seule juste, distracteurs plausibles).\n\
         Règles :\n\
         - Une carte = une notion importante du cours : définition, mécanisme, cause et conséquence, formule (avec ses \
         unités), date ou personnage clé, méthode. Jamais de détail anecdotique, ni de question sur la mise en forme, \
         les titres, les liens ou « l'extrait ».\n\
         - La question se comprend seule, sans voir la note : nomme le sujet (« Pendant la méiose, … ») au lieu de \
         « ici » ou « dans ce cours ».\n\
         - La réponse se trouve dans l'extrait cité.\n\
         - Pas deux cartes sur la même chose. Répartis les cartes sur tous les extraits, en proportion de leur contenu.\n\
         - Formules en LaTeX entre $…$."
    );
    if !deja_posees.is_empty() {
        c += "\nCes cartes existent déjà : ne les refais pas.\n";
        for q in deja_posees.iter().take(40) {
            c += &format!("- {q}\n");
        }
    }
    c
}

/// Notes dont l'IA a fait les cartes : leurs textes à trous et listes à remettre dans l'ordre sont retirés.
pub fn notes_remplacees(conn: &Connection) -> rusqlite::Result<HashSet<String>> {
    let mut req = conn.prepare("SELECT chemin FROM cartes_ia_notes WHERE statut IN ('faite', 'vide')")?;
    let lignes = req.query_map([], |r| r.get(0))?;
    lignes.collect()
}

/// Les anciennes cartes sans IA de ces notes quittent les révisions (désactivées : l'historique reste).
fn retirer_anciennes(conn: &Connection, chemin: &str) -> rusqlite::Result<()> {
    conn.execute(
        "UPDATE cartes SET active = 0 WHERE note_chemin = ?1 AND genre IN ('trou', 'ordre') AND cle NOT LIKE 'ia|%'",
        [chemin],
    )?;
    Ok(())
}

fn marquer_faite(conn: &Connection, chemin: &str, taille: usize, cartes: usize, maintenant: i64) -> rusqlite::Result<()> {
    let statut = if cartes == 0 { "vide" } else { "faite" };
    conn.execute(
        "INSERT INTO cartes_ia_notes(chemin, statut, taille, cartes, essais, erreur, maj) VALUES (?1, ?2, ?3, ?4, 0, NULL, ?5)
         ON CONFLICT(chemin) DO UPDATE SET
           statut = CASE WHEN cartes_ia_notes.statut = 'faite' THEN 'faite' ELSE excluded.statut END,
           taille = excluded.taille, cartes = cartes_ia_notes.cartes + excluded.cartes,
           essais = 0, erreur = NULL, maj = excluded.maj",
        params![chemin, statut, taille as i64, cartes as i64, maintenant],
    )?;
    retirer_anciennes(conn, chemin)
}

fn marquer_erreur(conn: &Connection, chemin: &str, erreur: &str, maintenant: i64) -> rusqlite::Result<()> {
    conn.execute(
        "INSERT INTO cartes_ia_notes(chemin, statut, essais, erreur, maj) VALUES (?1, 'erreur', 1, ?2, ?3)
         ON CONFLICT(chemin) DO UPDATE SET essais = cartes_ia_notes.essais + 1, erreur = excluded.erreur, maj = excluded.maj",
        params![chemin, erreur, maintenant],
    )?;
    Ok(())
}

// ---------- Travail en arrière-plan ----------

/// Ce que l'interface affiche pendant le travail.
#[derive(Debug, Clone, Default, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct EtatCartesIa {
    /// Chapitre en cours (« SVT › La méiose »).
    pub en_cours: Option<String>,
    /// Pourquoi la création attend (pas de clé, hors ligne, limite atteinte…).
    pub attente: Option<String>,
    /// Depuis le dernier moment où tout était à jour.
    pub notes_traitees: usize,
    pub cartes_creees: usize,
    pub chapitres: Vec<String>,
}

#[derive(Default)]
pub struct Suivi {
    pub etat: Mutex<EtatCartesIa>,
    reveil: tokio::sync::Notify,
}

impl Suivi {
    /// Après une indexation, un changement de réglage ou « Réessayer ».
    pub fn reveiller(&self) {
        self.reveil.notify_one();
    }
}

/// Bilan affiché dans l'Atelier IA.
#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Bilan {
    pub actif: bool,
    pub etat: EtatCartesIa,
    /// Notes de cours assez longues pour avoir des cartes.
    pub notes: usize,
    pub faites: usize,
    pub a_faire: usize,
    pub cartes: usize,
    pub erreurs: Vec<ErreurNote>,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ErreurNote {
    pub chemin: String,
    pub titre: String,
    pub erreur: String,
}

pub fn bilan(conn: &Connection, actif: bool, etat: EtatCartesIa, maintenant: i64) -> rusqlite::Result<Bilan> {
    let genres = GENRES_DE_COURS.map(|g| format!("'{g}'")).join(", ");
    let (notes, faites, cartes): (i64, i64, i64) = conn.query_row(
        &format!(
            "SELECT count(*), coalesce(sum(t.statut IN ('faite', 'vide')), 0), coalesce(sum(t.cartes), 0)
             FROM notes n LEFT JOIN cartes_ia_notes t ON t.chemin = n.chemin
             WHERE n.matiere IS NOT NULL AND n.genre IN ({genres})
               AND coalesce((SELECT sum(length(s.texte)) FROM sections s WHERE s.note_id = n.id), 0) >= ?1"
        ),
        [TAILLE_MINIMALE as i64],
        |r| Ok((r.get(0)?, r.get(1)?, r.get(2)?)),
    )?;
    let mut req = conn.prepare(
        "SELECT t.chemin, coalesce(n.titre, t.chemin), t.erreur FROM cartes_ia_notes t
         JOIN notes n ON n.chemin = t.chemin WHERE t.erreur IS NOT NULL ORDER BY t.chemin",
    )?;
    let erreurs = req
        .query_map([], |r| Ok(ErreurNote { chemin: r.get(0)?, titre: r.get(1)?, erreur: r.get(2)? }))?
        .collect::<rusqlite::Result<Vec<_>>>()?;
    Ok(Bilan {
        actif,
        etat,
        notes: notes as usize,
        faites: faites as usize,
        a_faire: notes_a_faire(conn, maintenant)?.len(),
        cartes: cartes as usize,
        erreurs,
    })
}

/// « Réessayer » : les notes en échec repartent tout de suite.
pub fn reessayer(conn: &Connection) -> rusqlite::Result<()> {
    conn.execute("UPDATE cartes_ia_notes SET essais = 0, maj = 0 WHERE erreur IS NOT NULL", [])?;
    Ok(())
}

fn noter(app: &AppHandle, modif: impl FnOnce(&mut EtatCartesIa)) {
    let etat = app.state::<Etat>();
    let copie = {
        let mut e = etat.cartes_ia.etat.lock().unwrap();
        modif(&mut e);
        e.clone()
    };
    let _ = app.emit("cartes-ia", copie);
}

/// Lance le travail en arrière-plan (tant que l'app est ouverte).
pub fn demarrer(app: AppHandle) {
    tauri::async_runtime::spawn(async move {
        // On laisse l'indexation du lancement se faire.
        tokio::time::sleep(Duration::from_secs(20)).await;
        loop {
            let pause = tour(&app).await;
            let etat = app.state::<Etat>();
            tokio::select! {
                _ = etat.cartes_ia.reveil.notified() => {}
                _ = tokio::time::sleep(pause) => {}
            }
        }
    });
}

/// Traite les chapitres en attente ; renvoie le temps à attendre avant de regarder à nouveau.
async fn tour(app: &AppHandle) -> Duration {
    let etat = app.state::<Etat>();
    loop {
        let (actif, vault) = {
            let r = etat.reglages.lock().unwrap();
            (r.cartes_ia_auto, r.chemin_vault.is_some())
        };
        if !actif || !vault {
            noter(app, |e| {
                e.en_cours = None;
                e.attente = None;
            });
            return Duration::from_secs(600);
        }
        if etat.indexation.lock().unwrap().en_cours {
            return Duration::from_secs(30);
        }
        let prochain = {
            let conn = etat.db.lock().unwrap();
            let aujourdhui = chrono::Local::now().format("%Y-%m-%d").to_string();
            notes_a_faire(&conn, maintenant_ms())
                .and_then(|n| Ok(par_priorite(n, &evaluations_a_venir(&conn, &aujourdhui)?)))
                .map(|c| c.into_iter().next())
        };
        let chapitre = match prochain {
            Ok(Some(c)) => c,
            Ok(None) => {
                // Tout est à jour : une notification résume ce qui vient d'être fait.
                let fini = {
                    let mut e = etat.cartes_ia.etat.lock().unwrap();
                    let fini = (e.notes_traitees > 0).then(|| e.clone());
                    *e = EtatCartesIa::default();
                    fini
                };
                if let Some(f) = fini {
                    let _ = app.emit("cartes-ia-fini", f);
                }
                noter(app, |_| {});
                return Duration::from_secs(600);
            }
            Err(_) => return Duration::from_secs(600),
        };
        if let Err(pause) = traiter(app, &etat, &chapitre).await {
            return pause;
        }
    }
}

/// Erreurs qui empêchent tout travail pour le moment (et non la note elle-même) : on attend.
fn pause_pour(e: &ErreurIa) -> Option<Duration> {
    match e {
        ErreurIa::Quota(_) => Some(Duration::from_secs(900)),
        ErreurIa::Surcharge => Some(Duration::from_secs(300)),
        ErreurIa::HorsLigne => Some(Duration::from_secs(120)),
        ErreurIa::PasDeCle
        | ErreurIa::CleInvalide
        | ErreurIa::GeminiCoupe
        | ErreurIa::OllamaAbsent
        | ErreurIa::ModeleLocalAbsent(_)
        | ErreurIa::Secours { .. } => Some(Duration::from_secs(600)),
        _ => None,
    }
}

async fn traiter(app: &AppHandle, etat: &Etat, chapitre: &Chapitre) -> Result<(), Duration> {
    let nom = chapitre.nom();
    noter(app, |e| {
        e.en_cours = Some(nom.clone());
        e.attente = None;
    });
    let (notes, deja) = {
        let conn = etat.db.lock().unwrap();
        let mut notes = Vec::new();
        for n in &chapitre.notes {
            let extraits = ia::ancrage::extraits_de_la_note(&conn, &n.chemin).unwrap_or_default();
            if extraits.is_empty() {
                let _ = marquer_faite(&conn, &n.chemin, n.taille, 0, maintenant_ms());
            } else {
                notes.push((n.clone(), extraits));
            }
        }
        let deja = questions::deja_posees(&conn, &chapitre.matiere, chapitre.chapitre.as_deref()).unwrap_or_default();
        (notes, deja)
    };
    // Modèle rapide : le modèle principal n'a que 20 appels par jour en gratuit (gemini-3.8-flash, constaté le
    // 01/11), souvent surchargé (1 à 4 min par lot) ; flash-lite fait un lot de 18 cartes valides en 13 s. Le quota
    // du modèle principal reste ainsi pour l'Atelier, les photos et la discussion.
    let modele = etat.reglages.lock().unwrap().modele_ia_rapide.clone();
    // L'IA locale seulement si l'élève l'a choisie pour tout (pas en secours : des heures de calcul en silence).
    let local = etat.config_locale().filter(|l| l.mode == ia::ModeLocal::Toujours);
    for lot in lots(&notes) {
        if !etat.reglages.lock().unwrap().cartes_ia_auto {
            return Ok(());
        }
        let requete = Requete {
            modele: modele.clone(),
            systeme: ia::ancrage::CONSIGNE_ANCRAGE.into(),
            parties: vec![Partie::Texte(format!(
                "{}\n\nExtraits des notes ({nom}) :\n\n{}",
                consigne(lot.nombre, &deja),
                ia::ancrage::texte_des_extraits(&lot.extraits)
            ))],
            schema: Some(questions::schema_types(lot.nombre, &["question", "qcm"])),
            temperature: Some(0.5),
            historique: Vec::new(),
        };
        let valider = |l: &LotQuestions| {
            let raisons: Vec<String> =
                l.questions.iter().filter_map(|q| questions::preparer(q.clone(), &lot.extraits).err()).collect();
            if raisons.len() < l.questions.len() {
                Ok(())
            } else {
                Err(format!("aucune carte valide ({})", raisons[..raisons.len().min(3)].join(" ; ")))
            }
        };
        let reponse = etat.ia.appeler_json_simple(&etat.db, "cartes", etat.limites_ia(), local.as_ref(), &requete, valider).await;
        let maintenant = maintenant_ms();
        match reponse {
            Ok((reponse, modele)) => {
                let bonnes: Vec<_> =
                    reponse.questions.into_iter().filter_map(|q| questions::preparer(q, &lot.extraits).ok()).collect();
                let mut par_note: HashMap<String, usize> = HashMap::new();
                for q in &bonnes {
                    if let Some(e) = extrait_cite(&lot.extraits, &q.source, &q.citation) {
                        *par_note.entry(e.note_chemin.clone()).or_default() += 1;
                    }
                }
                let creees = {
                    let mut conn = etat.db.lock().unwrap();
                    let creees = questions::enregistrer(
                        &mut conn,
                        &modele,
                        &chapitre.matiere,
                        chapitre.chapitre.as_deref(),
                        &lot.extraits,
                        &bonnes,
                        maintenant,
                    )
                    .unwrap_or(0);
                    for (chemin, taille) in &lot.finies {
                        let _ = marquer_faite(&conn, chemin, *taille, par_note.get(chemin).copied().unwrap_or(0), maintenant);
                    }
                    creees
                };
                noter(app, |e| {
                    e.notes_traitees += lot.finies.len();
                    e.cartes_creees += creees;
                    if !e.chapitres.contains(&nom) {
                        e.chapitres.push(nom.clone());
                    }
                });
            }
            Err(e) => match pause_pour(&e) {
                Some(pause) => {
                    noter(app, |s| {
                        s.en_cours = None;
                        s.attente = Some(e.to_string());
                    });
                    return Err(pause);
                }
                None => {
                    let conn = etat.db.lock().unwrap();
                    let chemins: HashSet<&String> = lot.extraits.iter().map(|x| &x.note_chemin).collect();
                    for c in chemins {
                        let _ = marquer_erreur(&conn, c, &e.to_string(), maintenant);
                    }
                }
            },
        }
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::db;

    fn note(chemin: &str, chapitre: &str, taille: usize, modifie: i64) -> NoteAFaire {
        NoteAFaire {
            chemin: chemin.into(),
            matiere: "SVT".into(),
            chapitre: Some(chapitre.into()),
            genre: if chemin.contains("Fiche") { "fiche_synthese".into() } else { "cours".into() },
            modifie,
            taille,
            deja: 0,
        }
    }

    fn extrait(chemin: &str, taille: usize) -> Extrait {
        Extrait {
            id: "E1".into(),
            note_chemin: chemin.into(),
            titre_note: chemin.into(),
            section: String::new(),
            texte: "x".repeat(taille),
            markdown: String::new(),
        }
    }

    #[test]
    fn notes_a_refaire_quand_elles_s_allongent() {
        let fait = |taille| SuiviNote { statut: "faite".into(), taille, essais: 0, maj: 0, en_erreur: false };
        assert!(a_faire(None, 2_000, 0));
        assert!(!a_faire(None, 100, 0), "trop courte");
        assert!(!a_faire(Some(&fait(2_000)), 2_300, 0), "à peine retouchée");
        assert!(a_faire(Some(&fait(2_000)), 2_700, 0), "bien allongée");
        assert!(a_faire(Some(&fait(200)), 650, 0));
        let erreur = |essais, maj| SuiviNote { statut: "erreur".into(), taille: 0, essais, maj, en_erreur: true };
        assert!(!a_faire(Some(&erreur(1, 0)), 2_000, 1_000), "on attend une heure");
        assert!(a_faire(Some(&erreur(1, 0)), 2_000, PAUSE_APRES_ERREUR_MS));
        assert!(!a_faire(Some(&erreur(ESSAIS_MAX, 0)), 2_000, PAUSE_APRES_ERREUR_MS), "abandon après 3 essais");
    }

    #[test]
    fn evaluations_d_abord_puis_notes_recentes() {
        let notes = vec![
            note("SVT/Méiose/Fiche de synthèse.md", "Méiose", 3_000, 10),
            note("SVT/Méiose/Les étapes.md", "Méiose", 2_000, 10),
            note("SVT/Génétique/ADN.md", "Génétique", 2_000, 50),
            note("SVT/Immunité/Anticorps.md", "Immunité", 2_000, 90),
        ];
        let evals = vec![("2026-11-12".to_string(), "SVT".to_string(), vec![Some("Méiose".to_string())])];
        let c = par_priorite(notes, &evals);
        let noms: Vec<_> = c.iter().map(|c| c.chapitre.clone().unwrap()).collect();
        assert_eq!(noms, ["Méiose", "Immunité", "Génétique"]);
        assert_eq!(c[0].evaluation.as_deref(), Some("2026-11-12"));
        // La fiche de synthèse après les notes de cours.
        assert_eq!(c[0].notes[1].chemin, "SVT/Méiose/Fiche de synthèse.md");
        // Une évaluation sans chapitre précis couvre toute la matière.
        let toute = vec![("2026-11-12".to_string(), "SVT".to_string(), vec![])];
        assert!(par_priorite(vec![note("SVT/Génétique/ADN.md", "Génétique", 2_000, 1)], &toute)[0].evaluation.is_some());
    }

    #[test]
    fn lots_sans_couper_les_notes() {
        let a = note("A.md", "C", 2_000, 0);
        let b = note("B.md", "C", 2_000, 0);
        let longue = note("L.md", "C", 9_000, 0);
        let notes = vec![
            (a.clone(), vec![extrait("A.md", 1_200), extrait("A.md", 800)]),
            (b.clone(), vec![extrait("B.md", 2_000)]),
            (longue.clone(), (0..3).map(|_| extrait("L.md", 3_000)).collect()),
        ];
        let l = lots(&notes);
        // A + B (4 000) ; L coupée en 3 morceaux de 3 000.
        assert_eq!(l.len(), 4);
        assert_eq!(l[0].finies, vec![("A.md".to_string(), 2_000), ("B.md".to_string(), 2_000)]);
        assert_eq!(l[0].nombre, 8 + 8);
        assert!(l[1].finies.is_empty() && l[2].finies.is_empty());
        assert_eq!(l[3].finies, vec![("L.md".to_string(), 9_000)]);
        assert_eq!(l.iter().skip(1).map(|x| x.nombre).collect::<Vec<_>>(), [12, 12, 12]);
        // Identifiants renumérotés dans chaque lot.
        assert_eq!(l[0].extraits.iter().map(|e| e.id.as_str()).collect::<Vec<_>>(), ["E1", "E2", "E3"]);
        // Une note allongée : seulement les cartes de la partie nouvelle.
        let allongee = NoteAFaire { deja: 3_000, taille: 4_400, ..a };
        assert_eq!(allongee.cartes_visees(), 5);
    }

    #[test]
    fn suivi_et_remplacement_des_anciennes_cartes() {
        let conn = db::ouvrir_en_memoire().unwrap();
        conn.execute_batch(
            "INSERT INTO cartes(cle, genre, note_chemin, question, reponse, donnees, extrait, creee_le) VALUES
               ('trou|SVT/A.md|1', 'trou', 'SVT/A.md', 'q', 'r', '{}', '', 0),
               ('vocab|SVT/A.md|x|vers_fr', 'vocabulaire', 'SVT/A.md', 'q', 'r', '{}', '', 0),
               ('trou|SVT/B.md|1', 'trou', 'SVT/B.md', 'q', 'r', '{}', '', 0);",
        )
        .unwrap();
        marquer_faite(&conn, "SVT/A.md", 2_000, 5, 1).unwrap();
        let actives: Vec<String> =
            conn.prepare("SELECT cle FROM cartes WHERE active = 1 ORDER BY cle").unwrap().query_map([], |r| r.get(0)).unwrap().map(Result::unwrap).collect();
        assert_eq!(actives, ["trou|SVT/B.md|1", "vocab|SVT/A.md|x|vers_fr"]);
        assert_eq!(notes_remplacees(&conn).unwrap(), HashSet::from(["SVT/A.md".to_string()]));
        // Une erreur plus tard ne fait pas oublier que la note avait ses cartes.
        marquer_erreur(&conn, "SVT/A.md", "réponse illisible", 2).unwrap();
        assert!(notes_remplacees(&conn).unwrap().contains("SVT/A.md"));
        marquer_faite(&conn, "SVT/A.md", 3_000, 2, 3).unwrap();
        let (cartes, erreur): (i64, Option<String>) = conn
            .query_row("SELECT cartes, erreur FROM cartes_ia_notes WHERE chemin = 'SVT/A.md'", [], |r| Ok((r.get(0)?, r.get(1)?)))
            .unwrap();
        assert_eq!((cartes, erreur), (7, None));
    }
}
