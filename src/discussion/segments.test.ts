import { describe, expect, it } from "vitest";
import { decouper, lierCitations } from "./segments";

describe("réponse de l'IA découpée", () => {
  it("sépare le texte et les visuels, dans l'ordre", () => {
    const s = decouper("Voici [E1].\n\n```mermaid\n%% Titre : A\nflowchart LR\nA-->B\n```\nPuis :\n```graphique\n{}\n```\nFin.");
    expect(s.map((x) => x.genre)).toEqual(["texte", "diagramme", "texte", "graphique", "texte"]);
    const d = s[1] as { indice: number; complet: boolean; contenu: string };
    expect([d.indice, d.complet, d.contenu.startsWith("%% Titre")]).toEqual([0, true, true]);
    expect((s[3] as { indice: number }).indice).toBe(1);
  });

  it("un bloc en cours d'écriture est incomplet", () => {
    const s = decouper("Début\n```mermaid\nflowchart TD\nA-->");
    expect(s).toHaveLength(2);
    expect(s[1]).toMatchObject({ genre: "diagramme", complet: false });
  });

  it("les autres blocs de code restent du texte", () => {
    expect(decouper("```python\nprint(1)\n```").map((x) => x.genre)).toEqual(["texte"]);
  });
});

describe("citations", () => {
  const extraits = [
    { id: "E1", chemin: "SVT/Méiose/La méiose.md", titre: "La méiose", section: "" },
    { id: "E2", chemin: "SVT/Méiose/Brassage.md", titre: "Brassage", section: "" },
  ];
  it("deviennent des liens vers les notes", () => {
    expect(lierCitations("Vu [E1] et [E2, E1].", extraits)).toBe(
      "Vu [[SVT/Méiose/La méiose|E1]] et [[SVT/Méiose/Brassage|E2]] [[SVT/Méiose/La méiose|E1]].",
    );
  });
  it("un numéro inconnu reste du texte, les autres crochets ne bougent pas", () => {
    expect(lierCitations("[E9] et [voir plus haut]", extraits)).toBe("E9 et [voir plus haut]");
  });
});
