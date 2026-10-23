import { describe, expect, it } from "vitest";
import { evolution } from "./animations";
import { points } from "./CourbeActivite";

const jour = (revisions: number) => ({ jour: "2026-10-20", revisions, justes: 0, dureeMs: 0 });

describe("tableau de bord", () => {
  it("évolution entre deux semaines", () => {
    expect(evolution(10, 15)).toBe(50);
    expect(evolution(20, 10)).toBe(-50);
    expect(evolution(0, 10)).toBeNull();
  });

  it("points de la courbe : le maximum en haut, zéro en bas, de gauche à droite", () => {
    const p = points([jour(0), jour(10), jour(5)]);
    expect(p[0].x).toBeLessThan(p[1].x);
    expect(p[1].y).toBeLessThan(p[2].y);
    expect(p[2].y).toBeLessThan(p[0].y);
    // Moins de 5 réponses au maximum : l'échelle reste à 5 (la courbe ne touche pas le haut).
    expect(points([jour(1)])[0].y).toBeGreaterThan(points([jour(5)])[0].y);
  });
});
