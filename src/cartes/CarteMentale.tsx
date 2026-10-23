// Affichage d'une carte mentale avec Markmap (zoom, déplacement, branches pliables), hors ligne.
// Les formules $…$ sont rendues avec KaTeX ; l'icône « ouvrir » d'un nœud ouvre sa note source.
import { ChevronsDownUp, ChevronsUpDown, ExternalLink, Maximize } from "lucide-react";
import { Markmap } from "markmap-view";
import { useEffect, useRef } from "react";
import type { Noeud } from "../lib/api";
import { versMarkmap } from "./conversion";

interface Props {
  arbre: Noeud;
  onOuvrirNote: (chemin: string) => void;
}

export default function CarteMentale({ arbre, onOuvrirNote }: Props) {
  const svg = useRef<SVGSVGElement>(null);
  const mm = useRef<Markmap | null>(null);
  const ouvrir = useRef(onOuvrirNote);
  ouvrir.current = onOuvrirNote;

  useEffect(() => {
    if (!svg.current) return;
    const carte = Markmap.create(svg.current, { duration: 300, maxWidth: 280, spacingVertical: 8, spacingHorizontal: 90 });
    mm.current = carte;
    // Un clic sur l'icône « ouvrir » ouvre la note (au lieu de plier la branche).
    const clic = (e: MouseEvent) => {
      const lien = (e.target as Element | null)?.closest?.("[data-chemin]") as HTMLElement | null;
      if (lien?.dataset.chemin) {
        e.preventDefault();
        e.stopPropagation();
        ouvrir.current(lien.dataset.chemin);
      }
    };
    svg.current.addEventListener("click", clic, true);
    const element = svg.current;
    return () => {
      element.removeEventListener("click", clic, true);
      carte.destroy();
      mm.current = null;
    };
  }, []);

  useEffect(() => {
    mm.current?.setData(versMarkmap(arbre), { initialExpandLevel: -1 }).then(() => mm.current?.fit());
  }, [arbre]);

  const replier = (niveau: number) =>
    mm.current?.setData(versMarkmap(arbre), { initialExpandLevel: niveau }).then(() => mm.current?.fit());

  return (
    <div className="carte-mentale">
      <div className="rangee barre-carte">
        <button className="bouton" onClick={() => replier(1)}>
          <ChevronsDownUp size={15} aria-hidden /> Replier
        </button>
        <button className="bouton" onClick={() => replier(-1)}>
          <ChevronsUpDown size={15} aria-hidden /> Tout déplier
        </button>
        <button className="bouton" onClick={() => mm.current?.fit()}>
          <Maximize size={15} aria-hidden /> Ajuster à l'écran
        </button>
        <span className="texte-doux petit">
          Molette : zoom · glisser : déplacer · clic sur un rond : plier ou déplier ·{" "}
          <ExternalLink size={12} className="icone" aria-label="icône ouvrir" /> : ouvrir la note
        </span>
      </div>
      <svg ref={svg} className="markmap" />
    </div>
  );
}
