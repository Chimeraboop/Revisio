import { describe, expect, it } from "vitest";
import { noteAutoEvaluee, noteSuggeree, normaliser, variantes, verifier } from "./verification";
import { apercu, planifier, retention } from "./fsrs";

describe("vérification des réponses tapées", () => {
  it("accepte la réponse exacte, sans tenir compte de la casse ni des marques de dictionnaire", () => {
    expect(verifier("Die Verfolgung", "die Verfolgung").verdict).toBe("juste");
    expect(verifier("wegfahren", "weg/fahren*").verdict).toBe("juste");
    expect(verifier("überschreiten", "überschreiten (überschritt, hat überschritten)").verdict).toBe("juste");
    expect(verifier("sich zu widmen", "sich zu +DAT widmen (widmete, gewidmet)").verdict).toBe("juste");
  });

  it("accepte une des variantes et donne la réponse complète", () => {
    const r = verifier("die Flagge", "die Fahne / die Flagge");
    expect(r.verdict).toBe("juste");
    expect(r.message).toContain("die Fahne / die Flagge");
    expect(verifier("emigrieren", "aus/wandern, emigrieren").verdict).toBe("juste");
  });

  it("article manquant = presque, mauvais article = faux", () => {
    expect(verifier("Verfassung", "die Verfassung")).toEqual({
      verdict: "presque",
      message: "Il manque l'article : « die ».",
    });
    expect(verifier("der Verfassung", "die Verfassung").verdict).toBe("faux");
  });

  it("trémas et petites fautes = presque ; mot différent = faux", () => {
    expect(verifier("kampfen", "kämpfen").verdict).toBe("presque");
    const r = verifier("Gleichgultigkeit", "die Gleichgültigkeit");
    expect(r.verdict).toBe("presque");
    expect(r.message).toContain("Il manque l'article");
    expect(verifier("Gleichgültigkiet", "Gleichgültigkeit").verdict).toBe("presque");
    expect(verifier("Einheit", "die Vielfalt").verdict).toBe("faux");
    expect(verifier("", "die Vielfalt").verdict).toBe("faux");
  });

  it("trait d'union et œ", () => {
    expect(verifier("cellule oeuf", "Cellule-œuf (zygote)").verdict).toBe("juste");
    expect(verifier("cellule-œuf", "Cellule-œuf (zygote)").verdict).toBe("juste");
  });

  it("textes à trous : article ajouté accepté, pluriel = presque", () => {
    expect(verifier("la membrane plasmique", "membrane plasmique").verdict).toBe("juste");
    expect(verifier("l'endonucléase", "endonucléases").verdict).toBe("presque");
    expect(verifier("DM1", "DM1 (réductionnelle)").verdict).toBe("juste");
    expect(verifier("cytoplasme", "membrane plasmique").verdict).toBe("faux");
  });

  it("variantes et normalisation", () => {
    expect(variantes("s'en aller, partir")).toEqual(["s'en aller, partir", "s'en aller", "partir"]);
    expect(variantes("weg/fahren*")).toEqual(["weg/fahren*"]);
    expect(normaliser("  Ins  Ausland fahren* ")).toBe("ins ausland fahren");
  });
});

describe("note FSRS", () => {
  it("bonne réponse au hasard → à revoir", () => {
    expect(noteSuggeree("juste", "hasard")).toBe(1);
    expect(noteSuggeree("juste", "hesitant")).toBe(2);
    expect(noteSuggeree("juste", "sur")).toBe(3);
    expect(noteSuggeree("presque", "sur")).toBe(2);
    expect(noteSuggeree("faux", "sur")).toBe(1);
    expect(noteAutoEvaluee(4, "hasard")).toBe(1);
    expect(noteAutoEvaluee(4, "sur")).toBe(4);
  });

  it("accepte l'état « déjà apprise » fabriqué par le cœur Rust", () => {
    // Même forme que revision::marquer_apprises.
    const etat = JSON.stringify({
      due: "2026-11-02T10:00:00.000Z",
      stability: 30,
      difficulty: 5,
      elapsed_days: 0,
      scheduled_days: 15,
      learning_steps: 0,
      reps: 1,
      lapses: 0,
      state: 2,
      last_review: "2026-10-18T10:00:00.000Z",
    });
    expect(retention(etat, new Date("2026-10-18T10:00:00Z"))).toBeGreaterThan(0.99);
    expect(retention(etat, new Date("2026-11-17T10:00:00Z"))).toBeCloseTo(0.9, 1);
    const suite = planifier(etat, 3, new Date("2026-11-02T10:00:00Z"));
    expect(suite.echeance).toBeGreaterThan(new Date("2026-11-02T10:00:00Z").getTime());
  });

  it("planifie une nouvelle carte et retrouve son état", () => {
    const maintenant = new Date("2026-10-18T10:00:00");
    const bien = planifier(null, 3, maintenant);
    expect(bien.echeance).toBeGreaterThan(maintenant.getTime());
    const ensuite = planifier(bien.etat, 3, new Date(bien.echeance));
    expect(ensuite.echeance).toBeGreaterThan(bien.echeance);
    expect(retention(bien.etat, new Date(bien.echeance))).toBeGreaterThan(0.5);
    expect(retention(null)).toBe(0);
    const a = apercu(null, maintenant);
    expect(Object.keys(a)).toEqual(["1", "2", "3", "4"]);
  });
});
