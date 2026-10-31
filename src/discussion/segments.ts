// Découpe d'une réponse de l'IA : texte (Markdown) et visuels proposés (blocs ```mermaid et ```graphique).
// Pendant l'écriture, un bloc pas encore refermé est marqué « incomplet ».
import type { ExtraitCite } from "../lib/api";

export type Segment =
  | { genre: "texte"; texte: string }
  | { genre: "diagramme" | "graphique"; contenu: string; indice: number; complet: boolean };

const BLOC = /```(mermaid|graphique)[ \t]*\r?\n([\s\S]*?)(```|$)/g;

export function decouper(texte: string): Segment[] {
  const segments: Segment[] = [];
  let debut = 0;
  let indice = 0;
  for (const m of texte.matchAll(BLOC)) {
    const avant = texte.slice(debut, m.index);
    if (avant.trim()) segments.push({ genre: "texte", texte: avant });
    segments.push({
      genre: m[1] === "mermaid" ? "diagramme" : "graphique",
      contenu: m[2].trim(),
      indice: indice++,
      complet: m[3] === "```",
    });
    debut = m.index + m[0].length;
  }
  const reste = texte.slice(debut);
  if (reste.trim()) segments.push({ genre: "texte", texte: reste });
  return segments;
}

/** [E3] ou [E3, E5] → liens vers les notes citées (les numéros inconnus restent du texte). */
export function lierCitations(texte: string, extraits: ExtraitCite[]): string {
  return texte.replace(/\[(E\d+(?:\s*[,;]\s*E\d+)*)\]/g, (_, liste: string) =>
    liste
      .split(/\s*[,;]\s*/)
      .map((id) => {
        const e = extraits.find((x) => x.id === id);
        return e ? `[[${e.chemin.replace(/\.md$/i, "")}|${id}]]` : id;
      })
      .join(" "),
  );
}
