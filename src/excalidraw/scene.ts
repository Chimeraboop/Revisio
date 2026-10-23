// Lecture des dessins Excalidraw stockés dans les notes « (schéma).excalidraw.md ».
// Deux formats existent dans le vault :
// - bloc ```compressed-json (compression LZ-String, en base 64, coupé en lignes) ;
// - bloc ```json non compressé.
import LZString from "lz-string";

export interface ElementExcalidraw {
  id: string;
  type: string;
  x: number;
  y: number;
  isDeleted?: boolean;
  text?: string;
  originalText?: string;
  [cle: string]: unknown;
}

export interface Scene {
  elements: ElementExcalidraw[];
  files: Record<string, unknown> | null;
}

function bloc(markdown: string, langage: string): string | null {
  const debut = markdown.indexOf("```" + langage);
  if (debut < 0) return null;
  const apres = markdown.indexOf("\n", debut);
  const fin = markdown.indexOf("```", apres);
  if (apres < 0 || fin < 0) return null;
  return markdown.slice(apres + 1, fin);
}

/** Extrait la scène d'une note Excalidraw ; `null` si le dessin est illisible. */
export function extraireScene(markdown: string): Scene | null {
  let json: string | null = null;
  const compresse = bloc(markdown, "compressed-json");
  if (compresse !== null) {
    json = LZString.decompressFromBase64(compresse.replace(/\s+/g, "")) || null;
  } else {
    json = bloc(markdown, "json");
  }
  if (!json) return null;
  try {
    const dessin = JSON.parse(json);
    if (!Array.isArray(dessin.elements)) return null;
    return {
      elements: dessin.elements.filter((e: ElementExcalidraw) => !e.isDeleted),
      files: dessin.files ?? null,
    };
  } catch {
    return null;
  }
}

/** Texte d'un élément, espaces et retours à la ligne simplifiés. */
export function texteElement(e: ElementExcalidraw): string {
  return String(e.originalText ?? e.text ?? "").replace(/\s+/g, " ").trim();
}

export function normaliserLegende(t: string): string {
  return t.replace(/\s+/g, " ").trim().toLowerCase();
}

/** Ordre de lecture : de haut en bas, puis de gauche à droite (par bandes de 40 px). */
export function ordreDeLecture(a: ElementExcalidraw, b: ElementExcalidraw): number {
  const bandeA = Math.round(a.y / 40);
  const bandeB = Math.round(b.y / 40);
  return bandeA - bandeB || a.x - b.x;
}

export interface Trou {
  numero: number;
  attendu: string;
}

/**
 * Prépare un schéma muet : les textes qui sont des légendes à retrouver sont remplacés
 * par leur numéro. Renvoie la scène modifiée et la liste des trous.
 */
export function masquerLegendes(scene: Scene, legendes: string[]): { scene: Scene; trous: Trou[] } {
  const aMasquer = new Set(legendes.map(normaliserLegende));
  const candidats = scene.elements
    .filter((e) => e.type === "text" && aMasquer.has(normaliserLegende(texteElement(e))))
    .sort(ordreDeLecture);
  const numeros = new Map<string, number>();
  const trous: Trou[] = candidats.map((e, i) => {
    numeros.set(e.id, i + 1);
    return { numero: i + 1, attendu: texteElement(e) };
  });
  const elements = scene.elements.map((e) => {
    const n = numeros.get(e.id);
    if (n === undefined) return e;
    const etiquette = `(${n})`;
    return { ...e, text: etiquette, originalText: etiquette, strokeColor: "#3d5ccc" };
  });
  return { scene: { ...scene, elements }, trous };
}
