import { describe, expect, it } from "vitest";
import type { CarteStat } from "../lib/api";
import { planifier } from "./fsrs";
import { derniersJours, formaterDuree, maitriseParMatiere, serieDeJours, statut } from "./stats";

const jour = (j: string, revisions = 3) => ({ jour: j, revisions, justes: 2, dureeMs: 60_000 });

describe("tableau de bord", () => {
  it("série de jours consécutifs", () => {
    const aujourdhui = new Date(2026, 9, 18, 15);
    expect(serieDeJours([jour("2026-10-18"), jour("2026-10-17"), jour("2026-10-15")], aujourdhui)).toBe(2);
    // Pas encore révisé aujourd'hui : la série d'hier compte encore.
    expect(serieDeJours([jour("2026-10-17"), jour("2026-10-16")], aujourdhui)).toBe(2);
    expect(serieDeJours([jour("2026-10-15")], aujourdhui)).toBe(0);
  });

  it("derniers jours complétés par des zéros", () => {
    const d = derniersJours([jour("2026-10-17", 5)], 3, new Date(2026, 9, 18));
    expect(d.map((j) => [j.jour, j.revisions])).toEqual([
      ["2026-10-16", 0],
      ["2026-10-17", 5],
      ["2026-10-18", 0],
    ]);
  });

  it("maîtrise par matière et statut", () => {
    const maintenant = new Date(2026, 9, 18, 10);
    const appris = planifier(null, 4, maintenant).etat; // révisée « facile » à l'instant : très bien retenue
    const cartes: CarteStat[] = [
      { matiere: "Spé SVT", chapitre: "Méiose", genre: "trou", etatFsrs: appris },
      { matiere: "Spé SVT", chapitre: "Méiose", genre: "trou", etatFsrs: null },
      { matiere: "Allemand", chapitre: null, genre: "vocabulaire", etatFsrs: null },
    ];
    const m = maitriseParMatiere(cartes, maintenant);
    expect(m.map((x) => x.matiere)).toEqual(["Allemand", "Spé SVT"]);
    expect(statut(m[0])).toBe("pas-commence");
    const svt = m[1];
    expect(svt.vues).toBe(1);
    expect(svt.total).toBe(2);
    expect(svt.moyenne).toBeGreaterThan(0.45); // une carte à ~100 %, une jamais vue
    expect(svt.chapitres[0].chapitre).toBe("Méiose");
    expect(statut({ moyenne: 0.85, vues: 3, total: 3 })).toBe("solide");
    expect(statut({ moyenne: 0.6, vues: 3, total: 3 })).toBe("fragile");
    expect(statut({ moyenne: 0.2, vues: 3, total: 3 })).toBe("a-revoir");
  });

  it("durées lisibles", () => {
    expect(formaterDuree(25 * 60_000)).toBe("25 min");
    expect(formaterDuree(90 * 60_000)).toBe("1 h 30");
  });
});
