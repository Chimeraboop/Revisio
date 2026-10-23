// Planning des révisions avant les évaluations (fonctions pures, testées dans planning.test.ts).
//
// Pour chaque évaluation, les chapitres sont répartis sur les jours restants, les plus faibles
// d'abord : deux passages par chapitre si le temps le permet, puis une révision générale la veille.
// Le planning est recalculé à chaque affichage : il suit la maîtrise réelle des chapitres.
import type { CarteStat, ChapitreChoisi, Evaluation } from "../lib/api";
import { retention } from "../revision/fsrs";

export interface InfoChapitre {
  /** Rétention moyenne (0 à 1) ; une carte jamais vue compte 0. */
  maitrise: number;
  total: number;
  /** Cartes de vocabulaire (séance « Langues ») parmi `total`. */
  vocabulaire: number;
}

export type NatureElement = "chapitre" | "revision_generale" | "jour_j";

export interface ElementPlan {
  evaluation: Evaluation;
  chapitre: string | null;
  nature: NatureElement;
  /** 1 = premier passage sur le chapitre, 2 = deuxième passage. */
  passage: number;
}

export interface JourPlan {
  jour: string;
  elements: ElementPlan[];
}

const UN_JOUR = 86_400_000;

function versDate(jour: string): number {
  const [a, m, j] = jour.split("-").map(Number);
  return Date.UTC(a, m - 1, j);
}

/** Nombre de jours de `a` à `b` (dates AAAA-MM-JJ). */
export function ecartJours(a: string, b: string): number {
  return Math.round((versDate(b) - versDate(a)) / UN_JOUR);
}

export function ajouterJours(jour: string, n: number): string {
  return new Date(versDate(jour) + n * UN_JOUR).toISOString().slice(0, 10);
}

export const cleChapitre = (matiere: string | null, chapitre: string | null) => `${matiere ?? ""}|${chapitre ?? ""}`;

/** Maîtrise et nombre de cartes de chaque chapitre (clé `cleChapitre`). */
export function infoParChapitre(cartes: CarteStat[], maintenant = new Date()): Map<string, InfoChapitre> {
  const infos = new Map<string, InfoChapitre>();
  for (const c of cartes) {
    const cle = cleChapitre(c.matiere, c.chapitre);
    const i = infos.get(cle) ?? { maitrise: 0, total: 0, vocabulaire: 0 };
    i.maitrise += retention(c.etatFsrs, maintenant);
    i.total += 1;
    if (c.genre === "vocabulaire") i.vocabulaire += 1;
    infos.set(cle, i);
  }
  for (const i of infos.values()) i.maitrise = i.total ? i.maitrise / i.total : 0;
  return infos;
}

/**
 * Répartit les chapitres (déjà triés, les plus faibles d'abord) sur `jours` jours.
 * - assez de jours pour deux passages : on n'utilise que les derniers jours (pas de révision trop tôt) ;
 * - assez pour un passage : les plus faibles ont un deuxième passage ;
 * - pas assez : plusieurs chapitres par jour.
 */
export function repartir<T>(ordre: T[], jours: number): T[][] {
  const n = ordre.length;
  if (jours <= 0 || n === 0) return [];
  if (jours >= 2 * n) {
    const vides: T[][] = Array.from({ length: jours - 2 * n }, () => []);
    return [...vides, ...[...ordre, ...ordre].map((c) => [c])];
  }
  if (jours >= n) return [...ordre, ...ordre.slice(0, jours - n)].map((c) => [c]);
  return Array.from({ length: jours }, (_, k) => ordre.slice(Math.floor((k * n) / jours), Math.floor(((k + 1) * n) / jours)));
}

export function planifier(
  evaluations: Evaluation[],
  infos: Map<string, InfoChapitre>,
  aujourdhui: string,
): JourPlan[] {
  const parJour = new Map<string, ElementPlan[]>();
  const ajouter = (jour: string, e: ElementPlan) => parJour.set(jour, [...(parJour.get(jour) ?? []), e]);

  for (const evaluation of [...evaluations].sort((a, b) => a.jour.localeCompare(b.jour))) {
    const reste = ecartJours(aujourdhui, evaluation.jour);
    if (reste < 0) continue;
    if (reste === 0) {
      for (const chapitre of evaluation.chapitres) ajouter(aujourdhui, { evaluation, chapitre, nature: "jour_j", passage: 1 });
      continue;
    }
    const info = (c: string | null) => infos.get(cleChapitre(evaluation.matiere, c)) ?? { maitrise: 0, total: 0, vocabulaire: 0 };
    const ordre = [...evaluation.chapitres].sort(
      (a, b) => info(a).maitrise - info(b).maitrise || info(b).total - info(a).total || (a ?? "").localeCompare(b ?? "", "fr"),
    );
    // Jours disponibles : d'aujourd'hui à l'avant-veille ; la veille est réservée à la révision générale.
    const passages = new Map<string | null, number>();
    repartir(ordre, reste - 1).forEach((chapitres, k) => {
      for (const chapitre of chapitres) {
        const passage = (passages.get(chapitre) ?? 0) + 1;
        passages.set(chapitre, passage);
        ajouter(ajouterJours(aujourdhui, k), { evaluation, chapitre, nature: "chapitre", passage });
      }
    });
    for (const chapitre of evaluation.chapitres) {
      ajouter(ajouterJours(evaluation.jour, -1), { evaluation, chapitre, nature: "revision_generale", passage: 1 });
    }
  }
  return [...parJour.entries()].sort(([a], [b]) => a.localeCompare(b)).map(([jour, elements]) => ({ jour, elements }));
}

/** Le chapitre a-t-il des cartes dans cette séance du jour ? (sans carte : il compte dans « Autres »). */
export function concerneGroupe(info: InfoChapitre | undefined, groupe: "langues" | "autres"): boolean {
  if (groupe === "langues") return (info?.vocabulaire ?? 0) > 0;
  return !info || info.total > info.vocabulaire;
}

/** Chapitres à réviser aujourd'hui (sans doublon), pour la séance du jour. */
export function chapitresDuJour(plan: JourPlan[], aujourdhui: string): ChapitreChoisi[] {
  const vus = new Map<string, ChapitreChoisi>();
  for (const e of plan.find((j) => j.jour === aujourdhui)?.elements ?? []) {
    vus.set(cleChapitre(e.evaluation.matiere, e.chapitre), { matiere: e.evaluation.matiere, chapitre: e.chapitre });
  }
  return [...vus.values()];
}

const formatJour = new Intl.DateTimeFormat("fr-FR", { weekday: "long", day: "numeric", month: "long", timeZone: "UTC" });

/** « Aujourd'hui », « Demain » ou « mardi 21 octobre ». */
export function libelleJour(jour: string, aujourdhui: string): string {
  const ecart = ecartJours(aujourdhui, jour);
  if (ecart === 0) return "Aujourd'hui";
  if (ecart === 1) return "Demain";
  const t = formatJour.format(new Date(versDate(jour)));
  return t.charAt(0).toUpperCase() + t.slice(1);
}

/** « aujourd'hui », « demain », « dans 5 jours », « il y a 2 jours ». */
export function compteARebours(jour: string, aujourdhui: string): string {
  const ecart = ecartJours(aujourdhui, jour);
  if (ecart === 0) return "aujourd'hui";
  if (ecart === 1) return "demain";
  if (ecart > 1) return `dans ${ecart} jours`;
  return ecart === -1 ? "hier" : `il y a ${-ecart} jours`;
}
