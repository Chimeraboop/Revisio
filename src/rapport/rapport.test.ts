import { describe, expect, it } from "vitest";
import { nomFichier, rapportMarkdown } from "./rapport";

const maintenant = new Date(2026, 9, 19, 18, 0);

describe("rapport de progression", () => {
  it("résume la progression en Markdown", () => {
    const md = rapportMarkdown({
      maintenant,
      cartes: [
        { matiere: "Spé SVT", chapitre: "Méiose", genre: "trou", etatFsrs: null },
        { matiere: "Spé SVT", chapitre: "Méiose", genre: "trou", etatFsrs: null },
      ],
      jours: [{ jour: "2026-10-19", revisions: 10, justes: 8, dureeMs: 600_000 }],
      faibles: [],
      evaluations: [{ id: 1, jour: "2026-10-24", matiere: "Spé SVT", titre: "DS | génétique", genre: "ds", chapitres: ["Méiose", null] }],
      notes: [
        { date: maintenant.getTime(), sujetId: 1, titre: "Contrôle", typeNom: "Mode examen", matiere: "Spé SVT", note: 14 },
        { date: maintenant.getTime(), sujetId: 2, titre: "Trinôme", typeNom: "Exercice", matiere: "Spé Maths", note: 17 },
      ],
      erreurs: [],
    });
    expect(md).toContain("# Ma progression — Révisio");
    expect(md).toContain("**Réussite sur 7 jours :** 80 % (10 réponses)");
    expect(md).toContain("**Série :** 1 jour de révision");
    expect(md).toContain("dans 5 jours");
    expect(md).toContain("Méiose, notes générales");
    expect(md).toContain("| Spé SVT | ○ Pas commencé | 0 / 2 |");
    expect(md).toContain("Moyenne des 2 dernières copies : **15,5 / 20**");
    expect(md).toContain("Aucune erreur à retravailler.");
    expect(nomFichier(maintenant)).toBe("progression-2026-10-19.md");
  });

  it("échappe les barres verticales dans les tableaux", () => {
    const md = rapportMarkdown({
      maintenant,
      cartes: [],
      jours: [],
      faibles: [],
      evaluations: [],
      notes: [{ date: maintenant.getTime(), sujetId: 1, titre: "A | B", typeNom: "Exercice", matiere: "Maths", note: 10 }],
      erreurs: [],
    });
    expect(md).toContain("| A \\| B |");
  });
});
