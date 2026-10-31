// Écran « Discuter avec l'IA » : conversations enregistrées (liste à gauche), conversation ouverte avec ses
// sources (notes choisies) et ses réponses au fil de l'eau, et l'onglet « Créations » (schémas et graphiques
// gardés). La génération est une tâche de fond : on peut changer d'écran pendant que l'IA répond.
import { useEffect, useMemo, useRef, useState } from "react";
import {
  BookMarked,
  KeyRound,
  MessageSquarePlus,
  MessagesSquare,
  Pencil,
  RotateCcw,
  Search,
  SendHorizontal,
  Shapes,
  Trash,
} from "lucide-react";
import EntetePage from "../composants/EntetePage";
import Message from "../composants/Message";
import { BoutonArreter } from "../composants/TachesIa";
import { api, messageErreur, type Conversation, type ConversationComplete, type SourceConversation } from "../lib/api";
import { useMoteurIa } from "../lib/moteurIa";
import { lancerTache, oublierTache, useDerniereTache } from "../lib/taches";
import BadgeIa from "../views/BadgeIa";
import ChoixSources, { resumeSources } from "./ChoixSources";
import Creations from "./Creations";
import { ecouterFlux, effacerFlux, useFlux } from "./flux";
import { Question, ReponseIa } from "./Messages";

interface Props {
  ouvrirNote: (chemin: string) => void;
  onReglages: () => void;
  ecritureAutorisee: boolean;
  /** En arrivant : ouvrir une conversation, ou l'onglet Créations (lien depuis l'Atelier IA). */
  demande?: { conversationId?: number; onglet?: "creations" } | null;
}

const formatDate = new Intl.DateTimeFormat("fr-FR", { day: "numeric", month: "short" });
const formatHeure = new Intl.DateTimeFormat("fr-FR", { hour: "2-digit", minute: "2-digit" });

function quand(ms: number): string {
  const d = new Date(ms);
  return d.toDateString() === new Date().toDateString() ? formatHeure.format(d) : formatDate.format(d);
}

// La dernière conversation ouverte est retrouvée en revenant sur l'écran.
let derniereOuverte: number | null = null;

export default function DiscussionVue({ ouvrirNote, onReglages, ecritureAutorisee, demande }: Props) {
  const moteur = useMoteurIa();
  const [onglet, setOnglet] = useState<"conversations" | "creations">("conversations");
  const [liste, setListe] = useState<Conversation[]>([]);
  const [ouverte, setOuverte] = useState<ConversationComplete | null>(null);
  const [recherche, setRecherche] = useState("");
  const [question, setQuestion] = useState("");
  /** Question envoyée mais pas encore enregistrée (affichée tout de suite). */
  const [enAttente, setEnAttente] = useState<string | null>(null);
  const [choixSources, setChoixSources] = useState<"nouvelle" | "modifier" | null>(null);
  const [renommage, setRenommage] = useState<string | null>(null);
  const [aSupprimer, setASupprimer] = useState(false);
  const [erreur, setErreur] = useState<string | null>(null);
  const bas = useRef<HTMLDivElement>(null);

  const id = ouverte?.conversation.id ?? null;
  const tache = useDerniereTache("discussion", id === null ? "" : String(id));
  const enCours = tache?.statut === "en-cours";
  const flux = useFlux(id);

  useEffect(ecouterFlux, []);

  const charger = () =>
    api
      .conversations()
      .then(setListe)
      .catch((e) => setErreur(messageErreur(e)));

  function ouvrir(conversationId: number) {
    derniereOuverte = conversationId;
    setErreur(null);
    setASupprimer(false);
    setRenommage(null);
    setOnglet("conversations");
    return api
      .ouvrirConversation(conversationId)
      .then(setOuverte)
      .catch((e) => setErreur(messageErreur(e)));
  }

  useEffect(() => {
    charger();
    const cible = demande?.conversationId ?? derniereOuverte;
    if (cible !== null) ouvrir(cible).then(() => demande?.onglet && setOnglet(demande.onglet));
    else if (demande?.onglet) setOnglet(demande.onglet);
  }, [demande]);

  // Réponse terminée (ici ou pendant qu'on était ailleurs) : on relit la conversation.
  useEffect(() => {
    if (!tache || tache.statut === "en-cours" || id === null) return;
    effacerFlux(id);
    setEnAttente(null);
    ouvrir(id);
    charger();
  }, [tache?.statut]);

  // On suit le bas de la conversation pendant que la réponse s'écrit.
  useEffect(() => {
    bas.current?.scrollIntoView({ block: "end" });
  }, [ouverte?.messages.length, flux, enAttente]);

  async function nouvelle(sources: SourceConversation[]) {
    setChoixSources(null);
    try {
      const c = await api.creerConversation(sources);
      await charger();
      await ouvrir(c.id);
    } catch (e) {
      setErreur(messageErreur(e));
    }
  }

  async function changerSources(sources: SourceConversation[]) {
    setChoixSources(null);
    if (id === null) return;
    try {
      await api.definirSourcesConversation(id, sources);
      await ouvrir(id);
      charger();
    } catch (e) {
      setErreur(messageErreur(e));
    }
  }

  function lancer(travail: (tacheId: string) => Promise<unknown>) {
    if (!ouverte) return;
    const c = ouverte.conversation;
    lancerTache(
      {
        genre: "discussion",
        cle: String(c.id),
        titre: `Discussion · ${c.titre}`,
        ecran: "discussion",
        resume: () => "L'IA a répondu",
      },
      ({ id: tacheId }) => travail(tacheId),
    ).catch(() => {});
  }

  function envoyer() {
    const texte = question.trim();
    if (!texte || !ouverte || enCours) return;
    setQuestion("");
    setEnAttente(texte);
    const conversationId = ouverte.conversation.id;
    lancer((tacheId) => api.envoyerMessage(conversationId, texte, tacheId));
  }

  function reessayer() {
    if (!ouverte) return;
    const conversationId = ouverte.conversation.id;
    if (tache) oublierTache(tache.id);
    lancer((tacheId) => api.relancerReponse(conversationId, tacheId));
  }

  async function decider(creationId: number, garder: boolean) {
    try {
      await api.deciderCreation(creationId, garder);
      if (id !== null) await ouvrir(id);
    } catch (e) {
      setErreur(messageErreur(e));
    }
  }

  async function renommer() {
    if (id === null || renommage === null) return;
    try {
      await api.renommerConversation(id, renommage);
      setRenommage(null);
      await ouvrir(id);
      charger();
    } catch (e) {
      setErreur(messageErreur(e));
    }
  }

  async function supprimer() {
    if (id === null) return;
    try {
      await api.supprimerConversation(id);
      derniereOuverte = null;
      setOuverte(null);
      setASupprimer(false);
      charger();
    } catch (e) {
      setErreur(messageErreur(e));
    }
  }

  const filtrees = useMemo(() => {
    const r = recherche.trim().toLowerCase();
    return r ? liste.filter((c) => (c.titre + " " + c.apercu).toLowerCase().includes(r)) : liste;
  }, [liste, recherche]);

  const messages = ouverte?.messages ?? [];
  // La dernière question n'a pas de réponse (erreur ou arrêt) : on propose de réessayer.
  const sansReponse = !enCours && !enAttente && messages.length > 0 && !messages[messages.length - 1].ia;

  return (
    <div className="page page-large discussion">
      <EntetePage
        icone={MessagesSquare}
        surTitre="Créer avec l'IA"
        titre="Discuter avec l'IA"
        description="Pose tes questions : l'IA t'explique en s'appuyant sur les notes que tu choisis, et propose des schémas quand ça aide."
        actions={
          <button className="bouton bouton-principal" onClick={() => setChoixSources("nouvelle")}>
            <MessageSquarePlus size={16} aria-hidden /> Nouvelle conversation
          </button>
        }
      />

      <div className="discussion-grille">
        <aside className="carte discussion-liste" aria-label="Conversations">
          <div className="segmente discussion-onglets" role="tablist">
            <button
              role="tab"
              aria-selected={onglet === "conversations"}
              className={"segment" + (onglet === "conversations" ? " actif" : "")}
              onClick={() => setOnglet("conversations")}
            >
              <MessagesSquare size={15} aria-hidden /> Conversations
            </button>
            <button
              role="tab"
              aria-selected={onglet === "creations"}
              className={"segment" + (onglet === "creations" ? " actif" : "")}
              onClick={() => setOnglet("creations")}
            >
              <Shapes size={15} aria-hidden /> Créations
            </button>
          </div>
          {onglet === "conversations" && (
            <>
              <label className="champ-recherche">
                <Search size={15} className="icone" aria-hidden />
                <input className="champ" placeholder="Chercher…" value={recherche} onChange={(e) => setRecherche(e.target.value)} />
              </label>
              {liste.length === 0 && <p className="texte-doux petit">Aucune conversation pour l'instant.</p>}
              <ul className="liste-conversations">
                {filtrees.map((c) => (
                  <li key={c.id}>
                    <button className={"conversation-item" + (c.id === id ? " actif" : "")} onClick={() => ouvrir(c.id)}>
                      <span className="conversation-titre">{c.titre}</span>
                      <span className="texte-doux petit conversation-infos">
                        {quand(c.maj)} · {resumeSources(c.sources)}
                      </span>
                    </button>
                  </li>
                ))}
              </ul>
            </>
          )}
        </aside>

        {onglet === "creations" ? (
          <Creations
            ouvrirConversation={(conversationId) => ouvrir(conversationId)}
            ouvrirNote={ouvrirNote}
            ecritureAutorisee={ecritureAutorisee}
          />
        ) : !ouverte ? (
          <div className="carte etat-vide">
            <MessagesSquare size={30} strokeWidth={1.6} aria-hidden />
            <p>
              Commence une <strong>nouvelle conversation</strong> : choisis les notes sur lesquelles l'IA doit s'appuyer,
              puis pose ta question (« Explique-moi le brassage interchromosomique », « Pourquoi la dérivée de x² est
              2x ? »…). Tes conversations sont enregistrées : tu les retrouves ici même après avoir fermé Révisio.
            </p>
          </div>
        ) : (
          <section className="carte discussion-conversation" aria-label={ouverte.conversation.titre}>
            <header className="conversation-entete">
              {renommage !== null ? (
                <form
                  className="rangee"
                  onSubmit={(e) => {
                    e.preventDefault();
                    renommer();
                  }}
                >
                  <input className="champ" autoFocus value={renommage} onChange={(e) => setRenommage(e.target.value)} />
                  <button className="bouton bouton-principal" type="submit">
                    Renommer
                  </button>
                  <button className="lien-discret petit" type="button" onClick={() => setRenommage(null)}>
                    Annuler
                  </button>
                </form>
              ) : (
                <h2 className="titre-bloc">
                  {ouverte.conversation.titre}
                  <button
                    className="bouton-icone"
                    aria-label="Renommer la conversation"
                    onClick={() => setRenommage(ouverte.conversation.titre)}
                  >
                    <Pencil size={14} aria-hidden />
                  </button>
                </h2>
              )}
              <div className="rangee">
                <button className="bouton bouton-petit" onClick={() => setChoixSources("modifier")}>
                  <BookMarked size={14} aria-hidden /> Sources : {resumeSources(ouverte.conversation.sources)}
                </button>
                {aSupprimer ? (
                  <>
                    <button className="bouton bouton-danger bouton-petit" onClick={supprimer}>
                      <Trash size={14} aria-hidden /> Supprimer la conversation
                    </button>
                    <button className="lien-discret petit" onClick={() => setASupprimer(false)}>
                      Annuler
                    </button>
                  </>
                ) : (
                  <button className="lien-discret petit" onClick={() => setASupprimer(true)}>
                    <Trash size={13} aria-hidden /> Supprimer
                  </button>
                )}
              </div>
            </header>

            <div className="fil-messages">
              {messages.length === 0 && !enAttente && (
                <p className="texte-doux centre petit">
                  Pose ta première question. L'IA cite tes notes ([E1]…) ; ce qui n'y est pas est encadré « Hors de tes
                  notes ».
                </p>
              )}
              {messages.map((m) =>
                m.ia ? (
                  <ReponseIa
                    key={m.id}
                    texte={m.texte}
                    extraits={m.extraits}
                    creations={ouverte.creations.filter((c) => c.messageId === m.id)}
                    modele={m.modele}
                    ouvrirNote={ouvrirNote}
                    onDecider={decider}
                  />
                ) : (
                  <Question key={m.id} message={m} />
                ),
              )}
              {enAttente && (messages.length === 0 || messages[messages.length - 1].texte !== enAttente) && (
                <div className="bulle bulle-eleve">{enAttente}</div>
              )}
              {enCours &&
                (flux ? (
                  <ReponseIa texte={flux} extraits={[]} creations={[]} enCours ouvrirNote={ouvrirNote} onDecider={decider} />
                ) : (
                  <p className="progression-texte">{moteur.local ? "L'IA locale réfléchit… (plus lent)" : "L'IA réfléchit…"}</p>
                ))}
              {tache?.statut === "erreur" && !enCours && <p className="message-erreur">{tache.erreur}</p>}
              {tache?.statut === "interrompue" && !enCours && <Message genre="info">Réponse interrompue.</Message>}
              {sansReponse && (
                <div className="rangee centre">
                  <button className="bouton" onClick={reessayer} disabled={!moteur.simple}>
                    <RotateCcw size={15} aria-hidden /> Réessayer de répondre
                  </button>
                </div>
              )}
              <div ref={bas} />
            </div>

            <form
              className="zone-question"
              onSubmit={(e) => {
                e.preventDefault();
                envoyer();
              }}
            >
              <textarea
                className="champ"
                rows={2}
                placeholder={moteur.simple ? "Ta question… (Entrée pour envoyer, Maj + Entrée pour aller à la ligne)" : "IA indisponible pour le moment"}
                value={question}
                disabled={!moteur.simple}
                onChange={(e) => setQuestion(e.target.value)}
                onKeyDown={(e) => {
                  if (e.key === "Enter" && !e.shiftKey) {
                    e.preventDefault();
                    envoyer();
                  }
                }}
              />
              <div className="rangee">
                <BadgeIa moteur={moteur} />
                <span className="espace" />
                <BoutonArreter tache={tache} />
                <button className="bouton bouton-principal" type="submit" disabled={!question.trim() || enCours || !moteur.simple}>
                  <SendHorizontal size={16} aria-hidden /> Envoyer
                </button>
              </div>
              {!moteur.simple && (
                <p className="texte-doux petit">
                  {!moteur.enLigne
                    ? "Hors ligne : la discussion demande Internet, ou l'IA locale (Réglages). Tes conversations restent lisibles."
                    : moteur.etat && !moteur.etat.clePresente
                      ? "Aucune clé Gemini."
                      : "IA indisponible."}{" "}
                  {moteur.enLigne && moteur.etat && !moteur.etat.clePresente && (
                    <button className="lien-discret" type="button" onClick={onReglages}>
                      <KeyRound size={13} aria-hidden /> Ajouter une clé
                    </button>
                  )}
                </p>
              )}
            </form>
          </section>
        )}
      </div>
      {erreur && <p className="message-erreur">{erreur}</p>}

      {choixSources && (
        <ChoixSources
          titre={choixSources === "nouvelle" ? "Nouvelle conversation : sur quelles notes ?" : "Sources de la conversation"}
          sources={choixSources === "modifier" ? (ouverte?.conversation.sources ?? []) : []}
          onValider={choixSources === "nouvelle" ? nouvelle : changerSources}
          onFermer={() => setChoixSources(null)}
        />
      )}
    </div>
  );
}
