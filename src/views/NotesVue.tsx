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
}

export default function NotesVue({ chemin, ancre, peutRevenir, onOuvrir, onRetour, indexation, ecritureAutorisee }: Props) {
  return (
    <div className="notes">
      <aside className="panneau-arbre">
        <ArbreNotes cheminActif={chemin} onOuvrir={onOuvrir} indexation={indexation} />
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
