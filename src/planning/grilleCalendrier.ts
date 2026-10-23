// Calendrier du planning (fonctions pures, testées dans grilleCalendrier.test.ts) : grille d'un mois
// (semaines du lundi au dimanche) et couleur stable de chaque matière.
import { ajouterJours } from "./planning";

export interface Mois {
  annee: number;
  /** 0 = janvier. */
  mois: number;
}

const pad = (n: number) => String(n).padStart(2, "0");

export function moisDe(jour: string): Mois {
  const [a, m] = jour.split("-").map(Number);
  return { annee: a, mois: m - 1 };
}

export function decalerMois({ annee, mois }: Mois, n: number): Mois {
  const total = annee * 12 + mois + n;
  return { annee: Math.floor(total / 12), mois: ((total % 12) + 12) % 12 };
}

/** Les 6 semaines affichées pour ce mois : 42 jours (AAAA-MM-JJ), du lundi précédant le 1er. */
export function grilleMois({ annee, mois }: Mois): string[] {
  const premier = `${annee}-${pad(mois + 1)}-01`;
  // getUTCDay : 0 = dimanche … 6 = samedi ; on commence la semaine le lundi.
  const decalage = (new Date(Date.UTC(annee, mois, 1)).getUTCDay() + 6) % 7;
  const debut = ajouterJours(premier, -decalage);
  return Array.from({ length: 42 }, (_, i) => ajouterJours(debut, i));
}

export function dansLeMois(jour: string, { annee, mois }: Mois): boolean {
  return jour.startsWith(`${annee}-${pad(mois + 1)}-`);
}

/** « Chapitre 4 - Les mutations » → « Ch. 4 · Les mutations » (les cases du calendrier sont étroites). */
export function abregerChapitre(chapitre: string): string {
  return chapitre.replace(/^Chapitre\s*(\d+)\s*[-–—:]\s*/i, "Ch. $1 · ");
}

/** Teintes des matières : lisibles sur fond clair comme sombre. */
const TEINTES = ["#3b82f6", "#22c55e", "#f59e0b", "#ef4444", "#a855f7", "#14b8a6", "#ec4899", "#f97316", "#64748b"];

/** Même matière → même couleur, partout et à chaque lancement. */
export function couleurMatiere(matiere: string): string {
  let h = 0;
  for (const c of matiere) h = (h * 31 + c.codePointAt(0)!) >>> 0;
  return TEINTES[h % TEINTES.length];
}

const formatMois = new Intl.DateTimeFormat("fr-FR", { month: "long", year: "numeric", timeZone: "UTC" });

/** « Octobre 2026 ». */
export function nomMois({ annee, mois }: Mois): string {
  const t = formatMois.format(new Date(Date.UTC(annee, mois, 1)));
  return t.charAt(0).toUpperCase() + t.slice(1);
}
