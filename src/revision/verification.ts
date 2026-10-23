// Vérification tolérante des réponses tapées, et note FSRS suggérée.
import type { Confiance } from "../lib/api";
import type { Note } from "./fsrs";

export type Verdict = "juste" | "presque" | "faux";

export interface Resultat {
  verdict: Verdict;
  /** Explication affichée (ex. « Il manque l'article »). */
  message?: string;
}

// Abréviations de dictionnaire qu'on n'exige pas de retaper.
const MOTS_OUTILS = new Set(["etw.", "etw", "jm", "jn", "jmdm", "jmdn", "qc", "qch", "qn", "qqn", "qqch", "sb", "sth", "+dat", "+acc", "+akk", "+datif", "+accusatif"]);

/** Forme sans exposants ni indices Unicode, pour comparer une réponse tapée au clavier (N0 = N₀). */
export function aplatir(texte: string): string {
  return texte
    .normalize("NFKC") // ² → 2, ₀ → 0, ᴬ → A, ½ → 1⁄2
    .replace(/⁄/g, "/")
    .replace(/[×✕]/g, "x")
    .replace(/[−–]/g, "-");
}

/** Forme de comparaison : minuscules, sans parenthèses, ponctuation, ni marques de dictionnaire. */
export function normaliser(texte: string): string {
  return aplatir(texte)
    .normalize("NFC")
    .toLowerCase()
    .replace(/\([^)]*\)/g, " ") // (überschritt, hat überschritten), (-n), (au pluriel)
    .replace(/\+\s+(dat|acc|akk|datif|accusatif)\b/g, "+$1")
    .replace(/(\p{L})\/(\p{L})/gu, "$1$2") // weg/fahren → wegfahren (préfixe séparable)
    .replace(/œ/g, "oe")
    .replace(/æ/g, "ae")
    .replace(/[*.,;:!?«»"“”…\-–—]/g, " ") // cellule-œuf = cellule oeuf
    .replace(/[’`]/g, "'")
    .split(/\s+/)
    .filter((mot) => mot && !MOTS_OUTILS.has(mot))
    .join(" ")
    .trim();
}

export function sansAccents(texte: string): string {
  return texte.normalize("NFD").replace(/\p{Diacritic}/gu, "").replace(/ß/g, "ss");
}

/** Réponses acceptées : la réponse entière et chacune de ses variantes (« a, b », « a / b », « a ; b »). */
export function variantes(attendue: string): string[] {
  const morceaux: string[] = [];
  let profondeur = 0;
  let courant = "";
  for (let i = 0; i < attendue.length; i++) {
    const c = attendue[i];
    if (c === "(") profondeur++;
    if (c === ")") profondeur--;
    const separateur = profondeur === 0 && (c === "," || c === ";" || (c === "/" && attendue[i - 1] === " "));
    if (separateur) {
      morceaux.push(courant);
      courant = "";
    } else courant += c;
  }
  morceaux.push(courant);
  const liste = [attendue, ...morceaux].map((m) => m.trim()).filter(Boolean);
  return [...new Set(liste)];
}

/** Distance d'édition (nombre de lettres à changer, ajouter ou enlever). */
export function distance(a: string, b: string): number {
  const ligne = Array.from({ length: b.length + 1 }, (_, j) => j);
  for (let i = 1; i <= a.length; i++) {
    let precedent = ligne[0];
    ligne[0] = i;
    for (let j = 1; j <= b.length; j++) {
      const temp = ligne[j];
      ligne[j] = Math.min(ligne[j] + 1, ligne[j - 1] + 1, precedent + (a[i - 1] === b[j - 1] ? 0 : 1));
      precedent = temp;
    }
  }
  return ligne[b.length];
}

const ARTICLES = /^(der|die|das|den|dem|des|the|a|an|le|la|les|l'|un|une|du|de la|de l')\s*/;

export function verifier(donnee: string, attendue: string): Resultat {
  const d = normaliser(donnee);
  if (!d) return { verdict: "faux", message: "Pas de réponse." };
  const liste = variantes(attendue);
  const complete = (alt: string) => (alt === attendue ? undefined : `Réponse complète : ${attendue}`);

  for (const alt of liste) {
    const a = normaliser(alt);
    if (a === d) return { verdict: "juste", message: complete(alt) };
    // « la membrane plasmique » pour « membrane plasmique » : l'article ajouté n'est pas une faute.
    if (!ARTICLES.test(a) && d.replace(ARTICLES, "") === a) return { verdict: "juste", message: complete(alt) };
  }
  for (const alt of liste) {
    const a = normaliser(alt);
    const articleAttendu = a.match(ARTICLES)?.[0].trim();
    const sansArticle = a.replace(ARTICLES, "");
    if (articleAttendu && d.replace(ARTICLES, "") === sansArticle && d !== a && d !== sansArticle) {
      return { verdict: "faux", message: `Mauvais article : c'est « ${articleAttendu} ».` };
    }
    // On compare avec et sans l'article attendu ; un article ajouté (non attendu) est ignoré.
    const dc = articleAttendu ? d : d.replace(ARTICLES, "");
    const formes: [string, boolean][] = articleAttendu ? [[a, false], [sansArticle, true]] : [[a, false]];
    for (const [forme, articleOublie] of formes) {
      const oubli = articleOublie ? `Il manque l'article : « ${articleAttendu} ». ` : "";
      if (forme === dc) return { verdict: "presque", message: oubli.trim() };
      if (sansAccents(forme) === sansAccents(dc)) {
        return { verdict: "presque", message: `${oubli}Attention aux accents (ou aux trémas) : « ${alt} ».` };
      }
      const tolerance = forme.length <= 5 ? 0 : forme.length <= 10 ? 1 : 2;
      if (tolerance > 0 && distance(sansAccents(forme), sansAccents(dc)) <= tolerance) {
        return { verdict: "presque", message: `${oubli}Petite faute d'orthographe : « ${alt} ».` };
      }
    }
  }
  return { verdict: "faux" };
}

/**
 * Note FSRS suggérée selon le résultat et la confiance annoncée.
 * Règle du cahier des charges : une bonne réponse « au hasard » est à revoir.
 */
export function noteSuggeree(verdict: Verdict, confiance: Confiance): Note {
  if (verdict === "faux" || confiance === "hasard") return 1;
  if (verdict === "presque" || confiance === "hesitant") return 2;
  return 3;
}

/** Pour une carte retournée (auto-notation) : une note ≥ 2 annoncée « au hasard » devient 1. */
export function noteAutoEvaluee(note: Note, confiance: Confiance): Note {
  return confiance === "hasard" && note >= 2 ? 1 : note;
}
