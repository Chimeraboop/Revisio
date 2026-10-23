// Petits calculs du lecteur de podcast (testés dans temps.test.ts).
import type { Replique } from "../lib/api";

/** « 4:07 » ou « 1:02:03 ». */
export function formaterTemps(secondes: number): string {
  const s = Math.max(0, Math.floor(Number.isFinite(secondes) ? secondes : 0));
  const h = Math.floor(s / 3600);
  const m = Math.floor((s % 3600) / 60);
  const sec = String(s % 60).padStart(2, "0");
  return h ? `${h}:${String(m).padStart(2, "0")}:${sec}` : `${m}:${sec}`;
}

/** Indice de la réplique prononcée à l'instant `ms` (−1 avant la première). */
export function repliqueEnCours(repliques: Pick<Replique, "debutMs">[], ms: number): number {
  let indice = -1;
  repliques.forEach((r, i) => {
    if (r.debutMs !== null && r.debutMs <= ms) indice = i;
  });
  return indice;
}
