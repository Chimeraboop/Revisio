import { useState } from "react";
import { ArrowLeft, BookOpen } from "lucide-react";
import { EtatIndexation } from "../lib/api";
import ArbreNotes from "./ArbreNotes";
import LecteurNote from "./LecteurNote";
import { TOUCHE_ALT } from "../lib/plateforme";

interface Props {
  chemin: string | null;
  ancre: string | null;
  peutRevenir: boolean;
  onOuvrir: (chemin: string, ancre?: string | null) => void;
  onRetour: () => void;
  indexation: EtatIndexation | null;
  ecritureAutorisee: boolean;
  onAutoriser: () => void;
  /** Une note ouverte a changé de place (renommée, déplacée) ou est partie à la corbeille (`null`). */
  onRemplacer: (ancien: string, nouveau: string | null) => void;
}

export default function NotesVue(props: Props) {
  const { chemin, ancre, peutRevenir, onOuvrir, onRetour, indexation, ecritureAutorisee } = props;
  // Note qui vient d'être créée : elle s'ouvre directement en mode édition.
  const [aEditer, setAEditer] = useState<string | null>(null);
  return (
    <div className="notes">
      <aside className="panneau-arbre">
        <ArbreNotes
          cheminActif={chemin}
          onOuvrir={onOuvrir}
          indexation={indexation}
          ecritureAutorisee={ecritureAutorisee}
          onAutoriser={props.onAutoriser}
          onNoteCreee={(c) => {
            setAEditer(c);
            onOuvrir(c);
          }}
          onDeplace={(ancien, nouveau) => {
            if (!chemin) return;
            if (chemin === ancien) props.onRemplacer(ancien, nouveau);
            else if (chemin.startsWith(ancien + "/")) props.onRemplacer(chemin, nouveau + chemin.slice(ancien.length));
          }}
          onSupprime={(c) => {
            if (chemin && (chemin === c || chemin.startsWith(c + "/"))) props.onRemplacer(chemin, null);
          }}
        />
      </aside>
      <section className="panneau-note">
        <div className="barre-note">
          <button className="bouton" disabled={!peutRevenir} onClick={onRetour} title={`Retour (${TOUCHE_ALT} + ←)`}>
            <ArrowLeft size={15} aria-hidden /> Retour
          </button>
        </div>
        {chemin ? (
          <LecteurNote
            chemin={chemin}
            ancre={ancre}
            onOuvrir={onOuvrir}
            versionIndex={indexation?.dernier}
            ecritureAutorisee={ecritureAutorisee}
            ouvrirEnEdition={aEditer === chemin}
            onEditionOuverte={() => setAEditer(null)}
          />
        ) : (
          <div className="etat-vide etat-vide-plein">
            <BookOpen size={32} strokeWidth={1.6} aria-hidden />
            <p>Choisis une note dans la liste.</p>
          </div>
        )}
      </section>
    </div>
  );
}
