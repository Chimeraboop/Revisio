import { describe, expect, it } from "vitest";
import type { Diapo } from "../lib/api";
import { PAUSE_APRES_MS, PAUSE_AVANT_MS, pointsVisibles, segments } from "./montage";

const diapo = (dureeMs: number): Diapo => ({ titre: "t", points: [], schema: null, narration: "n", source: "", dureeMs });

describe("minutage des diapositives", () => {
  it("chaque diapositive = pause + narration + pause", () => {
    const pauses = PAUSE_AVANT_MS + PAUSE_APRES_MS;
    expect(segments([diapo(10_000), diapo(4_000)])).toEqual([
      { debut: 0, duree: 10_000 + pauses },
      { debut: 10_000 + pauses, duree: 4_000 + pauses },
    ]);
  });

  it("les points apparaissent au fil de la narration", () => {
    expect(pointsVisibles(0, 5000, 10_000)).toBe(0);
    expect(pointsVisibles(4, 0, 10_000)).toBe(1);
    // 4 points sur 85 % de 10 s : un nouveau toutes les 2,125 s.
    expect(pointsVisibles(4, PAUSE_AVANT_MS + 2000, 10_000)).toBe(1);
    expect(pointsVisibles(4, PAUSE_AVANT_MS + 2200, 10_000)).toBe(2);
    expect(pointsVisibles(4, PAUSE_AVANT_MS + 6500, 10_000)).toBe(4);
    expect(pointsVisibles(4, 60_000, 10_000)).toBe(4);
  });
});
