// Petite fenêtre par-dessus l'écran (créer une note, déplacer…). Échap ou un clic à côté la ferment.
import { type ReactNode, useEffect, useRef } from "react";
import { X } from "lucide-react";

interface Props {
  titre: string;
  onFermer: () => void;
  children: ReactNode;
  /** Boutons du bas (à droite). */
  actions?: ReactNode;
}

export default function Fenetre({ titre, onFermer, children, actions }: Props) {
  const boite = useRef<HTMLDivElement>(null);

  useEffect(() => {
    const touche = (e: KeyboardEvent) => e.key === "Escape" && onFermer();
    window.addEventListener("keydown", touche);
    // Le premier champ reçoit le curseur.
    boite.current?.querySelector<HTMLElement>("input, select, textarea, button")?.focus();
    return () => window.removeEventListener("keydown", touche);
  }, [onFermer]);

  return (
    <div className="voile" onMouseDown={(e) => e.target === e.currentTarget && onFermer()}>
      <div className="fenetre carte" role="dialog" aria-modal="true" aria-label={titre} ref={boite}>
        <div className="rangee entre">
          <h2 className="titre-bloc">{titre}</h2>
          <button className="bouton-icone" onClick={onFermer} aria-label="Fermer">
            <X size={16} aria-hidden />
          </button>
        </div>
        <div className="fenetre-corps">{children}</div>
        {actions && <div className="fenetre-actions">{actions}</div>}
      </div>
    </div>
  );
}
