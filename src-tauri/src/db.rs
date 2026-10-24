//! Base SQLite de Révisio (`%APPDATA%\Revisio\app.db`).
//!
//! Le schéma évolue par « migrations » numérotées : `PRAGMA user_version` retient
//! la dernière appliquée, et chaque lancement applique celles qui manquent.

use std::path::Path;

use rusqlite::Connection;

use crate::paths::verifier_hors_vault;

const MIGRATIONS: &[&str] = &[
    // 1 — index du vault + recherche plein texte
    r#"
    CREATE TABLE notes (
        id          INTEGER PRIMARY KEY,
        chemin      TEXT NOT NULL UNIQUE,      -- relatif au vault, séparateurs « / »
        nom         TEXT NOT NULL,             -- nom du fichier sans « .md » (sert à résoudre les [[liens]])
        titre       TEXT NOT NULL,             -- nom affiché
        matiere     TEXT,
        chapitre    TEXT,
        genre       TEXT NOT NULL,             -- cours, fiche_synthese, schema, vocabulaire…
        tags        TEXT NOT NULL DEFAULT '[]',
        frontmatter TEXT NOT NULL DEFAULT '{}',
        taille      INTEGER NOT NULL,
        modifie     INTEGER NOT NULL
    );
    CREATE INDEX notes_nom ON notes(nom COLLATE NOCASE);

    CREATE TABLE sections (
        id           INTEGER PRIMARY KEY,
        note_id      INTEGER NOT NULL REFERENCES notes(id) ON DELETE CASCADE,
        ordre        INTEGER NOT NULL,
        genre        TEXT NOT NULL,
        niveau       INTEGER NOT NULL,
        type_callout TEXT,
        titre        TEXT NOT NULL,
        texte        TEXT NOT NULL,
        ligne        INTEGER NOT NULL
    );
    CREATE INDEX sections_note ON sections(note_id);

    CREATE TABLE liens (
        source_id INTEGER NOT NULL REFERENCES notes(id) ON DELETE CASCADE,
        cible     TEXT NOT NULL,                -- tel qu'écrit dans [[...]]
        ancre     TEXT,
        integre   INTEGER NOT NULL
    );
    CREATE INDEX liens_source ON liens(source_id);

    -- Fichiers non Markdown (images…), pour afficher ![[image.png]]
    CREATE TABLE fichiers (
        chemin  TEXT PRIMARY KEY,
        nom     TEXT NOT NULL,
        taille  INTEGER NOT NULL,
        modifie INTEGER NOT NULL
    );
    CREATE INDEX fichiers_nom ON fichiers(nom COLLATE NOCASE);

    -- Recherche plein texte, insensible aux accents (« evolution » trouve « évolution »)
    CREATE VIRTUAL TABLE recherche USING fts5(
        titre, section, texte,
        note_id UNINDEXED, section_id UNINDEXED,
        tokenize = 'unicode61 remove_diacritics 2'
    );
    "#,
    // 2 — valeurs diverses (ex. version du parseur ayant produit l'index)
    r#"
    CREATE TABLE meta (
        cle    TEXT PRIMARY KEY,
        valeur TEXT NOT NULL
    );
    "#,
    // 3 — révision : cartes (avec état FSRS), historique des révisions, carnet d'erreurs
    r#"
    CREATE TABLE cartes (
        id            INTEGER PRIMARY KEY,
        cle           TEXT NOT NULL UNIQUE,    -- identifiant stable (la progression survit aux réindexations)
        genre         TEXT NOT NULL,           -- vocabulaire, trou, schema, ordre
        note_chemin   TEXT NOT NULL,
        matiere       TEXT,
        chapitre      TEXT,
        section       TEXT NOT NULL DEFAULT '',
        question      TEXT NOT NULL,
        reponse       TEXT NOT NULL,
        donnees       TEXT NOT NULL DEFAULT '{}', -- JSON propre au genre de carte
        extrait       TEXT NOT NULL DEFAULT '',   -- passage de la note contenant la réponse
        active        INTEGER NOT NULL DEFAULT 1, -- 0 si la note source ne la produit plus
        suspendue     INTEGER NOT NULL DEFAULT 0, -- 1 si signalée « fausse / inutile »
        creee_le      INTEGER NOT NULL,
        etat_fsrs     TEXT,                       -- JSON ts-fsrs ; NULL = jamais révisée
        echeance      INTEGER,                    -- date de la prochaine révision (ms)
        introduite_le INTEGER                     -- date de la première révision (ms)
    );
    CREATE INDEX cartes_echeance ON cartes(echeance);
    CREATE INDEX cartes_note ON cartes(note_chemin);

    CREATE TABLE revisions (
        id             INTEGER PRIMARY KEY,
        carte_id       INTEGER NOT NULL REFERENCES cartes(id) ON DELETE CASCADE,
        date           INTEGER NOT NULL,
        note           INTEGER NOT NULL,          -- 1 à revoir, 2 difficile, 3 bien, 4 facile
        confiance      TEXT NOT NULL,             -- sur, hesitant, hasard
        correcte       INTEGER NOT NULL,
        reponse_donnee TEXT,
        duree_ms       INTEGER NOT NULL DEFAULT 0
    );
    CREATE INDEX revisions_date ON revisions(date);
    CREATE INDEX revisions_carte ON revisions(carte_id);

    CREATE TABLE erreurs (
        id               INTEGER PRIMARY KEY,
        carte_id         INTEGER REFERENCES cartes(id) ON DELETE SET NULL,
        date             INTEGER NOT NULL,
        genre            TEXT NOT NULL,
        matiere          TEXT,
        chapitre         TEXT,
        note_chemin      TEXT NOT NULL,
        section          TEXT NOT NULL DEFAULT '',
        question         TEXT NOT NULL,
        reponse_attendue TEXT NOT NULL,
        reponse_donnee   TEXT,
        extrait          TEXT NOT NULL DEFAULT '',
        comprise         INTEGER NOT NULL DEFAULT 0
    );
    CREATE INDEX erreurs_date ON erreurs(date);
    "#,
    // 4 — IA : journal des appels (débit, quota du jour) et générations conservées (hors ligne)
    r#"
    CREATE TABLE journal_ia (
        id            INTEGER PRIMARY KEY,
        date          INTEGER NOT NULL,
        usage         TEXT NOT NULL,          -- questions, correction, feynman…
        modele        TEXT NOT NULL,
        statut        TEXT NOT NULL,          -- ok, erreur
        jetons_entree INTEGER NOT NULL DEFAULT 0,
        jetons_sortie INTEGER NOT NULL DEFAULT 0,
        duree_ms      INTEGER NOT NULL DEFAULT 0,
        erreur        TEXT
    );
    CREATE INDEX journal_ia_date ON journal_ia(date);

    CREATE TABLE generations_ia (
        id        INTEGER PRIMARY KEY,
        date      INTEGER NOT NULL,
        usage     TEXT NOT NULL,
        modele    TEXT NOT NULL,
        matiere   TEXT,
        chapitre  TEXT,
        sources   TEXT NOT NULL DEFAULT '[]',  -- extraits envoyés (chemins + sections)
        resultat  TEXT NOT NULL                -- JSON validé renvoyé par l'IA
    );
    "#,
    // 5 — méthode Feynman : explications de l'élève et retours de l'IA
    r#"
    CREATE TABLE feynman (
        id          INTEGER PRIMARY KEY,
        date        INTEGER NOT NULL,
        matiere     TEXT NOT NULL,
        chapitre    TEXT,
        notion      TEXT NOT NULL,
        explication TEXT NOT NULL,
        note        INTEGER NOT NULL,
        retour      TEXT NOT NULL              -- JSON (ce qui est juste, manques, erreurs, conseil)
    );
    "#,
    // 6 — exercices type bac et mode examen : sujets générés (réutilisables hors ligne) et copies corrigées
    r#"
    CREATE TABLE sujets (
        id            INTEGER PRIMARY KEY,
        date          INTEGER NOT NULL,
        type          TEXT NOT NULL,
        matiere       TEXT NOT NULL,
        chapitres     TEXT NOT NULL,            -- JSON [{matiere, chapitre}]
        modele        TEXT NOT NULL,
        duree_minutes INTEGER NOT NULL,
        sujet         TEXT NOT NULL,            -- JSON validé (questions, barème, valeurs recalculées)
        extraits      TEXT NOT NULL             -- JSON : extraits des notes envoyés (sources affichées à la correction)
    );
    CREATE TABLE copies (
        id         INTEGER PRIMARY KEY,
        sujet_id   INTEGER NOT NULL REFERENCES sujets(id) ON DELETE CASCADE,
        date       INTEGER NOT NULL,
        duree_ms   INTEGER NOT NULL,
        reponses   TEXT NOT NULL,               -- JSON [texte de chaque réponse]
        resultats  TEXT NOT NULL,               -- JSON [résultat de chaque question]
        appreciation TEXT NOT NULL DEFAULT '',
        note       REAL                         -- sur 20 (questions corrigées seulement)
    );
    CREATE INDEX copies_sujet ON copies(sujet_id);
    "#,
    // 7 — dates d'évaluation (le planning des révisions est recalculé chaque jour à partir d'elles)
    r#"
    CREATE TABLE evaluations (
        id        INTEGER PRIMARY KEY,
        jour      TEXT NOT NULL,                -- AAAA-MM-JJ (jour local)
        matiere   TEXT NOT NULL,
        titre     TEXT NOT NULL,
        genre     TEXT NOT NULL,                -- controle, ds, oral, bac_blanc, autre
        chapitres TEXT NOT NULL,                -- JSON [chapitre ou null pour les notes générales]
        creee_le  INTEGER NOT NULL
    );
    CREATE INDEX evaluations_jour ON evaluations(jour);
    "#,
    // 8 — cartes mentales rédigées par l'IA (le « plan du chapitre » sans IA est recalculé à la demande)
    r#"
    CREATE TABLE cartes_mentales (
        id       INTEGER PRIMARY KEY,
        date     INTEGER NOT NULL,
        matiere  TEXT NOT NULL,
        chapitre TEXT,
        modele   TEXT NOT NULL,
        arbre    TEXT NOT NULL                  -- JSON {texte, detail, note, enfants}
    );
    CREATE INDEX cartes_mentales_chapitre ON cartes_mentales(matiere, chapitre);
    "#,
    // 9 — podcasts à deux voix (fichiers audio dans %APPDATA%\Revisio\audio)
    r#"
    CREATE TABLE podcasts (
        id             INTEGER PRIMARY KEY,
        date           INTEGER NOT NULL,
        matiere        TEXT NOT NULL,
        chapitre       TEXT,
        titre          TEXT NOT NULL,
        options        TEXT NOT NULL,           -- JSON {dureeMinutes, ton, detail, langue}
        modele_script  TEXT NOT NULL,
        modele_tts     TEXT NOT NULL,
        orateurs       TEXT NOT NULL,           -- JSON [{nom, voix}]
        repliques      TEXT NOT NULL,           -- JSON (texte, source, morceau, début en ms)
        extraits       TEXT NOT NULL,           -- JSON : sources affichées avec la transcription
        morceaux       INTEGER NOT NULL,        -- morceaux audio prévus (1 appel de synthèse vocale chacun)
        morceaux_faits INTEGER NOT NULL DEFAULT 0,
        fichier        TEXT,                    -- fichier final (WAV, puis Opus/WebM après compression)
        duree_ms       INTEGER,
        position_ms    INTEGER NOT NULL DEFAULT 0  -- reprise de l'écoute
    );
    "#,
    // 10 — espace web : favoris par matière et passages enregistrés comme sources (jamais dans le vault)
    r#"
    CREATE TABLE favoris (
        id       INTEGER PRIMARY KEY,
        matiere  TEXT,                          -- NULL = favori général
        titre    TEXT NOT NULL,
        url      TEXT NOT NULL,
        creee_le INTEGER NOT NULL
    );
    CREATE TABLE sources_web (
        id       INTEGER PRIMARY KEY,
        date     INTEGER NOT NULL,
        url      TEXT NOT NULL,
        titre    TEXT NOT NULL,
        texte    TEXT NOT NULL,
        matiere  TEXT NOT NULL,
        chapitre TEXT
    );
    CREATE INDEX sources_web_chapitre ON sources_web(matiere, chapitre);
    "#,
    // 11 — vidéos : diaporamas narrés (narrations dans audio/, MP4 dans videos/)
    r#"
    CREATE TABLE videos (
        id            INTEGER PRIMARY KEY,
        date          INTEGER NOT NULL,
        matiere       TEXT NOT NULL,
        chapitre      TEXT,
        titre         TEXT NOT NULL,
        options       TEXT NOT NULL,            -- JSON {diapos, langue}
        modele_script TEXT NOT NULL,
        modele_tts    TEXT NOT NULL,
        voix          TEXT NOT NULL,
        diapos        TEXT NOT NULL,            -- JSON [{titre, points, schema, narration, source, dureeMs}]
        extraits      TEXT NOT NULL,
        faites        INTEGER NOT NULL DEFAULT 0, -- narrations déjà enregistrées
        fichier       TEXT,                     -- vidéo finale (MP4, ou WebM si le H.264 manque)
        duree_ms      INTEGER
    );
    "#,
    // 12 — écriture dans le vault (si l'élève l'autorise) : historique pour tout annuler,
    //      photos de cours transformées en notes, modifications proposées à valider
    r#"
    CREATE TABLE historique_vault (
        id             INTEGER PRIMARY KEY,
        date           INTEGER NOT NULL,
        chemin         TEXT NOT NULL,           -- relatif au vault, « / »
        genre          TEXT NOT NULL,           -- creation | modification
        avant          BLOB,                    -- contenu exact avant (NULL pour une création)
        apres          BLOB NOT NULL,           -- contenu écrit
        origine        TEXT NOT NULL,           -- manuel | photos
        lot_id         INTEGER,
        raison         TEXT,
        dossiers_crees TEXT NOT NULL DEFAULT '[]', -- JSON : dossiers créés pour cette note (supprimés à l'annulation s'ils sont vides)
        annulee_le     INTEGER
    );
    CREATE INDEX historique_vault_lot ON historique_vault(lot_id);

    CREATE TABLE lots_photos (
        id        INTEGER PRIMARY KEY,
        date      INTEGER NOT NULL,
        source    TEXT NOT NULL,                -- import | dossier
        photos    TEXT NOT NULL,                -- JSON : chemins absolus des photos
        statut    TEXT NOT NULL,                -- attente | en_cours | termine | illisible | erreur
        matiere   TEXT,
        chapitre  TEXT,
        dossier   TEXT,                         -- dossier du vault choisi par l'IA
        message   TEXT,
        modele    TEXT
    );

    CREATE TABLE photos_vues (
        chemin  TEXT PRIMARY KEY,               -- chemin absolu dans le dossier surveillé
        taille  INTEGER NOT NULL,
        date    INTEGER NOT NULL,
        lot_id  INTEGER                         -- NULL : déjà là quand la surveillance a commencé (ignorée)
    );

    CREATE TABLE propositions_vault (
        id            INTEGER PRIMARY KEY,
        date          INTEGER NOT NULL,
        lot_id        INTEGER,
        chemin        TEXT NOT NULL,
        avant         BLOB NOT NULL,            -- contenu lu au moment de la proposition
        apres         TEXT NOT NULL,
        raison        TEXT NOT NULL,
        statut        TEXT NOT NULL DEFAULT 'attente', -- attente | acceptee | refusee | perimee
        historique_id INTEGER
    );
    "#,
    // 13 — gestion des fichiers : dossiers, renommages/déplacements, corbeille (suppressions annulables)
    r#"
    ALTER TABLE historique_vault ADD COLUMN chemin_nouveau TEXT;   -- déplacement : nouvelle place
    ALTER TABLE historique_vault ADD COLUMN groupe TEXT;           -- opérations annulées ensemble
    CREATE INDEX historique_vault_groupe ON historique_vault(groupe);
    "#,
];

pub fn lire_meta(conn: &Connection, cle: &str) -> Option<String> {
    conn.query_row("SELECT valeur FROM meta WHERE cle = ?1", [cle], |r| r.get(0)).ok()
}

pub fn ecrire_meta(conn: &Connection, cle: &str, valeur: &str) -> rusqlite::Result<()> {
    conn.execute(
        "INSERT INTO meta(cle, valeur) VALUES (?1, ?2) ON CONFLICT(cle) DO UPDATE SET valeur = excluded.valeur",
        [cle, valeur],
    )?;
    Ok(())
}

/// Ouvre (ou crée) la base et applique les migrations manquantes.
pub fn ouvrir(chemin: &Path, vault: Option<&Path>) -> Result<Connection, String> {
    verifier_hors_vault(chemin, vault)?;
    if let Some(parent) = chemin.parent() {
        std::fs::create_dir_all(parent).map_err(|e| format!("Création du dossier de données : {e}"))?;
    }
    let conn = Connection::open(chemin).map_err(|e| format!("Ouverture de la base : {e}"))?;
    preparer(conn)
}

/// Base en mémoire (tests).
pub fn ouvrir_en_memoire() -> Result<Connection, String> {
    preparer(Connection::open_in_memory().map_err(|e| e.to_string())?)
}

fn preparer(mut conn: Connection) -> Result<Connection, String> {
    conn.execute_batch("PRAGMA journal_mode = WAL; PRAGMA foreign_keys = ON;")
        .map_err(|e| e.to_string())?;
    migrer(&mut conn)?;
    Ok(conn)
}

fn migrer(conn: &mut Connection) -> Result<(), String> {
    let version: i64 = conn
        .query_row("PRAGMA user_version", [], |r| r.get(0))
        .map_err(|e| e.to_string())?;
    for (i, sql) in MIGRATIONS.iter().enumerate().skip(version as usize) {
        let tx = conn.transaction().map_err(|e| e.to_string())?;
        tx.execute_batch(sql).map_err(|e| format!("Migration {} : {e}", i + 1))?;
        tx.pragma_update(None, "user_version", (i + 1) as i64).map_err(|e| e.to_string())?;
        tx.commit().map_err(|e| e.to_string())?;
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn migrations_appliquees_et_fts5_disponible() {
        let conn = ouvrir_en_memoire().unwrap();
        let v: i64 = conn.query_row("PRAGMA user_version", [], |r| r.get(0)).unwrap();
        assert_eq!(v as usize, MIGRATIONS.len());
        conn.execute(
            "INSERT INTO recherche(titre, section, texte, note_id, section_id) VALUES ('Évolution', '', 'la méiose', 1, 1)",
            [],
        )
        .unwrap();
        let n: i64 = conn
            .query_row("SELECT count(*) FROM recherche WHERE recherche MATCH 'evolution meiose'", [], |r| r.get(0))
            .unwrap();
        assert_eq!(n, 1);
    }
}
