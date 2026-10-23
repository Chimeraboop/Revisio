import { describe, expect, it } from "vitest";
import { cleChapitreSeance, parMatiere } from "./ChoixChapitres";

const c = (matiere: string | null, chapitre: string | null) => ({ matiere, chapitre, dues: 1, nouvelles: 0, total: 2 });

describe("choix des chapitres d'une séance ciblée", () => {
  it("regroupe par matière dans l'ordre reçu, « Hors matière » pour les notes sans matière", () => {
    const groupes = parMatiere([c("Spé SVT", "Méiose"), c("Spé SVT", null), c("Allemand", "Wortschatz"), c(null, null)]);
    expect(groupes.map(([m, l]) => [m, l.length])).toEqual([
      ["Spé SVT", 2],
      ["Allemand", 1],
      ["Hors matière", 1],
    ]);
  });

  it("distingue « notes générales » et un chapitre nommé, et les matières absentes", () => {
    const cles = [c("A", null), c("A", "null"), c(null, "A"), c("A", "A")].map(cleChapitreSeance);
    expect(new Set(cles).size).toBe(4);
  });
});
