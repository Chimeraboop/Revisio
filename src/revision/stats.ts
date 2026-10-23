// Calculs du tableau de bord (fonctions pures, testées dans stats.test.ts).
import type { CarteStat, JourActivite } from "../lib/api";
import { retention } from "./fsrs";

export interface Maitrise {
  /** Rétention moyenne (0 à 1) ; une carte jamais vue compte 0. */
  moyenne: number;
  vues: number;
  total: number;
}

export interface MaitriseMatiere extends Maitrise {
  matiere: string;
  chapitres: (Maitrise & { chapitre: string })[];
}

export type Statut = "solide" | "fragile" | "a-revoir" | "pas-commence";

export const LIBELLES_STATUT: Record<Statut, string> = {
  solide: "Solide",
  fragile: "Fragile",
  "a-revoir": "À revoir",
  "pas-commence": "Pas commencé",
};

/** Vert à partir de 80 % de rétention moyenne, orange à partir de 50 %, rouge en dessous. */
export function statut(m: Maitrise): Statut {
  if (m.vues === 0) return "pas-commence";
  if (m.moyenne >= 0.8) return "solide";
  if (m.moyenne >= 0.5) return "fragile";
  return "a-revoir";
}

function ajouter(m: Maitrise, r: number, vue: boolean) {
  m.total += 1;
  m.vues += vue ? 1 : 0;
  m.moyenne += r; // somme pour l'instant, divisée à la fin
}

export function maitriseParMatiere(cartes: CarteStat[], maintenant = new Date()): MaitriseMatiere[] {
  const matieres = new Map<string, { m: Maitrise; chapitres: Map<string, Maitrise> }>();
  for (const c of cartes) {
    const nomMatiere = c.matiere ?? "Hors matière";
    const nomChapitre = c.chapitre ?? "Notes générales";
    let mat = matieres.get(nomMatiere);
    if (!mat) {
      mat = { m: { moyenne: 0, vues: 0, total: 0 }, chapitres: new Map() };
      matieres.set(nomMatiere, mat);
    }
    let chap = mat.chapitres.get(nomChapitre);
    if (!chap) {
      chap = { moyenne: 0, vues: 0, total: 0 };
      mat.chapitres.set(nomChapitre, chap);
    }
    const r = retention(c.etatFsrs, maintenant);
    ajouter(mat.m, r, !!c.etatFsrs);
    ajouter(chap, r, !!c.etatFsrs);
  }
  const finir = <T extends Maitrise>(m: T): T => ({ ...m, moyenne: m.total ? m.moyenne / m.total : 0 });
  const trieur = new Intl.Collator("fr", { numeric: true });
  return [...matieres.entries()]
    .map(([matiere, { m, chapitres }]) => ({
      ...finir(m),
      matiere,
      chapitres: [...chapitres.entries()]
        .map(([chapitre, c]) => ({ ...finir(c), chapitre }))
        .sort((a, b) => trieur.compare(a.chapitre, b.chapitre)),
    }))
    .sort((a, b) => trieur.compare(a.matiere, b.matiere));
}

/** Jour local au format AAAA-MM-JJ (comme renvoyé par la base). */
export function jourLocal(d: Date): string {
  const p = (n: number) => String(n).padStart(2, "0");
  return `${d.getFullYear()}-${p(d.getMonth() + 1)}-${p(d.getDate())}`;
}

function veille(d: Date): Date {
  const v = new Date(d);
  v.setDate(v.getDate() - 1);
  return v;
}

/** Nombre de jours consécutifs avec au moins une révision (aujourd'hui compte s'il y en a déjà eu). */
export function serieDeJours(jours: JourActivite[], aujourdhui = new Date()): number {
  const actifs = new Set(jours.filter((j) => j.revisions > 0).map((j) => j.jour));
  let d = actifs.has(jourLocal(aujourdhui)) ? aujourdhui : veille(aujourdhui);
  let serie = 0;
  while (actifs.has(jourLocal(d))) {
    serie++;
    d = veille(d);
  }
  return serie;
}

/** Les `n` derniers jours (du plus ancien au plus récent), y compris ceux sans révision. */
export function derniersJours(jours: JourActivite[], n: number, aujourdhui = new Date()): JourActivite[] {
  const parJour = new Map(jours.map((j) => [j.jour, j]));
  const resultat: JourActivite[] = [];
  let d = aujourdhui;
  for (let i = 0; i < n; i++) {
    const cle = jourLocal(d);
    resultat.unshift(parJour.get(cle) ?? { jour: cle, revisions: 0, justes: 0, dureeMs: 0 });
    d = veille(d);
  }
  return resultat;
}

export function totaux(jours: JourActivite[]) {
  return jours.reduce(
    (t, j) => ({ revisions: t.revisions + j.revisions, justes: t.justes + j.justes, dureeMs: t.dureeMs + j.dureeMs }),
    { revisions: 0, justes: 0, dureeMs: 0 },
  );
}

export function formaterDuree(ms: number): string {
  const minutes = Math.round(ms / 60_000);
  if (minutes < 60) return `${minutes} min`;
  const h = Math.floor(minutes / 60);
  const m = minutes % 60;
  return m ? `${h} h ${m}` : `${h} h`;
}
