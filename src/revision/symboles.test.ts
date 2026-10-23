import { createElement } from "react";
import { renderToStaticMarkup } from "react-dom/server";
import { describe, expect, it } from "vitest";
import { decouper, latexEnTexte, TexteMaths } from "./symboles";
import { aplatir, verifier } from "./verification";

describe("exposants, indices et formules dans les réponses", () => {
  it("découpe les suites d'exposants et d'indices", () => {
    expect(decouper("N₀")).toEqual([
      { type: "texte", texte: "N" },
      { type: "indice", texte: "0" },
    ]);
    expect(decouper("²³⁵U")).toEqual([
      { type: "exposant", texte: "235" },
      { type: "texte", texte: "U" },
    ]);
    expect(decouper("ᴬ⁻⁴Z₋₂Y").map((m) => `${m.type}:${m.texte}`)).toEqual([
      "exposant:A−4",
      "texte:Z",
      "indice:−2",
      "texte:Y",
    ]);
  });

  it("affiche de vrais exposants, indices et formules", () => {
    const html = renderToStaticMarkup(createElement(TexteMaths, { texte: "²³⁵U, N₀ et $x^2$" }));
    expect(html).toContain("<sup>235</sup>U");
    expect(html).toContain("N<sub>0</sub>");
    expect(html).toContain('class="katex"');
  });

  it("isole les formules LaTeX", () => {
    expect(decouper("on a $\\Delta > 0$ donc")).toEqual([
      { type: "texte", texte: "on a " },
      { type: "formule", texte: "\\Delta > 0" },
      { type: "texte", texte: " donc" },
    ]);
    expect(decouper("5 $ et 3 $")).toEqual([
      { type: "texte", texte: "5 " },
      { type: "formule", texte: " et 3 " },
    ]);
  });

  it("rend lisible un morceau de formule coupé", () => {
    expect(latexEnTexte("$$_{Z}^{A}\\text{X} \\rightarrow")).toBe("_Z^AX →");
    expect(latexEnTexte("$\\alpha$ et $\\exists\\, a \\in \\mathbb{R}")).toBe("α et ∃  a ∈ ℝ");
    expect(latexEnTexte("\\frac{1}{2}")).toBe("1/2");
  });

  it("aplatit pour comparer avec ce qu'on tape au clavier", () => {
    expect(aplatir("C₀×V₀ = C×V")).toBe("C0xV0 = CxV");
    expect(aplatir("t½")).toBe("t1/2");
    expect(aplatir("ᴬ⁻⁴Z₋₂Y")).toBe("A-4Z-2Y");
  });

  it("accepte les réponses tapées sans exposants ni indices", () => {
    expect(verifier("N0", "N₀").verdict).toBe("juste");
    expect(verifier("235U", "²³⁵U").verdict).toBe("juste");
    expect(verifier("t1/2", "t½").verdict).toBe("juste");
    expect(verifier("C0xV0 = CxV", "C₀×V₀ = C×V").verdict).toBe("juste");
    expect(verifier("N1", "N₀").verdict).not.toBe("juste");
  });
});
