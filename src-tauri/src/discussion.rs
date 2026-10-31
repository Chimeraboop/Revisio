//! Discussion avec l'IA : l'élève pose des questions sur un concept, l'IA explique en s'appuyant
//! d'abord sur les notes choisies (sources) et signale ce qui n'y est pas.
//!
//! - Les conversations et leurs messages sont enregistrés : on les reprend après avoir fermé l'app.
//! - La réponse arrive au fil de l'eau (évènement « discussion-flux »).
//! - Un schéma (bloc ```mermaid) ou un graphique (bloc ```graphique) écrit par l'IA devient une
//!   « création » proposée : elle n'est gardée dans les créations de l'élève que s'il l'accepte.

use std::sync::Mutex;
use std::time::Instant;

use regex::Regex;
use rusqlite::{params, Connection, OptionalExtension};
use serde::{Deserialize, Serialize};
use tauri::{AppHandle, Emitter, State};

use crate::ia::ancrage::{self, Extrait};
use crate::ia::gemini::{Partie, Requete, Tour};
use crate::{maintenant_ms, Etat};

/// Titre d'une conversation tant que l'élève n'a rien demandé.
pub const TITRE_PAR_DEFAUT: &str = "Nouvelle conversation";
/// Messages précédents renvoyés à l'IA (au-delà, elle ne voit plus le début de la conversation).
const MESSAGES_GARDES: usize = 16;
/// Longueur maximale d'un ancien message renvoyé à l'IA.
const CARACTERES_PAR_MESSAGE: usize = 4_000;

/// Une source de la conversation : une note précise (`note` = chemin), ou un chapitre d'une matière
/// (`chapitre` absent : notes générales de la matière ; `matiere` absente : notes hors matière).
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct Source {
    #[serde(default)]
    pub matiere: Option<String>,
    #[serde(default)]
    pub chapitre: Option<String>,
    #[serde(default)]
    pub note: Option<String>,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Conversation {
    pub id: i64,
    pub titre: String,
    pub creee: i64,
    pub maj: i64,
    pub sources: Vec<Source>,
    pub messages: i64,
    /// Début de la dernière question (aperçu dans la liste).
    pub apercu: String,
}

/// Extrait cité par l'IA dans une réponse (pour rendre la citation cliquable).
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct ExtraitCite {
    pub id: String,
    pub chemin: String,
    pub titre: String,
    pub section: String,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Message {
    pub id: i64,
    pub ia: bool,
    pub texte: String,
    pub date: i64,
    pub modele: Option<String>,
    pub extraits: Vec<ExtraitCite>,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Creation {
    pub id: i64,
    pub date: i64,
    /// « diagramme » (Mermaid) ou « graphique » (JSON).
    pub genre: String,
    pub titre: String,
    pub contenu: String,
    pub conversation_id: Option<i64>,
    pub conversation_titre: Option<String>,
    pub message_id: Option<i64>,
    /// Rang du bloc dans le message (0 pour le premier).
    pub indice: i64,
    /// « proposee », « acceptee » ou « refusee ».
    pub statut: String,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ConversationComplete {
    pub conversation: Conversation,
    pub messages: Vec<Message>,
    pub creations: Vec<Creation>,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ReponseIa {
    pub message: Message,
    pub creations: Vec<Creation>,
}

#[derive(Clone, Serialize)]
#[serde(rename_all = "camelCase")]
struct Flux<'a> {
    conversation_id: i64,
    texte: &'a str,
}

// ---------- Base de données ----------

fn lire_conversation(r: &rusqlite::Row) -> rusqlite::Result<Conversation> {
    Ok(Conversation {
        id: r.get(0)?,
        titre: r.get(1)?,
        creee: r.get(2)?,
        maj: r.get(3)?,
        sources: serde_json::from_str(&r.get::<_, String>(4)?).unwrap_or_default(),
        messages: r.get(5)?,
        apercu: r.get::<_, Option<String>>(6)?.unwrap_or_default(),
    })
}

const SELECT_CONVERSATION: &str = "SELECT c.id, c.titre, c.creee, c.maj, c.sources,
        (SELECT count(*) FROM messages_conversation m WHERE m.conversation_id = c.id),
        (SELECT substr(m.texte, 1, 120) FROM messages_conversation m WHERE m.conversation_id = c.id AND m.ia = 0
           ORDER BY m.id DESC LIMIT 1)
     FROM conversations c";

pub fn conversations(conn: &Connection) -> rusqlite::Result<Vec<Conversation>> {
    conn.prepare(&format!("{SELECT_CONVERSATION} ORDER BY c.maj DESC"))?.query_map([], lire_conversation)?.collect()
}

fn conversation(conn: &Connection, id: i64) -> rusqlite::Result<Option<Conversation>> {
    conn.query_row(&format!("{SELECT_CONVERSATION} WHERE c.id = ?1"), [id], lire_conversation).optional()
}

pub fn creer(conn: &Connection, sources: &[Source], maintenant: i64) -> rusqlite::Result<i64> {
    conn.execute(
        "INSERT INTO conversations(titre, creee, maj, sources) VALUES (?1, ?2, ?2, ?3)",
        params![TITRE_PAR_DEFAUT, maintenant, serde_json::to_string(sources).unwrap_or_else(|_| "[]".into())],
    )?;
    Ok(conn.last_insert_rowid())
}

/// Supprime la conversation et ses messages. Les créations acceptées restent (sans lien vers la conversation) ;
/// les propositions jamais acceptées disparaissent avec elle.
pub fn supprimer(conn: &mut Connection, id: i64) -> rusqlite::Result<()> {
    let tx = conn.transaction()?;
    tx.execute("DELETE FROM creations_ia WHERE conversation_id = ?1 AND statut <> 'acceptee'", [id])?;
    tx.execute("UPDATE creations_ia SET conversation_id = NULL, message_id = NULL WHERE conversation_id = ?1", [id])?;
    tx.execute("DELETE FROM messages_conversation WHERE conversation_id = ?1", [id])?;
    tx.execute("DELETE FROM conversations WHERE id = ?1", [id])?;
    tx.commit()
}

fn lire_message(r: &rusqlite::Row) -> rusqlite::Result<Message> {
    Ok(Message {
        id: r.get(0)?,
        ia: r.get(1)?,
        texte: r.get(2)?,
        date: r.get(3)?,
        modele: r.get(4)?,
        extraits: serde_json::from_str(&r.get::<_, String>(5)?).unwrap_or_default(),
    })
}

pub fn messages(conn: &Connection, conversation_id: i64) -> rusqlite::Result<Vec<Message>> {
    conn.prepare(
        "SELECT id, ia, texte, date, modele, extraits FROM messages_conversation WHERE conversation_id = ?1 ORDER BY id",
    )?
    .query_map([conversation_id], lire_message)?
    .collect()
}

fn ajouter_message(
    conn: &Connection,
    conversation_id: i64,
    ia: bool,
    texte: &str,
    modele: Option<&str>,
    extraits: &[ExtraitCite],
    maintenant: i64,
) -> rusqlite::Result<i64> {
    conn.execute(
        "INSERT INTO messages_conversation(conversation_id, ia, texte, date, modele, extraits) VALUES (?1, ?2, ?3, ?4, ?5, ?6)",
        params![conversation_id, ia, texte, maintenant, modele, serde_json::to_string(extraits).unwrap_or_else(|_| "[]".into())],
    )?;
    let id = conn.last_insert_rowid();
    conn.execute("UPDATE conversations SET maj = ?2 WHERE id = ?1", params![conversation_id, maintenant])?;
    // Première question : elle donne son titre à la conversation (l'élève peut le changer).
    if !ia {
        conn.execute(
            "UPDATE conversations SET titre = ?2 WHERE id = ?1 AND titre = ?3",
            params![conversation_id, titre_depuis(texte), TITRE_PAR_DEFAUT],
        )?;
    }
    Ok(id)
}

/// Titre court tiré de la première question (60 caractères au plus, coupé sur un mot).
pub fn titre_depuis(question: &str) -> String {
    let une_ligne = question.split_whitespace().collect::<Vec<_>>().join(" ");
    if une_ligne.chars().count() <= 60 {
        return une_ligne;
    }
    let debut: String = une_ligne.chars().take(60).collect();
    let coupe = debut.rfind(' ').filter(|&i| i > 30).unwrap_or(debut.len());
    format!("{}…", debut[..coupe].trim_end_matches([',', ';', ':', ' ']))
}

fn lire_creation(r: &rusqlite::Row) -> rusqlite::Result<Creation> {
    Ok(Creation {
        id: r.get(0)?,
        date: r.get(1)?,
        genre: r.get(2)?,
        titre: r.get(3)?,
        contenu: r.get(4)?,
        conversation_id: r.get(5)?,
        conversation_titre: r.get(6)?,
        message_id: r.get(7)?,
        indice: r.get(8)?,
        statut: r.get(9)?,
    })
}

const SELECT_CREATION: &str = "SELECT k.id, k.date, k.genre, k.titre, k.contenu, k.conversation_id, c.titre, k.message_id,
        k.indice, k.statut
     FROM creations_ia k LEFT JOIN conversations c ON c.id = k.conversation_id";

/// Créations acceptées (les plus récentes d'abord) : l'onglet « Créations ».
pub fn creations_acceptees(conn: &Connection) -> rusqlite::Result<Vec<Creation>> {
    conn.prepare(&format!("{SELECT_CREATION} WHERE k.statut = 'acceptee' ORDER BY k.date DESC"))?
        .query_map([], lire_creation)?
        .collect()
}

fn creations_de(conn: &Connection, conversation_id: i64) -> rusqlite::Result<Vec<Creation>> {
    conn.prepare(&format!("{SELECT_CREATION} WHERE k.conversation_id = ?1 ORDER BY k.message_id, k.indice"))?
        .query_map([conversation_id], lire_creation)?
        .collect()
}

pub fn creation(conn: &Connection, id: i64) -> rusqlite::Result<Option<Creation>> {
    conn.query_row(&format!("{SELECT_CREATION} WHERE k.id = ?1"), [id], lire_creation).optional()
}

/// Bloc ```mermaid ou ```graphique trouvé dans une réponse : (genre, titre, contenu).
pub fn blocs_crees(texte: &str) -> Vec<(String, String, String)> {
    let motif = Regex::new(r"(?s)```(mermaid|graphique)[ \t]*\r?\n(.*?)```").unwrap();
    motif
        .captures_iter(texte)
        .enumerate()
        .map(|(i, c)| {
            let genre = if &c[1] == "mermaid" { "diagramme" } else { "graphique" };
            let contenu = c[2].trim().to_string();
            let titre = titre_du_bloc(genre, &contenu).unwrap_or_else(|| {
                format!("{} {}", if genre == "diagramme" { "Schéma" } else { "Graphique" }, i + 1)
            });
            (genre.to_string(), titre, contenu)
        })
        .collect()
}

/// Titre écrit par l'IA : `%% Titre : …` en tête d'un diagramme, champ `titre` d'un graphique.
fn titre_du_bloc(genre: &str, contenu: &str) -> Option<String> {
    let titre = if genre == "diagramme" {
        contenu.lines().find_map(|l| {
            let l = l.trim().strip_prefix("%%")?.trim();
            let reste = l.strip_prefix("Titre").or_else(|| l.strip_prefix("titre"))?;
            Some(reste.trim_start_matches([' ', ':']).trim().to_string())
        })
    } else {
        serde_json::from_str::<serde_json::Value>(contenu).ok()?["titre"].as_str().map(str::to_owned)
    };
    titre.filter(|t| !t.is_empty())
}

// ---------- Ce qu'on envoie à l'IA ----------

pub const CONSIGNE: &str = "Tu es un professeur de lycée (classe de Première, France), patient et clair, qui aide un élève à comprendre son cours. Tu réponds en français et tu le tutoies.

Tu reçois des extraits des notes de cours de l'élève, numérotés [E1], [E2]…
- Appuie-toi d'abord sur ces extraits. Juste après une idée qui en vient, cite son numéro entre crochets, par exemple [E3]. Ne cite jamais un numéro qui n'existe pas.
- Si tu ajoutes une explication, un exemple ou un fait qui n'est PAS dans les extraits, mets-le dans un encadré qui commence ainsi :
> [!hors-notes] Hors de tes notes
> ton texte ici (chaque ligne de l'encadré commence par « > »)
- Si une affirmation de l'élève ou d'un extrait te semble fausse, dis-le avec tact.
- Adapte la longueur à la question : courte pour une question simple, structurée (titres, listes) pour une explication. Pas de formule de politesse inutile.
- Formules mathématiques ou scientifiques en LaTeX : $…$ dans le texte, $$…$$ seules sur une ligne.

Visuels : quand un visuel aide VRAIMENT à comprendre (étapes d'un processus, cycle, classification, comparaison, chronologie, évolution d'une grandeur, fonction), tu peux en ajouter UN au plus par réponse. L'élève verra une proposition et choisira de la garder ou non. N'en mets pas si le texte suffit.
1. Diagramme : un bloc de code ```mermaid dont la première ligne est « %% Titre : … ». Types permis : flowchart TD ou LR, mindmap, timeline, sequenceDiagram, stateDiagram-v2. Textes courts entre guillemets droits, par exemple A[\"Prophase\"] --> B[\"Métaphase\"]. Jamais de LaTeX, de parenthèses non mises entre guillemets ni de point-virgule dans les textes.
2. Graphique : un bloc de code ```graphique contenant UNIQUEMENT un objet JSON, de l'une de ces formes :
   {\"titre\": \"…\", \"type\": \"fonction\", \"axeX\": \"x\", \"axeY\": \"y\", \"xmin\": -3, \"xmax\": 3, \"fonctions\": [{\"nom\": \"f(x) = x²\", \"expression\": \"x^2\"}]}
   (expression avec x, + - * / ^, parenthèses, sqrt, exp, ln, sin, cos ; Révisio calcule les points lui-même)
   {\"titre\": \"…\", \"type\": \"courbes\", \"axeX\": \"…\", \"axeY\": \"…\", \"series\": [{\"nom\": \"…\", \"points\": [[0, 1], [1, 3]]}]}
   {\"titre\": \"…\", \"type\": \"barres\", \"axeY\": \"…\", \"categories\": [\"A\", \"B\"], \"series\": [{\"nom\": \"…\", \"valeurs\": [4, 7]}]}
   N'invente pas de données chiffrées : utilise celles des extraits ou une fonction exacte.";

/// Extraits de toutes les sources (sans doublon), numérotés une fois pour toutes dans l'ordre des sources.
pub fn extraits_des_sources(conn: &Connection, sources: &[Source]) -> rusqlite::Result<Vec<Extrait>> {
    let mut tous: Vec<Extrait> = Vec::new();
    for s in sources {
        let liste = match (&s.note, &s.matiere) {
            (Some(chemin), _) => ancrage::extraits_de_la_note(conn, chemin)?,
            (None, Some(m)) => ancrage::extraits_du_chapitre(conn, m, s.chapitre.as_deref())?,
            (None, None) => Vec::new(),
        };
        for e in liste {
            if !tous.iter().any(|x| x.note_chemin == e.note_chemin && x.section == e.section && x.texte == e.texte) {
                tous.push(e);
            }
        }
    }
    Ok(ancrage::renumeroter(tous, 1))
}

/// Les anciens messages renvoyés à l'IA (les plus récents, raccourcis si besoin).
fn historique(messages: &[Message]) -> Vec<Tour> {
    let debut = messages.len().saturating_sub(MESSAGES_GARDES);
    messages[debut..]
        .iter()
        .map(|m| {
            let texte: String = m.texte.chars().take(CARACTERES_PAR_MESSAGE).collect();
            Tour { ia: m.ia, texte }
        })
        .collect()
}

/// La requête pour répondre à `question`, avec les échanges précédents et les extraits utiles.
pub fn requete(modele: &str, precedents: &[Message], extraits: &[Extrait], question: &str) -> Requete {
    // La question précédente aide à garder les bons extraits (« et pourquoi ? »).
    let contexte = precedents.iter().rev().find(|m| !m.ia).map(|m| m.texte.as_str()).unwrap_or("");
    let choisis = ancrage::pour_la_question(extraits, &format!("{question} {contexte}"), ancrage::CARACTERES_PAR_APPEL);
    let notes = if choisis.is_empty() {
        "Aucune note n'a été choisie pour cette conversation : tout ce que tu expliques est donc « hors de tes notes ».".to_string()
    } else {
        format!("Extraits des notes de l'élève :\n\n{}", ancrage::texte_des_extraits(&choisis))
    };
    Requete {
        modele: modele.to_string(),
        systeme: CONSIGNE.into(),
        parties: vec![Partie::Texte(format!("{notes}\n\n---\nQuestion de l'élève :\n{question}"))],
        schema: None,
        temperature: Some(0.5),
        historique: historique(precedents),
    }
}

/// Les extraits réellement cités dans la réponse ([E3], [E3, E5]…), pour des citations cliquables.
pub fn extraits_cites(texte: &str, extraits: &[Extrait]) -> Vec<ExtraitCite> {
    let motif = Regex::new(r"\bE(\d+)\b").unwrap();
    let mut cites: Vec<ExtraitCite> = Vec::new();
    for c in motif.captures_iter(texte) {
        let id = format!("E{}", &c[1]);
        if cites.iter().any(|x| x.id == id) {
            continue;
        }
        if let Some(e) = extraits.iter().find(|e| e.id == id) {
            cites.push(ExtraitCite { id, chemin: e.note_chemin.clone(), titre: e.titre_note.clone(), section: e.section.clone() });
        }
    }
    cites
}

/// Répond à la dernière question de la conversation : appel à l'IA (au fil de l'eau), puis enregistrement
/// de la réponse et des créations proposées.
async fn repondre(app: &AppHandle, etat: &Etat, conversation_id: i64) -> Result<ReponseIa, String> {
    let (precedents, question, extraits) = {
        let conn = etat.db.lock().unwrap();
        let conv = conversation(&conn, conversation_id).map_err(|e| e.to_string())?.ok_or("Conversation introuvable.")?;
        let mut tous = messages(&conn, conversation_id).map_err(|e| e.to_string())?;
        let derniere = tous.pop().filter(|m| !m.ia).ok_or("Pas de question en attente de réponse.")?;
        let extraits = extraits_des_sources(&conn, &conv.sources).map_err(|e| e.to_string())?;
        (tous, derniere.texte, extraits)
    };
    let modele = etat.reglages.lock().unwrap().modele_ia.clone();
    let requete = requete(&modele, &precedents, &extraits, &question);
    // Au plus un évènement toutes les 60 ms : l'écran suit sans être submergé.
    let dernier_envoi: Mutex<Option<Instant>> = Mutex::new(None);
    let flux = |texte: &str| {
        let mut dernier = dernier_envoi.lock().unwrap();
        if dernier.is_some_and(|t| t.elapsed().as_millis() < 60) {
            return;
        }
        *dernier = Some(Instant::now());
        let _ = app.emit("discussion-flux", Flux { conversation_id, texte });
    };
    let local = etat.config_locale();
    let (reponse, modele) = etat
        .ia
        .appeler_texte_flux(&etat.db, "discussion", etat.limites_ia(), local.as_ref(), &requete, &flux)
        .await?;
    let texte = reponse.texte.trim().to_string();
    let cites = extraits_cites(&texte, &extraits);
    let conn = etat.db.lock().unwrap();
    let maintenant = maintenant_ms();
    let id = ajouter_message(&conn, conversation_id, true, &texte, Some(&modele), &cites, maintenant).map_err(|e| e.to_string())?;
    for (indice, (genre, titre, contenu)) in blocs_crees(&texte).into_iter().enumerate() {
        conn.execute(
            "INSERT INTO creations_ia(date, genre, titre, contenu, conversation_id, message_id, indice, statut)
             VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, 'proposee')",
            params![maintenant, genre, titre, contenu, conversation_id, id, indice as i64],
        )
        .map_err(|e| e.to_string())?;
    }
    let message = messages(&conn, conversation_id)
        .map_err(|e| e.to_string())?
        .into_iter()
        .find(|m| m.id == id)
        .ok_or("Réponse introuvable.")?;
    let creations = creations_de(&conn, conversation_id)
        .map_err(|e| e.to_string())?
        .into_iter()
        .filter(|c| c.message_id == Some(id))
        .collect();
    Ok(ReponseIa { message, creations })
}

// ---------- Commandes ----------

#[tauri::command]
pub fn liste_conversations(etat: State<Etat>) -> Result<Vec<Conversation>, String> {
    conversations(&etat.db.lock().unwrap()).map_err(|e| e.to_string())
}

#[tauri::command]
pub fn creer_conversation(etat: State<Etat>, sources: Vec<Source>) -> Result<Conversation, String> {
    let conn = etat.db.lock().unwrap();
    let id = creer(&conn, &sources, maintenant_ms()).map_err(|e| e.to_string())?;
    conversation(&conn, id).map_err(|e| e.to_string())?.ok_or_else(|| "Conversation introuvable.".into())
}

#[tauri::command]
pub fn ouvrir_conversation(etat: State<Etat>, id: i64) -> Result<ConversationComplete, String> {
    let conn = etat.db.lock().unwrap();
    let conversation = conversation(&conn, id).map_err(|e| e.to_string())?.ok_or("Conversation introuvable.")?;
    Ok(ConversationComplete {
        conversation,
        messages: messages(&conn, id).map_err(|e| e.to_string())?,
        creations: creations_de(&conn, id).map_err(|e| e.to_string())?,
    })
}

#[tauri::command]
pub fn renommer_conversation(etat: State<Etat>, id: i64, titre: String) -> Result<(), String> {
    let titre = titre.trim();
    if titre.is_empty() {
        return Err("Le titre ne peut pas être vide.".into());
    }
    let titre: String = titre.chars().take(120).collect();
    etat.db.lock().unwrap().execute("UPDATE conversations SET titre = ?2 WHERE id = ?1", params![id, titre]).map_err(|e| e.to_string())?;
    Ok(())
}

#[tauri::command]
pub fn supprimer_conversation(etat: State<Etat>, id: i64) -> Result<(), String> {
    supprimer(&mut etat.db.lock().unwrap(), id).map_err(|e| e.to_string())
}

#[tauri::command]
pub fn definir_sources_conversation(etat: State<Etat>, id: i64, sources: Vec<Source>) -> Result<(), String> {
    etat.db
        .lock()
        .unwrap()
        .execute(
            "UPDATE conversations SET sources = ?2 WHERE id = ?1",
            params![id, serde_json::to_string(&sources).map_err(|e| e.to_string())?],
        )
        .map_err(|e| e.to_string())?;
    Ok(())
}

/// Enregistre la question de l'élève puis fait répondre l'IA (interruptible avec `tache`).
/// La question reste enregistrée même si l'IA échoue : « Réessayer » appelle `relancer_reponse`.
#[tauri::command]
pub async fn envoyer_message(
    app: AppHandle,
    etat: State<'_, Etat>,
    conversation_id: i64,
    texte: String,
    tache: Option<String>,
) -> Result<ReponseIa, String> {
    let texte = texte.trim();
    if texte.is_empty() {
        return Err("Écris d'abord ta question.".into());
    }
    {
        let conn = etat.db.lock().unwrap();
        conversation(&conn, conversation_id).map_err(|e| e.to_string())?.ok_or("Conversation introuvable.")?;
        ajouter_message(&conn, conversation_id, false, texte, None, &[], maintenant_ms()).map_err(|e| e.to_string())?;
    }
    etat.taches.annulable(tache, repondre(&app, &etat, conversation_id)).await
}

/// Nouvelle tentative de réponse à la dernière question (après une erreur ou un arrêt).
#[tauri::command]
pub async fn relancer_reponse(
    app: AppHandle,
    etat: State<'_, Etat>,
    conversation_id: i64,
    tache: Option<String>,
) -> Result<ReponseIa, String> {
    etat.taches.annulable(tache, repondre(&app, &etat, conversation_id)).await
}

/// L'élève garde (ou refuse) un schéma ou un graphique proposé par l'IA.
#[tauri::command]
pub fn decider_creation(etat: State<Etat>, id: i64, accepter: bool) -> Result<Creation, String> {
    let conn = etat.db.lock().unwrap();
    conn.execute(
        "UPDATE creations_ia SET statut = ?2, date = ?3 WHERE id = ?1",
        params![id, if accepter { "acceptee" } else { "refusee" }, maintenant_ms()],
    )
    .map_err(|e| e.to_string())?;
    creation(&conn, id).map_err(|e| e.to_string())?.ok_or_else(|| "Création introuvable.".into())
}

#[tauri::command]
pub fn creations_ia(etat: State<Etat>) -> Result<Vec<Creation>, String> {
    creations_acceptees(&etat.db.lock().unwrap()).map_err(|e| e.to_string())
}

#[tauri::command]
pub fn renommer_creation(etat: State<Etat>, id: i64, titre: String) -> Result<(), String> {
    let titre = titre.trim();
    if titre.is_empty() {
        return Err("Le titre ne peut pas être vide.".into());
    }
    etat.db.lock().unwrap().execute("UPDATE creations_ia SET titre = ?2 WHERE id = ?1", params![id, titre]).map_err(|e| e.to_string())?;
    Ok(())
}

/// Une création supprimée de l'onglet « Créations » redevient une proposition refusée dans sa conversation.
#[tauri::command]
pub fn retirer_creation(etat: State<Etat>, id: i64) -> Result<(), String> {
    let conn = etat.db.lock().unwrap();
    conn.execute("UPDATE creations_ia SET statut = 'refusee' WHERE id = ?1 AND conversation_id IS NOT NULL", [id])
        .map_err(|e| e.to_string())?;
    conn.execute("DELETE FROM creations_ia WHERE id = ?1 AND conversation_id IS NULL", [id]).map_err(|e| e.to_string())?;
    Ok(())
}

/// Ce qui est ajouté à la fin d'une note pour une création : un titre et le bloc de code. Obsidian dessine
/// les schémas Mermaid ; les graphiques sont dessinés par Révisio (dans Obsidian, on voit leurs données).
pub fn bloc_pour_note(c: &Creation) -> String {
    let langage = if c.genre == "diagramme" { "mermaid" } else { "graphique" };
    format!("## {}\n\n```{langage}\n{}\n```\n", c.titre.trim(), c.contenu.trim())
}

/// Ajoute une création gardée à la fin d'une note (écriture autorisée seulement ; annulable dans le journal).
#[tauri::command]
pub fn ajouter_creation_a_note(etat: State<Etat>, id: i64, chemin: String) -> Result<(), String> {
    let reglages = etat.reglages.lock().unwrap().clone();
    let racine = reglages.vault().ok_or("Aucun dossier de notes choisi.")?;
    let conn = etat.db.lock().unwrap();
    let c = creation(&conn, id).map_err(|e| e.to_string())?.ok_or("Création introuvable.")?;
    if c.statut != "acceptee" {
        return Err("Garde d'abord ce visuel avant de l'ajouter à une note.".into());
    }
    // Octets exacts lus maintenant : l'écriture est refusée si la note change entre-temps.
    let lu = crate::vault::lire_octets(&racine, &chemin)?;
    let actuel = crate::vault::lire_texte(&racine, &chemin)?;
    let fin = if actuel.contains("\r\n") { "\r\n" } else { "\n" };
    let ajout = bloc_pour_note(&c).replace('\n', fin);
    let nouveau = format!("{}{fin}{fin}{ajout}", actuel.trim_end());
    let raison = format!("Ajout de « {} » (discussion avec l'IA)", c.titre);
    let ctx = crate::ecriture::Contexte {
        racine: &racine,
        autorisee: reglages.ecriture_autorisee,
        origine: "discussion",
        lot_id: None,
        raison: Some(&raison),
        maintenant: maintenant_ms(),
        groupe: None,
    };
    crate::ecriture::modifier(&conn, &ctx, &chemin, &nouveau, &|octets: &[u8]| octets == lu.as_slice())?;
    Ok(())
}

/// Points d'une fonction pour un graphique (calculés par Révisio, pas par l'IA). Les points où la
/// fonction n'est pas définie sont sautés.
#[tauri::command]
pub fn points_fonction(expression: String, xmin: f64, xmax: f64) -> Result<Vec<[f64; 2]>, String> {
    if !(xmin.is_finite() && xmax.is_finite() && xmin < xmax) {
        return Err("Intervalle invalide.".into());
    }
    let mut variables = std::collections::HashMap::new();
    let n = 240;
    let mut points = Vec::new();
    let mut erreur = None;
    for i in 0..=n {
        let x = xmin + (xmax - xmin) * i as f64 / n as f64;
        variables.insert("x".to_string(), x);
        match crate::calcul::evaluer(&expression, &variables) {
            Ok(y) if y.is_finite() && y.abs() < 1e9 => points.push([x, y]),
            Ok(_) => {}
            Err(e) => erreur = Some(e),
        }
    }
    if points.is_empty() {
        return Err(erreur.unwrap_or_else(|| "Fonction non définie sur cet intervalle.".into()));
    }
    Ok(points)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::db;

    fn message(ia: bool, texte: &str) -> Message {
        Message { id: 0, ia, texte: texte.into(), date: 0, modele: None, extraits: Vec::new() }
    }

    #[test]
    fn titre_tire_de_la_question() {
        assert_eq!(titre_depuis("  Pourquoi la   méiose ?  "), "Pourquoi la méiose ?");
        let long = "Peux-tu m'expliquer la différence entre la mitose et la méiose, avec les étapes principales de chacune ?";
        let t = titre_depuis(long);
        assert!(t.ends_with('…') && t.chars().count() <= 61, "{t}");
    }

    #[test]
    fn blocs_de_creation_et_titres() {
        let texte = "Voici les étapes [E1].\n\n```mermaid\n%% Titre : Étapes de la méiose\nflowchart LR\n  A[\"Prophase I\"] --> B[\"Métaphase I\"]\n```\n\nEt une courbe :\n```graphique\n{\"titre\": \"Croissance\", \"type\": \"fonction\", \"xmin\": 0, \"xmax\": 2, \"fonctions\": [{\"nom\": \"f\", \"expression\": \"exp(x)\"}]}\n```\n```python\nprint(1)\n```";
        let blocs = blocs_crees(texte);
        assert_eq!(blocs.len(), 2);
        assert_eq!((blocs[0].0.as_str(), blocs[0].1.as_str()), ("diagramme", "Étapes de la méiose"));
        assert!(blocs[0].2.starts_with("%% Titre"));
        assert_eq!((blocs[1].0.as_str(), blocs[1].1.as_str()), ("graphique", "Croissance"));
        // Sans titre : nom par défaut.
        assert_eq!(blocs_crees("```mermaid\nflowchart TD\nA-->B\n```")[0].1, "Schéma 1");
    }

    #[test]
    fn citations_cliquables() {
        let extraits = vec![
            Extrait { id: "E1".into(), note_chemin: "a.md".into(), titre_note: "A".into(), section: "s".into(), texte: "t".into(), markdown: "t".into() },
            Extrait { id: "E2".into(), note_chemin: "b.md".into(), titre_note: "B".into(), section: "s".into(), texte: "t".into(), markdown: "t".into() },
        ];
        let cites = extraits_cites("Vu en [E2] puis [E2, E1] et [E9].", &extraits);
        assert_eq!(cites.iter().map(|c| c.id.as_str()).collect::<Vec<_>>(), vec!["E2", "E1"]);
    }

    #[test]
    fn requete_avec_historique_et_sans_sources() {
        let precedents: Vec<Message> = (0..20).map(|i| message(i % 2 == 1, &format!("message {i}"))).collect();
        let r = requete("m", &precedents, &[], "Et pourquoi ?");
        assert_eq!(r.historique.len(), MESSAGES_GARDES);
        assert_eq!(r.historique.last().unwrap().texte, "message 19");
        let Partie::Texte(t) = &r.parties[0] else { panic!() };
        assert!(t.contains("hors de tes notes") && t.ends_with("Et pourquoi ?"));
    }

    #[test]
    fn conversation_enregistree_puis_supprimee() {
        let mut conn = db::ouvrir_en_memoire().unwrap();
        let sources = vec![Source { matiere: Some("Spé SVT".into()), chapitre: Some("Méiose".into()), note: None }];
        let id = creer(&conn, &sources, 1).unwrap();
        ajouter_message(&conn, id, false, "Qu'est-ce que le brassage ?", None, &[], 2).unwrap();
        let m = ajouter_message(&conn, id, true, "Réponse\n```mermaid\nflowchart TD\nA-->B\n```", Some("g"), &[], 3).unwrap();
        conn.execute(
            "INSERT INTO creations_ia(date, genre, titre, contenu, conversation_id, message_id, indice) VALUES (3, 'diagramme', 'S', 'x', ?1, ?2, 0)",
            params![id, m],
        )
        .unwrap();
        conn.execute(
            "INSERT INTO creations_ia(date, genre, titre, contenu, conversation_id, message_id, indice, statut)
             VALUES (3, 'graphique', 'G', '{}', ?1, ?2, 1, 'acceptee')",
            params![id, m],
        )
        .unwrap();
        let c = conversation(&conn, id).unwrap().unwrap();
        assert_eq!((c.titre.as_str(), c.messages, c.sources.clone()), ("Qu'est-ce que le brassage ?", 2, sources));
        assert_eq!(creations_acceptees(&conn).unwrap().len(), 1);
        supprimer(&mut conn, id).unwrap();
        assert!(conversations(&conn).unwrap().is_empty());
        // La création acceptée reste, sans conversation ; la proposition non acceptée a disparu.
        let restantes = creations_acceptees(&conn).unwrap();
        assert_eq!((restantes.len(), restantes[0].conversation_id), (1, None));
        let total: i64 = conn.query_row("SELECT count(*) FROM creations_ia", [], |r| r.get(0)).unwrap();
        assert_eq!(total, 1);
    }

    #[test]
    fn bloc_ajoute_a_une_note() {
        let c = Creation {
            id: 1,
            date: 0,
            genre: "diagramme".into(),
            titre: " Étapes ".into(),
            contenu: "flowchart TD\nA-->B\n".into(),
            conversation_id: None,
            conversation_titre: None,
            message_id: None,
            indice: 0,
            statut: "acceptee".into(),
        };
        assert_eq!(bloc_pour_note(&c), "## Étapes\n\n```mermaid\nflowchart TD\nA-->B\n```\n");
    }

    #[test]
    fn fonction_calculee_par_revisio() {
        let p = points_fonction("x^2".into(), -2.0, 2.0).unwrap();
        assert_eq!(p.len(), 241);
        assert_eq!(p[0], [-2.0, 4.0]);
        // ln n'est pas définie pour x ≤ 0 : ces points sont sautés.
        let l = points_fonction("ln(x)".into(), -1.0, 1.0).unwrap();
        assert!(l.iter().all(|[x, _]| *x > 0.0));
        assert!(points_fonction("x^2".into(), 1.0, 0.0).is_err());
    }
}
