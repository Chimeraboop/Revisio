import { describe, expect, it } from "vitest";
import { construireArbre } from "./ArbreNotes";
import type { NoteResume } from "../lib/api";

const note = (chemin: string, genre: NoteResume["genre"] = "cours"): NoteResume => ({
  chemin,
  titre: chemin.split("/").pop()!.replace(/\.md$/, ""),
  genre,
});

describe("construireArbre", () => {
  it("range les notes dans leurs dossiers, à toute profondeur, et garde les dossiers vides", () => {
    const racine = construireArbre(
      [
        note("Accueil.md", "accueil"),
        note("SVT/Chapitre 2/Mitose.md"),
        note("SVT/Chapitre 2/Fiche de synthèse - Chapitre 2.md", "fiche_synthese"),
        note("SVT/Chapitre 2/Annexes/Vieux schéma.md"),
      ],
      ["Histoire", "SVT", "SVT/Chapitre 10", "SVT/Chapitre 2", "SVT/Chapitre 2/Annexes"],
    );
    expect(racine.notes.map((n) => n.titre)).toEqual(["Accueil"]);
    expect(racine.dossiers.map((d) => d.nom)).toEqual(["Histoire", "SVT"]);
    const svt = racine.dossiers[1];
    // Tri « naturel » : Chapitre 2 avant Chapitre 10 ; la fiche de synthèse en tête du chapitre.
    expect(svt.dossiers.map((d) => d.nom)).toEqual(["Chapitre 2", "Chapitre 10"]);
    expect(svt.dossiers[0].notes.map((n) => n.titre)).toEqual(["Fiche de synthèse - Chapitre 2", "Mitose"]);
    expect(svt.dossiers[0].dossiers[0].notes[0].chemin).toBe("SVT/Chapitre 2/Annexes/Vieux schéma.md");
    expect(svt.dossiers[1].notes).toEqual([]);
    expect(racine.dossiers[0].chemin).toBe("Histoire");
  });
});
