import { useEffect, useRef, useState } from "react";
import { ExternalLink, Pencil, Save, Undo2, X } from "lucide-react";
import { api, messageErreur, Note } from "../lib/api";
import Message from "../composants/Message";
import Dessin from "../excalidraw/Dessin";
import { extraireScene } from "../excalidraw/scene";
import { MarkdownObsidian } from "../markdown/rendu";
import { BADGES } from "./ArbreNotes";
import { TOUCHE_COMMANDE, toucheCommande } from "../lib/plateforme";

interface Props {
  chemin: string;
  ancre: string | null;
  onOuvrir: (chemin: string, ancre?: string | null) => void;
  /** Change à chaque réindexation : la note est relue si elle a été modifiée dans Obsidian. */
  versionIndex: unknown;
  /** Mode Modifier proposé seulement si l'élève a autorisé l'écriture dans le vault. */
  ecritureAutorisee: boolean;
}

const LIBELLES_GENRE: Record<string, string> = {
  fiche_synthese: "Fiche de synthèse",
  schema: "Schéma",
  vocabulaire: "Vocabulaire",
  methodologie: "Méthodologie",
  reference: "Fiche de référence",
  etude_lineaire: "Étude linéaire",
  revision: "Révision hebdomadaire",
  accueil: "Accueil",
};

export default function LecteurNote({ chemin, ancre, onOuvrir, versionIndex, ecritureAutorisee }: Props) {
  const [note, setNote] = useState<Note | null>(null);
  const [erreur, setErreur] = useState<string | null>(null);
  // Mode Modifier : texte en cours d'édition (null = lecture).
  const [brouillon, setBrouillon] = useState<string | null>(null);
  const [enregistre, setEnregistre] = useState<{ id: number } | null>(null);
  const [erreurEdition, setErreurEdition] = useState<string | null>(null);
  const [relecture, setRelecture] = useState(0);
  const [cheminParent, setCheminParent] = useState<string | null>(null);
  const corps = useRef<HTMLDivElement>(null);

  // Pour un schéma « X (schéma) », la note « X » associée, si elle existe.
  const parent = note?.genre === "schema" ? note.titre.replace(/ \(schéma\)$/, "") : null;
  useEffect(() => {
    setCheminParent(null);
    if (!parent || !note) return;
    api.resoudreLien(parent, note.chemin).then((c) => setCheminParent(c?.genre === "note" ? c.chemin : null));
  }, [parent, note]);

  useEffect(() => {
    let actif = true;
    setErreur(null);
    api
      .note(chemin)
      .then((n) => actif && setNote(n))
      .catch((e) => actif && setErreur(messageErreur(e)));
    return () => {
      actif = false;
    };
  }, [chemin, versionIndex, relecture]);

  // Autre note : on quitte le mode Modifier.
  useEffect(() => {
    setBrouillon(null);
    setEnregistre(null);
    setErreurEdition(null);
  }, [chemin]);

  async function enregistrer() {
    if (!note || brouillon === null) return;
    setErreurEdition(null);
    try {
      const id = await api.enregistrerNote(note.chemin, brouillon, note.contenu);
      setEnregistre({ id });
      setBrouillon(null);
      setRelecture((n) => n + 1);
    } catch (e) {
      setErreurEdition(messageErreur(e));
    }
  }

  async function annulerEnregistrement() {
    if (!enregistre) return;
    try {
      await api.annulerEcriture(enregistre.id);
      setEnregistre(null);
      setRelecture((n) => n + 1);
    } catch (e) {
      setErreurEdition(messageErreur(e));
    }
  }

  // Remonte en haut à chaque nouvelle note, ou va à la section visée par [[Note#Section]].
  useEffect(() => {
    if (!note || note.chemin !== chemin) return;
    const conteneur = corps.current;
    if (!conteneur) return;
    const cible = ancre?.toLowerCase().trim();
    const titre = cible
      ? [...conteneur.querySelectorAll("h1,h2,h3,h4,h5,h6,.callout-titre")].find(
          (h) => h.textContent?.toLowerCase().trim() === cible,
        )
      : null;
    if (titre) titre.scrollIntoView({ block: "start" });
    else conteneur.closest(".lecteur")?.scrollTo({ top: 0 });
  }, [note, chemin, ancre]);

  if (erreur) return <div className="lecteur"><p className="message-erreur">{erreur}</p></div>;
  if (!note) return <div className="lecteur texte-doux">Chargement…</div>;

  const navigation = { source: note.chemin, ouvrirNote: onOuvrir };

  return (
    <div className="lecteur">
      <article className="page-note">
        <header className="entete-note">
          <div className="fil texte-doux petit">
            {[note.matiere, note.chapitre].filter(Boolean).join(" › ") || "Hors matière"}
          </div>
          <h1>{note.genre === "accueil" ? `Accueil — ${note.matiere ?? "mes notes"}` : note.titre}</h1>
          <div className="rangee">
            {LIBELLES_GENRE[note.genre] && (
              <span className={`badge badge-${note.genre}`}>{LIBELLES_GENRE[note.genre] ?? BADGES[note.genre]}</span>
            )}
            {note.tags.map((t) => (
              <span key={t} className="tag">
                #{t}
              </span>
            ))}
            <span className="espace" />
            {ecritureAutorisee && note.genre !== "schema" && brouillon === null && (
              <button className="bouton" onClick={() => (setBrouillon(note.contenu), setEnregistre(null))}>
                <Pencil size={15} aria-hidden /> Modifier
              </button>
            )}
            {note.urlObsidian && (
              <button className="bouton" onClick={() => api.ouvrirDansObsidian(note.chemin)} title={note.urlObsidian}>
                <ExternalLink size={15} aria-hidden /> Ouvrir dans Obsidian
              </button>
            )}
          </div>
        </header>

        {enregistre && (
          <Message genre="succes">
            Note enregistrée. L'ancienne version est gardée dans le journal.{" "}
            <button className="lien-discret" onClick={annulerEnregistrement}>
              <Undo2 size={14} aria-hidden /> Annuler cette modification
            </button>
          </Message>
        )}
        {erreurEdition && <p className="message-erreur">{erreurEdition}</p>}

        <div ref={corps}>
          {brouillon !== null ? (
            <div className="editeur-note">
              <textarea
                value={brouillon}
                onChange={(e) => setBrouillon(e.target.value)}
                spellCheck
                lang="fr"
                aria-label="Contenu de la note (Markdown)"
                onKeyDown={(e) => {
                  if (toucheCommande(e) && e.key.toLowerCase() === "s") {
                    e.preventDefault();
                    enregistrer();
                  }
                }}
              />
              <div className="rangee">
                <button className="bouton bouton-principal" disabled={brouillon === note.contenu} onClick={enregistrer}>
                  <Save size={15} aria-hidden /> Enregistrer
                </button>
                <button className="bouton" onClick={() => (setBrouillon(null), setErreurEdition(null))}>
                  <X size={15} aria-hidden /> Fermer sans enregistrer
                </button>
                <span className="texte-doux petit">{TOUCHE_COMMANDE} S pour enregistrer. Syntaxe Markdown d'Obsidian.</span>
              </div>
            </div>
          ) : note.genre === "schema" ? (
            <div className="schema">
              <Dessin scene={extraireScene(note.contenu)} titre={note.titre} />
              <p className="texte-doux petit">Légendes du schéma :</p>
              <div className="integration-legendes">
                {note.legendes.map((l, i) => (
                  <span key={i} className="etiquette">
                    {l}
                  </span>
                ))}
              </div>
              {cheminParent && (
                <p>
                  Note associée :{" "}
                  <a href="#" onClick={(e) => (e.preventDefault(), onOuvrir(cheminParent))}>
                    {parent}
                  </a>
                </p>
              )}
            </div>
          ) : (
            <MarkdownObsidian markdown={note.contenu} navigation={navigation} />
          )}
        </div>

        {note.retroliens.length > 0 && (
          <footer className="retroliens">
            <h2>Mentionnée dans</h2>
            <ul>
              {note.retroliens.map((r) => (
                <li key={r.chemin}>
                  <a href="#" onClick={(e) => (e.preventDefault(), onOuvrir(r.chemin))}>
                    {r.titre}
                  </a>
                </li>
              ))}
            </ul>
          </footer>
        )}
      </article>
    </div>
  );
}
