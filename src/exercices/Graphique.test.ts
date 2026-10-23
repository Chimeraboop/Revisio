import { describe, expect, it } from "vitest";
import { graduations } from "./Graphique";

describe("graduations du graphique", () => {
  it("donne des pas ronds qui couvrent les données", () => {
    expect(graduations(0, 37)).toEqual([0, 5, 10, 15, 20, 25, 30, 35, 40]);
    expect(graduations(0, 1.3)).toEqual([0, 0.2, 0.4, 0.6, 0.8, 1, 1.2, 1.4]);
    expect(graduations(-3, 4)).toEqual([-3, -2, -1, 0, 1, 2, 3, 4]);
  });

  it("gère une série constante", () => {
    const g = graduations(5, 5);
    expect(g[0]).toBeLessThan(5);
    expect(g[g.length - 1]).toBeGreaterThan(5);
  });
});
