import { describe, expect, it } from "vitest";
import { estPdf, etatDuDepot, idDepot, nomFichierDepot, nomFinDepot, tailleReduite } from "./depot";

describe("dépôt de photos", () => {
  it("noms de fichiers : le fichier de fin ne ressemble pas à une photo", () => {
    const id = idDepot("iphone-123", 1731100000000);
    expect(id).toBe("iphone-123_1731100000000");
    expect(nomFichierDepot(id, 2, "jpg")).toBe("depot-iphone-123_1731100000000_2.jpg");
    expect(nomFinDepot(id)).toBe("depot-iphone-123_1731100000000.json");
  });

  it("réduction : jamais d'agrandissement, proportions gardées", () => {
    expect(tailleReduite(1000, 800)).toEqual({ largeur: 1000, hauteur: 800 });
    expect(tailleReduite(4000, 3000)).toEqual({ largeur: 2000, hauteur: 1500 });
    expect(tailleReduite(3000, 4000)).toEqual({ largeur: 1500, hauteur: 2000 });
    expect(tailleReduite(10000, 1)).toEqual({ largeur: 2000, hauteur: 1 });
  });

  it("état d'un dépôt", () => {
    expect(etatDuDepot(undefined)).toMatchObject({ fini: false, probleme: false });
    expect(etatDuDepot({ statut: "en_cours", message: null }).texte).toContain("rédige");
    expect(etatDuDepot({ statut: "termine", message: "2 notes" })).toEqual({ texte: "2 notes", fini: true, probleme: false });
    expect(etatDuDepot({ statut: "erreur", message: null })).toMatchObject({ fini: true, probleme: true });
    expect(etatDuDepot({ statut: "illisible", message: "Flou" })).toMatchObject({ texte: "Flou", probleme: true });
  });

  it("reconnaît un PDF par son type ou son nom", () => {
    expect(estPdf({ name: "cours.PDF", type: "" })).toBe(true);
    expect(estPdf({ name: "x", type: "application/pdf" })).toBe(true);
    expect(estPdf({ name: "photo.jpg", type: "image/jpeg" })).toBe(false);
  });
});
