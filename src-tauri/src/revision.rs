//! Séances de révision : choix des cartes, enregistrement des réponses, carnet d'erreurs.
//!
//! Le calcul FSRS (prochaine date de révision) est fait par l'interface avec ts-fsrs ;
//! ce module stocke l'état obtenu et l'historique.

use rusqlite::{params, Connection, OptionalExtension};
use serde::{Deserialize, Serialize};
use serde_json::Value;

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct CarteSeance {
    pub id: i64,
    pub genre: String,
    pub note_chemin: String,
    pub titre_note: String,
    pub matiere: Option<String>,
    pub chapitre: Option<String>,
    pub section: String,
    pub question: String,
    pub reponse: String,
    pub donnees: Value,
    pub extrait: String,
    /// État FSRS (JSON ts-fsrs) ; absent pour une carte jamais révisée.
    pub etat_fsrs: Option<String>,
    pub echeance: Option<i64>,
}

#[derive(Debug, Default, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct FiltreSeance {
    pub matiere: Option<String>,
    pub chapitre: Option<String>,
    /// Genres de cartes voulus (vide = tous).
    #[serde(default)]
    pub genres: Vec<String>,
    /// Début de la journée locale (ms), calculé par l'interface.
    pub debut_jour: i64,
    /// Nombre maximal de cartes dues renvoyées.
    pub limite_dues: Option<usize>,
    /// Séance du jour « Langues » ou « Autres matières » (aucun = les deux).
    pub groupe: Option<Groupe>,
    /// Chapitres du planning d'aujourd'hui : leurs cartes passent en premier, même pas encore dues.
    #[serde(default)]
    pub planifies: Vec<ChapitrePlanifie>,
    /// Séance ciblée sur des chapitres choisis (vide = pas de restriction).
    #[serde(default)]
    pub chapitres: Vec<ChapitrePlanifie>,
    /// Matières laissées de côté (séances du jour) : posé par Révisio d'après les réglages, pas par l'interface.
    #[serde(skip)]
    pub exclues: Vec<String>,
}

/// Un chapitre d'une matière (`matiere` absente : notes sans matière ; `chapitre` absent : notes générales).
#[derive(Debug, Clone, Deserialize)]
pub struct ChapitrePlanifie {
    pub matiere: Option<String>,
    pub chapitre: Option<String>,
}

/// Matière affichée pour une carte sans matière.
pub const HORS_MATIERE: &str = "Hors matière";

/// `(c.matiere IS ? AND c.chapitre IS ?) OR …` pour une liste de chapitres (valeurs ajoutées à `p`).
fn condition_chapitres(chapitres: &[ChapitrePlanifie], p: &mut Vec<rusqlite::types::Value>) -> String {
    use rusqlite::types::Value as V;
    let morceaux: Vec<String> = chapitres
        .iter()
        .map(|c| {
            p.push(c.matiere.clone().map_or(V::Null, V::Text));
            p.push(c.chapitre.clone().map_or(V::Null, V::Text));
            format!("(c.matiere IS ?{} AND c.chapitre IS ?{})", p.len() - 1, p.len())
        })
        .collect();
    format!("({})", morceaux.join(" OR "))
}

/// `coalesce(c.matiere, 'Hors matière') NOT IN (…)` (valeurs ajoutées à `p`), ou rien si la liste est vide.
fn condition_exclues(exclues: &[String], p: &mut Vec<rusqlite::types::Value>) -> String {
    if exclues.is_empty() {
        return String::new();
    }
    let marques: Vec<String> = exclues
        .iter()
        .map(|m| {
            p.push(rusqlite::types::Value::Text(m.clone()));
            format!("?{}", p.len())
        })
        .collect();
    format!(" AND coalesce(c.matiere, '{HORS_MATIERE}') NOT IN ({})", marques.join(", "))
}

/// Les deux séances du jour sont séparées : les langues (toutes les cartes de vocabulaire,
/// quelle que soit la matière) et tout le reste.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum Groupe {
    Langues,
    Autres,
}

impl Groupe {
    pub const TOUS: [Groupe; 2] = [Groupe::Langues, Groupe::Autres];

    /// Condition SQL sur la table `cartes` (alias `c`).
    pub fn condition(self) -> &'static str {
        match self {
            Groupe::Langues => "c.genre = 'vocabulaire'",
            Groupe::Autres => "c.genre <> 'vocabulaire'",
        }
    }
}

/// Nombre de nouvelles cartes autorisées par jour, pour chaque groupe.
#[derive(Debug, Clone, Copy)]
pub struct Quotas {
    pub langues: usize,
    pub autres: usize,
}

impl Quotas {
    pub fn de(&self, g: Groupe) -> usize {
        match g {
            Groupe::Langues => self.langues,
            Groupe::Autres => self.autres,
        }
    }
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Seance {
    /// Cartes des chapitres du planning (révision anticipée avant une évaluation), sauf celles
    /// déjà révisées aujourd'hui : échéance la plus proche d'abord, puis les cartes jamais vues.
    pub planifiees: Vec<CarteSeance>,
    pub dues: Vec<CarteSeance>,
    /// Points faibles : cartes déjà vues, pas encore dues, avec des erreurs ou des oublis.
    pub faibles: Vec<CarteSeance>,
    pub nouvelles: Vec<CarteSeance>,
}

/// Score de faiblesse d'une carte : erreurs non comprises (×2) + oublis FSRS.
const SCORE_FAIBLESSE: &str = "(2 * (SELECT count(*) FROM erreurs e WHERE e.carte_id = c.id AND e.comprise = 0)
     + coalesce(json_extract(c.etat_fsrs, '$.lapses'), 0))";

const COLONNES: &str = "c.id, c.genre, c.note_chemin, coalesce(n.titre, c.note_chemin), c.matiere, c.chapitre, c.section,
     c.question, c.reponse, c.donnees, c.extrait, c.etat_fsrs, c.echeance";

fn lire_carte(r: &rusqlite::Row) -> rusqlite::Result<CarteSeance> {
    Ok(CarteSeance {
        id: r.get(0)?,
        genre: r.get(1)?,
        note_chemin: r.get(2)?,
        titre_note: r.get(3)?,
        matiere: r.get(4)?,
        chapitre: r.get(5)?,
        section: r.get(6)?,
        question: r.get(7)?,
        reponse: r.get(8)?,
        donnees: serde_json::from_str(&r.get::<_, String>(9)?).unwrap_or(Value::Null),
        extrait: r.get(10)?,
        etat_fsrs: r.get(11)?,
        echeance: r.get(12)?,
    })
}

/// Conditions SQL communes (carte utilisable + filtres de la séance).
fn conditions(f: &FiltreSeance) -> (String, Vec<rusqlite::types::Value>) {
    use rusqlite::types::Value as V;
    let mut sql = String::from("c.active = 1 AND c.suspendue = 0");
    let mut valeurs: Vec<V> = Vec::new();
    if let Some(m) = &f.matiere {
        valeurs.push(V::Text(m.clone()));
        sql += &format!(" AND c.matiere = ?{}", valeurs.len());
    }
    if let Some(ch) = &f.chapitre {
        valeurs.push(V::Text(ch.clone()));
        sql += &format!(" AND c.chapitre = ?{}", valeurs.len());
    }
    if !f.genres.is_empty() {
        let marques: Vec<String> = f
            .genres
            .iter()
            .map(|g| {
                valeurs.push(V::Text(g.clone()));
                format!("?{}", valeurs.len())
            })
            .collect();
        sql += &format!(" AND c.genre IN ({})", marques.join(", "));
    }
    if let Some(g) = f.groupe {
        sql += &format!(" AND {}", g.condition());
    }
    if !f.chapitres.is_empty() {
        sql += &format!(" AND {}", condition_chapitres(&f.chapitres, &mut valeurs));
    }
    sql += &condition_exclues(&f.exclues, &mut valeurs);
    (sql, valeurs)
}

/// Nombre de nouvelles cartes du groupe déjà commencées aujourd'hui.
pub fn nouvelles_du_jour(conn: &Connection, debut_jour: i64, groupe: Groupe) -> rusqlite::Result<usize> {
    conn.query_row(
        &format!("SELECT count(*) FROM cartes c WHERE c.introduite_le >= ?1 AND {}", groupe.condition()),
        [debut_jour],
        |r| r.get::<_, i64>(0),
    )
    .map(|n| n as usize)
}

/// Cartes dues (à revoir maintenant), points faibles, puis nouvelles cartes dans la limite
/// du quota quotidien de chaque groupe.
pub fn preparer_seance(conn: &Connection, f: &FiltreSeance, maintenant: i64, quotas: Quotas) -> rusqlite::Result<Seance> {
    let (cond, valeurs) = conditions(f);
    let n = valeurs.len();

    let sql_dues = format!(
        "SELECT {COLONNES} FROM cartes c LEFT JOIN notes n ON n.chemin = c.note_chemin
         WHERE {cond} AND c.echeance IS NOT NULL AND c.echeance <= ?{} ORDER BY c.echeance LIMIT ?{}",
        n + 1,
        n + 2
    );
    let mut p = valeurs.clone();
    p.push(maintenant.into());
    p.push((f.limite_dues.unwrap_or(500) as i64).into());
    let dues = conn
        .prepare(&sql_dues)?
        .query_map(rusqlite::params_from_iter(p), lire_carte)?
        .collect::<rusqlite::Result<Vec<_>>>()?;

    let sql_faibles = format!(
        "SELECT {COLONNES} FROM cartes c LEFT JOIN notes n ON n.chemin = c.note_chemin
         WHERE {cond} AND c.etat_fsrs IS NOT NULL AND c.echeance > ?{} AND {SCORE_FAIBLESSE} > 0
         ORDER BY {SCORE_FAIBLESSE} DESC LIMIT 30",
        n + 1
    );
    let mut p = valeurs.clone();
    p.push(maintenant.into());
    let faibles = conn
        .prepare(&sql_faibles)?
        .query_map(rusqlite::params_from_iter(p), lire_carte)?
        .collect::<rusqlite::Result<Vec<_>>>()?;

    // Nouvelles cartes : chaque groupe a son propre quota quotidien.
    let mut nouvelles = Vec::new();
    let groupes: Vec<Groupe> = f.groupe.map_or(Groupe::TOUS.to_vec(), |g| vec![g]);
    for g in groupes {
        let quota = quotas.de(g).saturating_sub(nouvelles_du_jour(conn, f.debut_jour, g)?);
        let sql_nouvelles = format!(
            "SELECT {COLONNES} FROM cartes c LEFT JOIN notes n ON n.chemin = c.note_chemin
             WHERE {cond} AND {} AND c.etat_fsrs IS NULL ORDER BY c.id LIMIT ?{}",
            g.condition(),
            n + 1
        );
        let mut p = valeurs.clone();
        p.push((quota as i64).into());
        nouvelles.extend(
            conn.prepare(&sql_nouvelles)?
                .query_map(rusqlite::params_from_iter(p), lire_carte)?
                .collect::<rusqlite::Result<Vec<_>>>()?,
        );
    }

    let planifiees = cartes_planifiees(conn, f, &cond, &valeurs)?;
    Ok(Seance { planifiees, dues, faibles, nouvelles })
}

/// Cartes des chapitres planifiés aujourd'hui (le quota de nouvelles cartes ne s'applique pas :
/// avant une évaluation, tout le chapitre doit avoir été vu).
fn cartes_planifiees(
    conn: &Connection,
    f: &FiltreSeance,
    cond: &str,
    valeurs: &[rusqlite::types::Value],
) -> rusqlite::Result<Vec<CarteSeance>> {
    if f.planifies.is_empty() {
        return Ok(Vec::new());
    }
    let mut p = valeurs.to_vec();
    let chapitres = condition_chapitres(&f.planifies, &mut p);
    p.push(f.debut_jour.into());
    let sql = format!(
        "SELECT {COLONNES} FROM cartes c LEFT JOIN notes n ON n.chemin = c.note_chemin
         WHERE {cond} AND {chapitres} AND c.id NOT IN (SELECT carte_id FROM revisions WHERE date >= ?{})
         ORDER BY c.etat_fsrs IS NULL, c.echeance, c.id LIMIT 300",
        p.len()
    );
    conn.prepare(&sql)?.query_map(rusqlite::params_from_iter(p), lire_carte)?.collect()
}

const JOUR_MS: i64 = 86_400_000;
/// Les cartes « déjà apprises » sont revérifiées une première fois sur ce nombre de jours.
pub const JOURS_ETALEMENT: i64 = 30;

/// Date ISO 8601 en UTC (format des dates JavaScript, lu par ts-fsrs) d'un instant en ms.
pub fn date_iso(ms: i64) -> String {
    let (secondes, millis) = (ms.div_euclid(1000), ms.rem_euclid(1000));
    let (jours, s) = (secondes.div_euclid(86_400), secondes.rem_euclid(86_400));
    // Conversion jours → date du calendrier (algorithme « civil_from_days » de H. Hinnant).
    let z = jours + 719_468;
    let ere = z.div_euclid(146_097);
    let doe = z.rem_euclid(146_097);
    let yoe = (doe - doe / 1460 + doe / 36_524 - doe / 146_096) / 365;
    let doy = doe - (365 * yoe + yoe / 4 - yoe / 100);
    let mp = (5 * doy + 2) / 153;
    let jour = doy - (153 * mp + 2) / 5 + 1;
    let mois = if mp < 10 { mp + 3 } else { mp - 9 };
    let annee = yoe + ere * 400 + if mois <= 2 { 1 } else { 0 };
    format!("{annee:04}-{mois:02}-{jour:02}T{:02}:{:02}:{:02}.{millis:03}Z", s / 3600, (s % 3600) / 60, s % 60)
}

/// « Ces cartes sont déjà apprises » : chaque carte jamais révisée reçoit un état FSRS
/// « en révision » (mémoire d'environ un mois) et une première vérification étalée sur les
/// 30 prochains jours (une carte sur 30 chaque jour, dans l'ordre de création : chaque
/// chapitre est réparti sur tout le mois). Les cartes créées plus tard restent nouvelles.
pub fn marquer_apprises(conn: &mut Connection, maintenant: i64) -> rusqlite::Result<usize> {
    let tx = conn.transaction()?;
    let ids: Vec<i64> = tx
        .prepare("SELECT id FROM cartes WHERE active = 1 AND suspendue = 0 AND etat_fsrs IS NULL ORDER BY id")?
        .query_map([], |r| r.get(0))?
        .collect::<rusqlite::Result<_>>()?;
    for (i, id) in ids.iter().enumerate() {
        let jours = (i as i64 % JOURS_ETALEMENT) + 1;
        let echeance = maintenant + jours * JOUR_MS;
        let etat = serde_json::json!({
            "due": date_iso(echeance),
            "stability": JOURS_ETALEMENT as f64,
            "difficulty": 5.0,
            "elapsed_days": 0,
            "scheduled_days": jours,
            "learning_steps": 0,
            "reps": 1,
            "lapses": 0,
            "state": 2,
            "last_review": date_iso(maintenant),
        });
        // introduite_le = 0 : ces cartes ne comptent pas dans le quota de nouvelles cartes.
        tx.execute(
            "UPDATE cartes SET etat_fsrs = ?2, echeance = ?3, introduite_le = 0 WHERE id = ?1",
            params![id, etat.to_string(), echeance],
        )?;
    }
    tx.commit()?;
    Ok(ids.len())
}

/// Durée moyenne d'une réponse (sur les 200 dernières), bornée entre 8 et 60 s ; 20 s par défaut.
pub fn duree_moyenne_ms(conn: &Connection) -> rusqlite::Result<i64> {
    let moyenne: Option<f64> = conn.query_row(
        "SELECT avg(duree_ms) FROM (SELECT duree_ms FROM revisions ORDER BY date DESC LIMIT 200)",
        [],
        |r| r.get(0),
    )?;
    Ok(moyenne.map_or(20_000, |m| (m as i64).clamp(8_000, 60_000)))
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct CarteStat {
    pub matiere: Option<String>,
    pub chapitre: Option<String>,
    pub genre: String,
    pub etat_fsrs: Option<String>,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Jour {
    /// Jour local, au format AAAA-MM-JJ.
    pub jour: String,
    pub revisions: i64,
    pub justes: i64,
    pub duree_ms: i64,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct NotionFaible {
    pub carte_id: i64,
    pub genre: String,
    pub question: String,
    pub reponse: String,
    pub matiere: Option<String>,
    pub chapitre: Option<String>,
    pub note_chemin: String,
    pub section: String,
    pub erreurs: i64,
    pub oublis: i64,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct TableauDeBord {
    pub cartes: Vec<CarteStat>,
    /// Activité par jour (365 derniers jours avec au moins une révision), du plus récent au plus ancien.
    pub jours: Vec<Jour>,
    pub faibles: Vec<NotionFaible>,
}

pub fn tableau_de_bord(conn: &Connection) -> rusqlite::Result<TableauDeBord> {
    let cartes = conn
        .prepare("SELECT matiere, chapitre, genre, etat_fsrs FROM cartes WHERE active = 1 AND suspendue = 0")?
        .query_map([], |r| {
            Ok(CarteStat { matiere: r.get(0)?, chapitre: r.get(1)?, genre: r.get(2)?, etat_fsrs: r.get(3)? })
        })?
        .collect::<rusqlite::Result<Vec<_>>>()?;
    // « localtime » : SQLite utilise le fuseau horaire de Windows.
    let jours = conn
        .prepare(
            "SELECT date(date / 1000, 'unixepoch', 'localtime') AS j, count(*), sum(correcte), sum(duree_ms)
             FROM revisions GROUP BY j ORDER BY j DESC LIMIT 365",
        )?
        .query_map([], |r| Ok(Jour { jour: r.get(0)?, revisions: r.get(1)?, justes: r.get(2)?, duree_ms: r.get(3)? }))?
        .collect::<rusqlite::Result<Vec<_>>>()?;
    let faibles = conn
        .prepare(&format!(
            "SELECT * FROM (
               SELECT c.id, c.genre, c.question, c.reponse, c.matiere, c.chapitre, c.note_chemin, c.section,
                      (SELECT count(*) FROM erreurs e WHERE e.carte_id = c.id AND e.comprise = 0) AS err,
                      coalesce(json_extract(c.etat_fsrs, '$.lapses'), 0) AS oublis,
                      {SCORE_FAIBLESSE} AS score
               FROM cartes c WHERE c.active = 1 AND c.suspendue = 0 AND c.etat_fsrs IS NOT NULL
             ) WHERE score > 0 ORDER BY score DESC LIMIT 8"
        ))?
        .query_map([], |r| {
            Ok(NotionFaible {
                carte_id: r.get(0)?,
                genre: r.get(1)?,
                question: r.get(2)?,
                reponse: r.get(3)?,
                matiere: r.get(4)?,
                chapitre: r.get(5)?,
                note_chemin: r.get(6)?,
                section: r.get(7)?,
                erreurs: r.get(8)?,
                oublis: r.get(9)?,
            })
        })?
        .collect::<rusqlite::Result<Vec<_>>>()?;
    Ok(TableauDeBord { cartes, jours, faibles })
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct NouvelleRevision {
    pub carte_id: i64,
    /// 1 à revoir, 2 difficile, 3 bien, 4 facile.
    pub note: i64,
    /// "sur", "hesitant" ou "hasard".
    pub confiance: String,
    pub correcte: bool,
    pub reponse_donnee: Option<String>,
    pub duree_ms: i64,
    /// Nouvel état FSRS (JSON) calculé par l'interface.
    pub etat_fsrs: String,
    pub echeance: i64,
}

/// Enregistre une réponse ; une réponse fausse va dans le carnet d'erreurs avec l'extrait de la note.
pub fn enregistrer(conn: &mut Connection, r: &NouvelleRevision, maintenant: i64) -> Result<(), String> {
    if !(1..=4).contains(&r.note) {
        return Err("Note invalide (1 à 4).".into());
    }
    if !["sur", "hesitant", "hasard"].contains(&r.confiance.as_str()) {
        return Err("Indice de confiance invalide.".into());
    }
    let tx = conn.transaction().map_err(|e| e.to_string())?;
    tx.execute(
        "INSERT INTO revisions(carte_id, date, note, confiance, correcte, reponse_donnee, duree_ms)
         VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7)",
        params![r.carte_id, maintenant, r.note, r.confiance, r.correcte, r.reponse_donnee, r.duree_ms.max(0)],
    )
    .map_err(|e| e.to_string())?;
    let modifiees = tx
        .execute(
            "UPDATE cartes SET etat_fsrs = ?2, echeance = ?3, introduite_le = coalesce(introduite_le, ?4) WHERE id = ?1",
            params![r.carte_id, r.etat_fsrs, r.echeance, maintenant],
        )
        .map_err(|e| e.to_string())?;
    if modifiees == 0 {
        return Err("Carte introuvable.".into());
    }
    if !r.correcte {
        tx.execute(
            "INSERT INTO erreurs(carte_id, date, genre, matiere, chapitre, note_chemin, section,
                                 question, reponse_attendue, reponse_donnee, extrait)
             SELECT id, ?2, genre, matiere, chapitre, note_chemin, section, question, reponse, ?3, extrait
             FROM cartes WHERE id = ?1",
            params![r.carte_id, maintenant, r.reponse_donnee],
        )
        .map_err(|e| e.to_string())?;
    }
    tx.commit().map_err(|e| e.to_string())
}

/// « Carte fausse / inutile » : elle ne sera plus proposée.
pub fn suspendre(conn: &Connection, carte_id: i64) -> rusqlite::Result<bool> {
    Ok(conn.execute("UPDATE cartes SET suspendue = 1 WHERE id = ?1", [carte_id])? == 1)
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ComptesMatiere {
    pub matiere: String,
    pub dues: i64,
    pub nouvelles: i64,
    pub total: i64,
}

/// Cartes d'un chapitre (choix des chapitres d'une séance ciblée).
#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ComptesChapitre {
    /// Absente pour les notes sans matière.
    pub matiere: Option<String>,
    pub chapitre: Option<String>,
    pub dues: i64,
    pub nouvelles: i64,
    pub total: i64,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ResumeGroupe {
    pub groupe: Groupe,
    pub dues: i64,
    pub nouvelles_disponibles: i64,
    /// Nouvelles cartes encore autorisées aujourd'hui pour ce groupe.
    pub nouvelles_restantes: i64,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Resume {
    pub matieres: Vec<ComptesMatiere>,
    pub chapitres: Vec<ComptesChapitre>,
    pub groupes: Vec<ResumeGroupe>,
    pub par_genre: Vec<(String, i64)>,
    /// Pour estimer la durée d'une séance.
    pub duree_moyenne_ms: i64,
}

/// `exclues` : matières laissées de côté dans les séances du jour (les compteurs des groupes n'en tiennent pas compte).
pub fn resume(
    conn: &Connection,
    maintenant: i64,
    debut_jour: i64,
    quotas: Quotas,
    exclues: &[String],
) -> rusqlite::Result<Resume> {
    let mut req = conn.prepare(
        "SELECT coalesce(matiere, 'Hors matière'),
                sum(echeance IS NOT NULL AND echeance <= ?1), sum(etat_fsrs IS NULL), count(*)
         FROM cartes WHERE active = 1 AND suspendue = 0 GROUP BY 1 ORDER BY 1",
    )?;
    let matieres = req
        .query_map([maintenant], |r| {
            Ok(ComptesMatiere { matiere: r.get(0)?, dues: r.get(1)?, nouvelles: r.get(2)?, total: r.get(3)? })
        })?
        .collect::<rusqlite::Result<Vec<_>>>()?;
    let mut req = conn.prepare(
        "SELECT matiere, chapitre, sum(echeance IS NOT NULL AND echeance <= ?1), sum(etat_fsrs IS NULL), count(*)
         FROM cartes WHERE active = 1 AND suspendue = 0 GROUP BY matiere, chapitre
         ORDER BY matiere IS NULL, matiere, chapitre IS NOT NULL, chapitre",
    )?;
    let chapitres = req
        .query_map([maintenant], |r| {
            Ok(ComptesChapitre {
                matiere: r.get(0)?,
                chapitre: r.get(1)?,
                dues: r.get(2)?,
                nouvelles: r.get(3)?,
                total: r.get(4)?,
            })
        })?
        .collect::<rusqlite::Result<Vec<_>>>()?;
    let mut req =
        conn.prepare("SELECT genre, count(*) FROM cartes WHERE active = 1 AND suspendue = 0 GROUP BY genre ORDER BY genre")?;
    let par_genre = req.query_map([], |r| Ok((r.get(0)?, r.get(1)?)))?.collect::<rusqlite::Result<Vec<_>>>()?;
    let mut groupes = Vec::new();
    for g in Groupe::TOUS {
        let mut p = vec![rusqlite::types::Value::Integer(maintenant)];
        let sans_exclues = condition_exclues(exclues, &mut p);
        let (dues, nouvelles): (i64, i64) = conn.query_row(
            &format!(
                "SELECT coalesce(sum(c.echeance IS NOT NULL AND c.echeance <= ?1), 0), coalesce(sum(c.etat_fsrs IS NULL), 0)
                 FROM cartes c WHERE c.active = 1 AND c.suspendue = 0 AND {}{sans_exclues}",
                g.condition()
            ),
            rusqlite::params_from_iter(p),
            |r| Ok((r.get(0)?, r.get(1)?)),
        )?;
        let deja = nouvelles_du_jour(conn, debut_jour, g)?;
        groupes.push(ResumeGroupe {
            groupe: g,
            dues,
            nouvelles_disponibles: nouvelles,
            nouvelles_restantes: quotas.de(g).saturating_sub(deja) as i64,
        });
    }
    Ok(Resume { matieres, chapitres, groupes, par_genre, duree_moyenne_ms: duree_moyenne_ms(conn)? })
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Erreur {
    pub id: i64,
    pub carte_id: Option<i64>,
    pub date: i64,
    pub genre: String,
    pub matiere: Option<String>,
    pub chapitre: Option<String>,
    pub note_chemin: String,
    pub titre_note: String,
    pub section: String,
    pub question: String,
    pub reponse_attendue: String,
    pub reponse_donnee: Option<String>,
    pub extrait: String,
    pub comprise: bool,
    /// Nombre total d'erreurs sur cette carte.
    pub fois: i64,
}

/// Carnet d'erreurs : la dernière erreur de chaque carte, de la plus récente à la plus ancienne.
pub fn erreurs(conn: &Connection, matiere: Option<&str>, avec_comprises: bool) -> rusqlite::Result<Vec<Erreur>> {
    let mut req = conn.prepare(
        "SELECT e.id, e.carte_id, e.date, e.genre, e.matiere, e.chapitre, e.note_chemin,
                coalesce(n.titre, e.note_chemin), e.section, e.question, e.reponse_attendue,
                e.reponse_donnee, e.extrait, e.comprise,
                (SELECT count(*) FROM erreurs e2 WHERE e2.carte_id = e.carte_id)
         FROM erreurs e LEFT JOIN notes n ON n.chemin = e.note_chemin
         WHERE (?1 IS NULL OR e.matiere = ?1) AND (?2 OR e.comprise = 0)
           AND e.id = (SELECT max(e3.id) FROM erreurs e3 WHERE e3.carte_id IS e.carte_id
                       AND (e.carte_id IS NOT NULL OR e3.id = e.id))
         ORDER BY e.date DESC",
    )?;
    let lignes = req.query_map(params![matiere, avec_comprises], |r| {
        Ok(Erreur {
            id: r.get(0)?,
            carte_id: r.get(1)?,
            date: r.get(2)?,
            genre: r.get(3)?,
            matiere: r.get(4)?,
            chapitre: r.get(5)?,
            note_chemin: r.get(6)?,
            titre_note: r.get(7)?,
            section: r.get(8)?,
            question: r.get(9)?,
            reponse_attendue: r.get(10)?,
            reponse_donnee: r.get(11)?,
            extrait: r.get(12)?,
            comprise: r.get(13)?,
            fois: r.get(14)?,
        })
    })?;
    lignes.collect()
}

/// « J'ai compris » : toutes les erreurs de la même carte sont marquées.
pub fn marquer_comprise(conn: &Connection, erreur_id: i64, comprise: bool) -> rusqlite::Result<usize> {
    conn.execute(
        "UPDATE erreurs SET comprise = ?2
         WHERE id = ?1 OR (carte_id IS NOT NULL AND carte_id = (SELECT carte_id FROM erreurs WHERE id = ?1))",
        params![erreur_id, comprise],
    )
}

/// Les cartes demandées (ex. « retravailler mes erreurs »), qu'elles soient dues ou non.
pub fn cartes_choisies(conn: &Connection, ids: &[i64]) -> rusqlite::Result<Vec<CarteSeance>> {
    let mut cartes = Vec::new();
    for id in ids {
        if let Some(c) = carte(conn, *id)? {
            cartes.push(c);
        }
    }
    Ok(cartes)
}

/// Une carte précise (pour l'afficher hors séance).
pub fn carte(conn: &Connection, id: i64) -> rusqlite::Result<Option<CarteSeance>> {
    conn.query_row(
        &format!(
            "SELECT {COLONNES} FROM cartes c LEFT JOIN notes n ON n.chemin = c.note_chemin
             WHERE c.id = ?1 AND c.active = 1 AND c.suspendue = 0"
        ),
        [id],
        lire_carte,
    )
    .optional()
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{cartes, db, indexer};
    use std::path::Path;

    fn base() -> Connection {
        let mut conn = db::ouvrir_en_memoire().unwrap();
        let racine = Path::new(env!("CARGO_MANIFEST_DIR")).join("tests").join("vault_exemple");
        indexer::indexer(&mut conn, &racine).unwrap();
        cartes::synchroniser(&mut conn, 0).unwrap();
        conn
    }

    fn filtre() -> FiltreSeance {
        FiltreSeance { debut_jour: 1_000, ..Default::default() }
    }

    fn q(n: usize) -> Quotas {
        Quotas { langues: n, autres: n }
    }

    #[test]
    fn dates_iso() {
        assert_eq!(date_iso(0), "1970-01-01T00:00:00.000Z");
        assert_eq!(date_iso(1_700_000_000_123), "2023-11-14T22:13:20.123Z");
        assert_eq!(date_iso(951_782_400_000), "2000-02-29T00:00:00.000Z");
    }

    #[test]
    fn cartes_marquees_apprises() {
        let mut conn = base();
        let maintenant = 1_800_000_000_000;
        let total: i64 = conn.query_row("SELECT count(*) FROM cartes WHERE active = 1", [], |r| r.get(0)).unwrap();
        assert_eq!(marquer_apprises(&mut conn, maintenant).unwrap() as i64, total);
        // Plus aucune nouvelle carte, rien de dû aujourd'hui, tout est réparti sur 30 jours.
        let s = preparer_seance(&conn, &filtre(), maintenant, q(50)).unwrap();
        assert_eq!((s.dues.len(), s.nouvelles.len()), (0, 0));
        let s = preparer_seance(&conn, &filtre(), maintenant + 30 * JOUR_MS, q(50)).unwrap();
        assert_eq!(s.dues.len() as i64, total);
        let etat: Value = serde_json::from_str(&s.dues[0].etat_fsrs.clone().unwrap()).unwrap();
        assert_eq!(etat["state"], 2);
        // Le quota de nouvelles cartes du jour n'est pas entamé.
        assert_eq!(nouvelles_du_jour(&conn, maintenant - 1000, Groupe::Langues).unwrap(), 0);
        // Une deuxième fois : rien à faire.
        assert_eq!(marquer_apprises(&mut conn, maintenant).unwrap(), 0);
    }

    #[test]
    fn chapitres_planifies_en_avance() {
        let mut conn = base();
        let maintenant = 1_800_000_000_000;
        marquer_apprises(&mut conn, maintenant).unwrap();
        let (matiere, chapitre): (String, Option<String>) = conn
            .query_row("SELECT matiere, chapitre FROM cartes WHERE genre <> 'vocabulaire' AND matiere IS NOT NULL LIMIT 1", [], |r| {
                Ok((r.get(0)?, r.get(1)?))
            })
            .unwrap();
        let du_chapitre: i64 = conn
            .query_row(
                "SELECT count(*) FROM cartes WHERE active = 1 AND matiere = ?1 AND chapitre IS ?2 AND genre <> 'vocabulaire'",
                params![matiere, chapitre],
                |r| r.get(0),
            )
            .unwrap();
        let planifie = FiltreSeance {
            groupe: Some(Groupe::Autres),
            debut_jour: maintenant - 1000,
            planifies: vec![ChapitrePlanifie { matiere: Some(matiere.clone()), chapitre: chapitre.clone() }],
            ..filtre()
        };
        // Rien n'est dû, mais tout le chapitre planifié est proposé (révision anticipée).
        let s = preparer_seance(&conn, &planifie, maintenant, q(0)).unwrap();
        assert!(s.dues.is_empty());
        assert_eq!(s.planifiees.len() as i64, du_chapitre);
        assert!(s.planifiees.iter().all(|c| c.matiere.as_deref() == Some(&matiere) && c.chapitre == chapitre));
        assert!(s.planifiees.windows(2).all(|w| w[0].echeance <= w[1].echeance));
        // Une carte révisée aujourd'hui ne revient pas.
        let r = NouvelleRevision {
            carte_id: s.planifiees[0].id,
            note: 3,
            confiance: "sur".into(),
            correcte: true,
            reponse_donnee: None,
            duree_ms: 1000,
            etat_fsrs: "{}".into(),
            echeance: maintenant + 10 * JOUR_MS,
        };
        enregistrer(&mut conn, &r, maintenant).unwrap();
        let s = preparer_seance(&conn, &planifie, maintenant + 1, q(0)).unwrap();
        assert_eq!(s.planifiees.len() as i64, du_chapitre - 1);
        // Sans planning : rien.
        assert!(preparer_seance(&conn, &filtre(), maintenant, q(0)).unwrap().planifiees.is_empty());
    }

    #[test]
    fn seance_ciblee_sur_des_chapitres() {
        let mut conn = base();
        let maintenant = 1_800_000_000_000;
        marquer_apprises(&mut conn, maintenant).unwrap();
        let r = resume(&conn, maintenant, maintenant - 1000, q(0), &[]).unwrap();
        assert!(r.chapitres.len() >= 2, "le vault d'exemple a plusieurs chapitres");
        let total: i64 = conn.query_row("SELECT count(*) FROM cartes WHERE active = 1", [], |r| r.get(0)).unwrap();
        assert_eq!(r.chapitres.iter().map(|c| c.total).sum::<i64>(), total);
        // Deux chapitres choisis (de matières différentes si possible) : toutes leurs cartes, même pas dues.
        let choisis: Vec<ChapitrePlanifie> = r
            .chapitres
            .iter()
            .take(2)
            .map(|c| ChapitrePlanifie { matiere: c.matiere.clone(), chapitre: c.chapitre.clone() })
            .collect();
        let attendu: i64 = r.chapitres.iter().take(2).map(|c| c.total).sum();
        let f = FiltreSeance {
            debut_jour: maintenant - 1000,
            chapitres: choisis.clone(),
            planifies: choisis.clone(),
            ..filtre()
        };
        let s = preparer_seance(&conn, &f, maintenant, q(0)).unwrap();
        assert_eq!(s.planifiees.len() as i64, attendu);
        let dans_le_choix = |c: &CarteSeance| choisis.iter().any(|x| x.matiere == c.matiere && x.chapitre == c.chapitre);
        assert!(s.planifiees.iter().chain(&s.dues).chain(&s.nouvelles).all(dans_le_choix));
    }

    #[test]
    fn matieres_exclues_des_seances_du_jour() {
        let conn = base();
        let r = resume(&conn, 2_000, 1_000, q(500), &[]).unwrap();
        let jour = |exclues: Vec<String>| FiltreSeance { groupe: Some(Groupe::Autres), exclues, ..filtre() };
        let avant = preparer_seance(&conn, &jour(vec![]), 2_000, q(500)).unwrap().nouvelles;
        // Une matière présente dans la séance « Autres matières ».
        let exclue = avant.iter().find_map(|c| c.matiere.clone()).unwrap();
        let apres = preparer_seance(&conn, &jour(vec![exclue.clone()]), 2_000, q(500)).unwrap().nouvelles;
        assert!(apres.len() < avant.len());
        assert!(apres.iter().all(|c| c.matiere.as_deref() != Some(&exclue)));
        // « Hors matière » vise les cartes sans matière.
        let sans = preparer_seance(&conn, &jour(vec![HORS_MATIERE.into()]), 2_000, q(500)).unwrap().nouvelles;
        assert!(sans.iter().all(|c| c.matiere.is_some()));
        // Les compteurs des séances du jour suivent l'exclusion.
        let r2 = resume(&conn, 2_000, 1_000, q(500), &[exclue.clone()]).unwrap();
        let nouvelles = |r: &Resume| r.groupes.iter().map(|g| g.nouvelles_disponibles).sum::<i64>();
        assert!(nouvelles(&r2) < nouvelles(&r));
    }

    #[test]
    fn quotas_separes_langues_et_autres() {
        let conn = base();
        let langues = FiltreSeance { groupe: Some(Groupe::Langues), ..filtre() };
        let autres = FiltreSeance { groupe: Some(Groupe::Autres), ..filtre() };
        let quotas = Quotas { langues: 2, autres: 3 };
        let l = preparer_seance(&conn, &langues, 2_000, quotas).unwrap().nouvelles;
        let a = preparer_seance(&conn, &autres, 2_000, quotas).unwrap().nouvelles;
        assert_eq!(l.len(), 2);
        assert!(l.iter().all(|c| c.genre == "vocabulaire"));
        assert_eq!(a.len(), 3);
        assert!(a.iter().all(|c| c.genre != "vocabulaire"));
        // Sans groupe : les deux quotas s'additionnent.
        assert_eq!(preparer_seance(&conn, &filtre(), 2_000, quotas).unwrap().nouvelles.len(), 5);
        let r = resume(&conn, 2_000, 1_000, quotas, &[]).unwrap();
        assert_eq!(r.groupes.iter().map(|g| g.nouvelles_restantes).collect::<Vec<_>>(), vec![2, 3]);
    }

    #[test]
    fn quota_de_nouvelles_cartes() {
        let mut conn = base();
        let filtre = || FiltreSeance { groupe: Some(Groupe::Autres), ..filtre() };
        let s = preparer_seance(&conn, &filtre(), 2_000, q(1)).unwrap();
        assert_eq!((s.dues.len(), s.nouvelles.len()), (0, 1));
        let id = s.nouvelles[0].id;
        let r = NouvelleRevision {
            carte_id: id,
            note: 3,
            confiance: "sur".into(),
            correcte: true,
            reponse_donnee: None,
            duree_ms: 1200,
            etat_fsrs: "{}".into(),
            echeance: 5_000,
        };
        enregistrer(&mut conn, &r, 2_000).unwrap();
        // Quota du jour atteint ; la carte n'est pas encore due.
        let s = preparer_seance(&conn, &filtre(), 3_000, q(1)).unwrap();
        assert_eq!((s.dues.len(), s.nouvelles.len()), (0, 0));
        // Plus tard, elle est due.
        let s = preparer_seance(&conn, &filtre(), 6_000, q(1)).unwrap();
        assert_eq!(s.dues.iter().map(|c| c.id).collect::<Vec<_>>(), vec![id]);
    }

    #[test]
    fn erreur_dans_le_carnet() {
        let mut conn = base();
        let id = preparer_seance(&conn, &filtre(), 2_000, q(5)).unwrap().nouvelles[0].id;
        let r = NouvelleRevision {
            carte_id: id,
            note: 1,
            confiance: "hesitant".into(),
            correcte: false,
            reponse_donnee: Some("mauvaise".into()),
            duree_ms: 900,
            etat_fsrs: "{}".into(),
            echeance: 2_500,
        };
        enregistrer(&mut conn, &r, 2_000).unwrap();
        let (attendue, donnee, extrait): (String, String, String) = conn
            .query_row("SELECT reponse_attendue, reponse_donnee, extrait FROM erreurs", [], |r| {
                Ok((r.get(0)?, r.get(1)?, r.get(2)?))
            })
            .unwrap();
        assert!(!attendue.is_empty() && !extrait.is_empty());
        assert_eq!(donnee, "mauvaise");
    }

    #[test]
    fn carnet_regroupe_par_carte_et_comprise() {
        let mut conn = base();
        let ids: Vec<i64> = preparer_seance(&conn, &filtre(), 2_000, q(5)).unwrap().nouvelles.iter().map(|c| c.id).collect();
        let faux = |carte_id, date| NouvelleRevision {
            carte_id,
            note: 1,
            confiance: "sur".into(),
            correcte: false,
            reponse_donnee: Some(format!("essai {date}")),
            duree_ms: 1,
            etat_fsrs: "{}".into(),
            echeance: date,
        };
        enregistrer(&mut conn, &faux(ids[0], 10), 10).unwrap();
        enregistrer(&mut conn, &faux(ids[0], 20), 20).unwrap();
        enregistrer(&mut conn, &faux(ids[1], 30), 30).unwrap();

        let liste = erreurs(&conn, None, false).unwrap();
        assert_eq!(liste.len(), 2); // une ligne par carte
        assert_eq!((liste[0].carte_id, liste[0].fois), (Some(ids[1]), 1));
        assert_eq!((liste[1].carte_id, liste[1].fois), (Some(ids[0]), 2));
        assert_eq!(liste[1].reponse_donnee.as_deref(), Some("essai 20")); // la plus récente

        // Les cartes ratées (pas encore dues) sont des points faibles, celle ratée 2 fois en premier.
        let s = preparer_seance(&conn, &filtre(), 5, q(0)).unwrap();
        assert_eq!(s.faibles.iter().map(|c| c.id).collect::<Vec<_>>(), vec![ids[0], ids[1]]);
        let t = tableau_de_bord(&conn).unwrap();
        assert_eq!(t.faibles[0].carte_id, ids[0]);
        assert_eq!(t.faibles[0].erreurs, 2);
        assert_eq!(t.jours.iter().map(|j| j.revisions).sum::<i64>(), 3);

        assert_eq!(marquer_comprise(&conn, liste[1].id, true).unwrap(), 2);
        assert_eq!(erreurs(&conn, None, false).unwrap().len(), 1);
        assert_eq!(erreurs(&conn, None, true).unwrap().len(), 2);
        assert_eq!(cartes_choisies(&conn, &[ids[0], 999]).unwrap().len(), 1);
    }

    #[test]
    fn filtres_et_suspension() {
        let conn = base();
        let f = FiltreSeance { matiere: Some("Allemand".into()), genres: vec!["vocabulaire".into()], ..filtre() };
        let s = preparer_seance(&conn, &f, 2_000, q(50)).unwrap();
        assert_eq!(s.nouvelles.len(), 4); // 2 mots × 2 sens
        assert!(suspendre(&conn, s.nouvelles[0].id).unwrap());
        assert_eq!(preparer_seance(&conn, &f, 2_000, q(50)).unwrap().nouvelles.len(), 3);
        let f = FiltreSeance { matiere: Some("Spé SVT".into()), genres: vec!["vocabulaire".into()], ..filtre() };
        assert!(preparer_seance(&conn, &f, 2_000, q(50)).unwrap().nouvelles.is_empty());
    }
}
