import { describe, expect, it } from "vitest";
import { bouts, couperEnLignes, formuleEnBouts } from "./texte";

const texte = (b: { texte: string; position: string }[]) => b.map((x) => `${x.position[0]}:${x.texte}`).join("|");

describe("formules pour le canevas", () => {
  it("exposants, indices et commandes", () => {
    expect(texte(formuleEnBouts("x^2 + y_{n+1}"))).toBe("n:x|e:2|n: + y|i:n+1");
    expect(texte(formuleEnBouts("\\Delta = b^2 - 4ac"))).toBe("n:Δ = b|e:2|n: - 4ac");
    expect(texte(formuleEnBouts("e^{-\\lambda t}"))).toBe("n:e|e:-λ t");
  });

  it("fractions et racines, même imbriquées", () => {
    expect(texte(formuleEnBouts("\\frac{1}{2}"))).toBe("n:1/2");
    expect(texte(formuleEnBouts("\\frac{x^2}{2}"))).toBe("n:(x|e:2|n:)/2");
    expect(texte(formuleEnBouts("\\sqrt{\\frac{a}{b}}"))).toBe("n:√(a/b)");
    expect(texte(formuleEnBouts("\\sqrt{2}"))).toBe("n:√2");
  });

  it("texte avec formules, Unicode et Markdown", () => {
    expect(texte(bouts("**Méiose** : $2n$ → $n$"))).toBe("n:Méiose : 2n → n");
    expect(texte(bouts("N₀ et ²³⁵U"))).toBe("n:N|i:0|n: et |e:235|n:U");
  });
});

describe("coupure en lignes", () => {
  // Chaque caractère mesure 10 (6 en exposant/indice).
  const mesurer = (b: { texte: string; position: string }) => b.texte.length * (b.position === "normal" ? 10 : 6);

  it("coupe aux espaces sans séparer un mot de son exposant", () => {
    const lignes = couperEnLignes(bouts("aaa bbb $x^2$ ccc"), 75, mesurer);
    expect(lignes.map(texte)).toEqual(["n:aaa|n: |n:bbb", "n:x|e:2|n: |n:ccc"]);
  });

  it("un mot trop long reste seul", () => {
    expect(couperEnLignes(bouts("abcdefghij k"), 50, mesurer).length).toBe(2);
    expect(couperEnLignes([], 50, mesurer)).toEqual([]);
  });
});
