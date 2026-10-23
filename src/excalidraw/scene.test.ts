import { describe, expect, it } from "vitest";
import LZString from "lz-string";
import { extraireScene, masquerLegendes } from "./scene";

const dessin = {
  type: "excalidraw",
  elements: [
    { id: "t", type: "text", x: 0, y: 0, text: "Les étapes de la mitose", originalText: "Les étapes de la mitose" },
    { id: "b", type: "text", x: 300, y: 200, text: "Télophase", originalText: "Télophase" },
    { id: "a", type: "text", x: 10, y: 205, text: "Pro-\nphase", originalText: "Pro-\nphase" },
    { id: "r", type: "rectangle", x: 0, y: 0 },
    { id: "x", type: "text", x: 0, y: 0, text: "effacé", isDeleted: true },
  ],
  files: {},
};

describe("lecture des dessins Excalidraw", () => {
  it("format compressed-json (LZ-String, coupé en lignes)", () => {
    const compresse = LZString.compressToBase64(JSON.stringify(dessin));
    const coupe = compresse.match(/.{1,40}/g)!.join("\n\n");
    const note = `---\nexcalidraw-plugin: parsed\n---\n## Text Elements\nTélophase ^b\n\n%%\n## Drawing\n\`\`\`compressed-json\n${coupe}\n\`\`\`\n%%`;
    const scene = extraireScene(note)!;
    expect(scene.elements.map((e) => e.id)).toEqual(["t", "b", "a", "r"]); // l'élément effacé est retiré
  });

  it("format json non compressé", () => {
    const note = `# Excalidraw Data\n\n## Drawing\n\`\`\`json\n${JSON.stringify(dessin)}\n\`\`\`\n%%`;
    expect(extraireScene(note)?.elements).toHaveLength(4);
  });

  it("dessin illisible → null", () => {
    expect(extraireScene("pas de dessin")).toBeNull();
    expect(extraireScene("```json\n{pas du json\n```")).toBeNull();
  });

  it("schéma muet : légendes numérotées dans l'ordre de lecture", () => {
    const scene = extraireScene(`\`\`\`json\n${JSON.stringify(dessin)}\n\`\`\``)!;
    const { scene: muette, trous } = masquerLegendes(scene, ["Pro- phase", "Télophase"]);
    expect(trous).toEqual([
      { numero: 1, attendu: "Pro- phase" },
      { numero: 2, attendu: "Télophase" },
    ]);
    const textes = Object.fromEntries(muette.elements.map((e) => [e.id, e.text]));
    expect(textes).toMatchObject({ t: "Les étapes de la mitose", a: "(1)", b: "(2)" });
  });
});
