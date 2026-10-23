import { describe, expect, it } from "vitest";
import { texteEnHtml, versMarkmap } from "./conversion";

describe("carte mentale → Markmap", () => {
  it("échappe le HTML et rend les formules", () => {
    expect(texteEnHtml("a < b & c")).toBe("a &lt; b &amp; c");
    const html = texteEnHtml("Cellule $2n$ diploïde");
    expect(html.startsWith("Cellule ")).toBe(true);
    expect(html).toContain('class="katex"');
    expect(html.endsWith(" diploïde")).toBe(true);
  });

  it("garde le lien vers la note et l'explication au survol", () => {
    const m = versMarkmap({
      texte: "Méiose",
      detail: 'Deux "divisions"',
      note: "SVT/Méiose.md",
      enfants: [{ texte: "DM1", detail: "", note: null, enfants: [] }],
    });
    expect(m.content).toContain('title="Deux &quot;divisions&quot;"');
    expect(m.content).toContain('data-chemin="SVT/Méiose.md"');
    expect(m.children).toHaveLength(1);
    expect(m.children?.[0].content).not.toContain("data-chemin");
  });
});
