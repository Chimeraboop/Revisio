import { describe, expect, it } from "vitest";
import { diffLignes, replier } from "./diff";

describe("diffLignes", () => {
  it("repère les lignes ajoutées et retirées", () => {
    const d = diffLignes("A\nB\nC", "A\nB2\nC\nD");
    expect(d).toEqual([
      { genre: "egale", texte: "A" },
      { genre: "retiree", texte: "B" },
      { genre: "ajoutee", texte: "B2" },
      { genre: "egale", texte: "C" },
      { genre: "ajoutee", texte: "D" },
    ]);
  });

  it("ignore la différence entre fins de ligne Windows et Unix", () => {
    expect(diffLignes("A\r\nB", "A\nB").every((l) => l.genre === "egale")).toBe(true);
  });
});

describe("replier", () => {
  it("garde le contexte autour des changements et replie le reste", () => {
    const avant = Array.from({ length: 10 }, (_, i) => `L${i}`).join("\n");
    const apres = avant + "\nNouvelle";
    const m = replier(diffLignes(avant, apres), 2);
    expect(m[0]).toEqual({ genre: "replie", nombre: 8 });
    expect(m.slice(1).map((x) => ("texte" in x ? x.texte : ""))).toEqual(["L8", "L9", "Nouvelle"]);
  });
});
