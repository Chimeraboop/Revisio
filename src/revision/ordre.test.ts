import { describe, expect, it } from "vitest";
import { evaluerOrdre, melangerOrdre } from "./ModuleOrdre";

describe("remise dans l'ordre", () => {
  it("le mélange n'est jamais déjà dans le bon ordre", () => {
    for (let essai = 0; essai < 50; essai++) {
      const m = melangerOrdre(3);
      expect([...m].sort()).toEqual([0, 1, 2]);
      expect(m).not.toEqual([0, 1, 2]);
    }
  });

  it("évaluation", () => {
    expect(evaluerOrdre([0, 1, 2, 3, 4]).verdict).toBe("juste");
    // Une seule inversion de voisins sur 5 : presque.
    expect(evaluerOrdre([0, 1, 3, 2, 4])).toEqual({ verdict: "presque", message: "3 / 5 à la bonne place." });
    expect(evaluerOrdre([4, 3, 2, 1, 0]).verdict).toBe("faux");
  });
});
