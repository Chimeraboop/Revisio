// Affichage des messages d'une conversation : questions de l'élève, réponses de l'IA (Markdown, formules,
// citations cliquables, encadrés « hors de tes notes ») et visuels proposés à garder ou non.
import { useState } from "react";
import { BookOpen, Check, Eye, Network, ChartLine, X } from "lucide-react";
import type { CreationIa, ExtraitCite, MessageConversation } from "../lib/api";
import { nomModele } from "../lib/moteurIa";
import { MarkdownObsidian } from "../markdown/rendu";
import Diagramme from "./Diagramme";
import GraphiqueIa from "./GraphiqueIa";
import { decouper, lierCitations } from "./segments";

/** Un diagramme ou un graphique, dessiné. */
export function Visuel({ genre, contenu }: { genre: "diagramme" | "graphique"; contenu: string }) {
  return genre === "diagramme" ? <Diagramme code={contenu} /> : <GraphiqueIa contenu={contenu} />;
}

const NOMS = { diagramme: "schéma", graphique: "graphique" } as const;

/** Visuel proposé par l'IA : aperçu flouté tant que l'élève n'a pas choisi de le garder. */
function Proposition({
  genre,
  contenu,
  creation,
  onDecider,
}: {
  genre: "diagramme" | "graphique";
  contenu: string;
  creation: CreationIa | undefined;
  onDecider: (id: number, garder: boolean) => void;
}) {
  const [voir, setVoir] = useState(false);
  const Icone = genre === "diagramme" ? Network : ChartLine;
  const statut = creation?.statut ?? "proposee";
  if (statut === "refusee" && !voir)
    return (
      <div className="proposition proposition-refusee petit">
        <Icone size={15} className="icone" aria-hidden /> {creation?.titre ?? "Visuel"} : pas gardé.
        <button className="lien-discret" onClick={() => setVoir(true)}>
          <Eye size={13} aria-hidden /> Voir quand même
        </button>
        {creation && (
          <button className="lien-discret" onClick={() => onDecider(creation.id, true)}>
            <Check size={13} aria-hidden /> Le garder finalement
          </button>
        )}
      </div>
    );
  return (
    <figure className={`proposition statut-${statut}`}>
      <figcaption className="proposition-titre">
        <Icone size={16} className="icone" aria-hidden />
        <strong>{creation?.titre ?? `Un ${NOMS[genre]}`}</strong>
        {statut === "acceptee" && (
          <span className="texte-doux petit">
            <Check size={13} aria-hidden /> Gardé dans tes créations
          </span>
        )}
      </figcaption>
      <div className="proposition-visuel">
        <Visuel genre={genre} contenu={contenu} />
      </div>
      {statut === "proposee" && creation && (
        <div className="proposition-choix">
          <p className="petit">L'IA te propose ce {NOMS[genre]}. Tu le gardes ?</p>
          <div className="rangee centre">
            <button className="bouton bouton-principal" onClick={() => onDecider(creation.id, true)}>
              <Check size={15} aria-hidden /> Garder
            </button>
            <button className="bouton" onClick={() => onDecider(creation.id, false)}>
              <X size={15} aria-hidden /> Non merci
            </button>
          </div>
        </div>
      )}
    </figure>
  );
}

interface PropsReponse {
  texte: string;
  extraits: ExtraitCite[];
  creations: CreationIa[];
  /** Réponse en train de s'écrire : les visuels ne sont pas encore proposés. */
  enCours?: boolean;
  modele?: string | null;
  ouvrirNote: (chemin: string) => void;
  onDecider: (id: number, garder: boolean) => void;
}

export function ReponseIa({ texte, extraits, creations, enCours, modele, ouvrirNote, onDecider }: PropsReponse) {
  const navigation = { source: "", ouvrirNote };
  return (
    <div className="bulle bulle-ia">
      {decouper(texte).map((s, i) =>
        s.genre === "texte" ? (
          <MarkdownObsidian key={i} markdown={lierCitations(s.texte, extraits)} navigation={navigation} />
        ) : enCours || !s.complet ? (
          <p key={i} className="progression-texte">
            L'IA prépare un {NOMS[s.genre]}…
          </p>
        ) : (
          <Proposition
            key={i}
            genre={s.genre}
            contenu={s.contenu}
            creation={creations.find((c) => c.indice === s.indice)}
            onDecider={onDecider}
          />
        ),
      )}
      {enCours && <span className="curseur-ecriture" aria-hidden />}
      {!enCours && (extraits.length > 0 || modele) && (
        <div className="bulle-pied petit texte-doux">
          {extraits.length > 0 && (
            <span className="sources-citees">
              <BookOpen size={13} aria-hidden /> Sources :
              {extraits.map((e) => (
                <button key={e.id} className="puce-source" title={e.section || e.titre} onClick={() => ouvrirNote(e.chemin)}>
                  {e.id} · {e.titre}
                  {e.section && e.section !== e.titre ? ` › ${e.section}` : ""}
                </button>
              ))}
            </span>
          )}
          {modele && <span>{nomModele(modele)}</span>}
        </div>
      )}
    </div>
  );
}

export function Question({ message }: { message: MessageConversation }) {
  return <div className="bulle bulle-eleve">{message.texte}</div>;
}
