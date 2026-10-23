import { describe, expect, it } from "vitest";
import { formaterTemps, repliqueEnCours } from "./temps";

describe("lecteur de podcast", () => {
  it("affiche les durées", () => {
    expect(formaterTemps(0)).toBe("0:00");
    expect(formaterTemps(247.9)).toBe("4:07");
    expect(formaterTemps(3723)).toBe("1:02:03");
    expect(formaterTemps(Number.NaN)).toBe("0:00");
  });

  it("trouve la réplique en cours", () => {
    const r = [{ debutMs: 0 }, { debutMs: 5000 }, { debutMs: 12_000 }];
    expect(repliqueEnCours(r, 0)).toBe(0);
    expect(repliqueEnCours(r, 6000)).toBe(1);
    expect(repliqueEnCours(r, 99_000)).toBe(2);
    expect(repliqueEnCours([{ debutMs: null }], 1000)).toBe(-1);
  });
});
