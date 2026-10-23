import { describe, expect, it } from "vitest";
import { abregerChapitre, couleurMatiere, dansLeMois, decalerMois, grilleMois, moisDe, nomMois } from "./grilleCalendrier";

describe("calendrier du planning", () => {
  it("grille d'octobre 2026 : commence le lundi 28 septembre, 6 semaines", () => {
    const g = grilleMois({ annee: 2026, mois: 9 });
    expect(g).toHaveLength(42);
    expect(g[0]).toBe("2026-09-28");
    expect(g[3]).toBe("2026-10-01"); // jeudi 1er octobre
    expect(g[41]).toBe("2026-11-08");
    expect(dansLeMois("2026-10-31", { annee: 2026, mois: 9 })).toBe(true);
    expect(dansLeMois(g[0], { annee: 2026, mois: 9 })).toBe(false);
  });

  it("un mois qui commence un lundi", () => {
    // 1er juin 2026 = lundi
    expect(grilleMois({ annee: 2026, mois: 5 })[0]).toBe("2026-06-01");
  });

  it("navigation entre les mois et noms", () => {
    expect(moisDe("2026-10-20")).toEqual({ annee: 2026, mois: 9 });
    expect(decalerMois({ annee: 2026, mois: 11 }, 1)).toEqual({ annee: 2027, mois: 0 });
    expect(decalerMois({ annee: 2026, mois: 0 }, -1)).toEqual({ annee: 2025, mois: 11 });
    expect(nomMois({ annee: 2026, mois: 9 })).toBe("Octobre 2026");
  });

  it("noms de chapitres abrégés", () => {
    expect(abregerChapitre("Chapitre 4 - Les mutations")).toBe("Ch. 4 · Les mutations");
    expect(abregerChapitre("Chapitre 12 : Optique")).toBe("Ch. 12 · Optique");
    expect(abregerChapitre("Le cycle cellulaire")).toBe("Le cycle cellulaire");
  });

  it("couleur stable par matière", () => {
    expect(couleurMatiere("Spé SVT")).toBe(couleurMatiere("Spé SVT"));
    expect(couleurMatiere("Spé SVT")).toMatch(/^#[0-9a-f]{6}$/);
  });
});
