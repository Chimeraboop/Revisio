// Arbre de Révisio → données Markmap (fonctions pures, testées dans conversion.test.ts).
import katex from "katex";
import type { IPureNode } from "markmap-common";
import type { Noeud } from "../lib/api";

export function echapper(t: string): string {
  return t.replace(/&/g, "&amp;").replace(/</g, "&lt;").replace(/>/g, "&gt;").replace(/"/g, "&quot;");
}

/** Texte d'un nœud en HTML : texte échappé, formules $…$ rendues par KaTeX. */
export function texteEnHtml(texte: string): string {
  return texte
    .split(/(\$[^$]+\$)/g)
    .map((morceau) =>
      /^\$[^$]+\$$/.test(morceau)
        ? katex.renderToString(morceau.slice(1, -1), { throwOnError: false, output: "html" })
        : echapper(morceau),
    )
    .join("");
}

/** Icône « ouvrir » (Lucide external-link), dessinée dans le nœud. */
const ICONE_OUVRIR = '<svg xmlns="http://www.w3.org/2000/svg" width="13" height="13" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2.2" stroke-linecap="round" stroke-linejoin="round" aria-hidden="true"><path d="M15 3h6v6"/><path d="M10 14 21 3"/><path d="M18 13v6a2 2 0 0 1-2 2H5a2 2 0 0 1-2-2V8a2 2 0 0 1 2-2h6"/></svg>';

/** Arbre de Révisio → données Markmap (le contenu de chaque nœud est du HTML). */
export function versMarkmap(n: Noeud): IPureNode {
  const detail = n.detail ? ` title="${echapper(n.detail)}"` : "";
  const lien = n.note ? ` <a class="lien-noeud" data-chemin="${echapper(n.note)}" title="Ouvrir la note">${ICONE_OUVRIR}</a>` : "";
  return {
    content: `<span${detail}>${texteEnHtml(n.texte)}</span>${lien}`,
    children: n.enfants.map(versMarkmap),
  };
}
