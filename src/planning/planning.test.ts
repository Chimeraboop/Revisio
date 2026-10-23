import { describe, expect, it } from "vitest";
import type { Evaluation } from "../lib/api";
import {
  ajouterJours,
  chapitresDuJour,
  cleChapitre,
  compteARebours,
  concerneGroupe,
  ecartJours,
  InfoChapitre,
  libelleJour,
  planifier,
  repartir,
} from "./planning";

const evaluation = (jour: string, chapitres: (string | null)[], id = 1): Evaluation => ({
  id,
  jour,
  matiere: "Spé SVT",
  titre: "Contrôle",
  genre: "controle",
  chapitres,
});

const infos = (m: Record<string, number>) =>
  new Map<string, InfoChapitre>(Object.entries(m).map(([c, maitrise]) => [cleChapitre("Spé SVT", c), { maitrise, total: 10, vocabulaire: 0 }]));

describe("dates", () => {
  it("compte les jours et passe les mois", () => {
    expect(ecartJours("2026-10-19", "2026-10-28")).toBe(9);
    expect(ajouterJours("2026-10-30", 3)).toBe("2026-11-02");
    expect(ajouterJours("2026-03-01", -1)).toBe("2026-02-28");
    expect(compteARebours("2026-10-24", "2026-10-19")).toBe("dans 5 jours");
    expect(compteARebours("2026-10-20", "2026-10-19")).toBe("demain");
    expect(libelleJour("2026-10-19", "2026-10-19")).toBe("Aujourd'hui");
    expect(libelleJour("2026-10-21", "2026-10-19")).toBe("Mercredi 21 octobre");
  });
});

describe("séances du jour concernées", () => {
  it("langues : seulement les chapitres avec du vocabulaire", () => {
    const vocab = { maitrise: 0, total: 30, vocabulaire: 30 };
    const mixte = { maitrise: 0, total: 30, vocabulaire: 10 };
    expect([concerneGroupe(vocab, "langues"), concerneGroupe(vocab, "autres")]).toEqual([true, false]);
    expect([concerneGroupe(mixte, "langues"), concerneGroupe(mixte, "autres")]).toEqual([true, true]);
    expect([concerneGroupe(undefined, "langues"), concerneGroupe(undefined, "autres")]).toEqual([false, true]);
  });
});

describe("répartition des chapitres", () => {
  it("deux passages sur les derniers jours quand le temps le permet", () => {
    expect(repartir(["A", "B"], 6)).toEqual([[], [], ["A"], ["B"], ["A"], ["B"]]);
  });
  it("un deuxième passage pour les plus faibles seulement", () => {
    expect(repartir(["A", "B", "C"], 4)).toEqual([["A"], ["B"], ["C"], ["A"]]);
  });
  it("plusieurs chapitres par jour quand il manque des jours", () => {
    expect(repartir(["A", "B", "C", "D", "E"], 2)).toEqual([["A", "B"], ["C", "D", "E"]]);
    expect(repartir(["A"], 0)).toEqual([]);
  });
});

describe("planning", () => {
  it("chapitres les plus faibles d'abord, révision générale la veille", () => {
    const plan = planifier([evaluation("2026-10-25", ["Méiose", "Mutations"])], infos({ Méiose: 0.9, Mutations: 0.3 }), "2026-10-19");
    // 5 jours disponibles (19 → 23) pour 2 chapitres : deux passages sur les 4 derniers jours.
    expect(plan.map((j) => [j.jour, j.elements.map((e) => `${e.chapitre}:${e.nature}:${e.passage}`)])).toEqual([
      ["2026-10-20", ["Mutations:chapitre:1"]],
      ["2026-10-21", ["Méiose:chapitre:1"]],
      ["2026-10-22", ["Mutations:chapitre:2"]],
      ["2026-10-23", ["Méiose:chapitre:2"]],
      ["2026-10-24", ["Méiose:revision_generale:1", "Mutations:revision_generale:1"]],
    ]);
    expect(chapitresDuJour(plan, "2026-10-20")).toEqual([{ matiere: "Spé SVT", chapitre: "Mutations" }]);
    expect(chapitresDuJour(plan, "2026-10-19")).toEqual([]);
  });

  it("évaluation demain : révision générale aujourd'hui ; aujourd'hui : jour J", () => {
    const plan = planifier(
      [evaluation("2026-10-20", ["Méiose"]), evaluation("2026-10-19", [null], 2), evaluation("2026-10-01", ["Vieux"], 3)],
      infos({}),
      "2026-10-19",
    );
    expect(plan).toHaveLength(1);
    expect(plan[0].elements.map((e) => [e.chapitre, e.nature])).toEqual([
      [null, "jour_j"],
      ["Méiose", "revision_generale"],
    ]);
    expect(chapitresDuJour(plan, "2026-10-19")).toEqual([
      { matiere: "Spé SVT", chapitre: null },
      { matiere: "Spé SVT", chapitre: "Méiose" },
    ]);
  });

  it("deux évaluations le même jour se partagent le planning sans doublon de chapitre", () => {
    const plan = planifier(
      [evaluation("2026-10-21", ["Méiose"]), evaluation("2026-10-21", ["Méiose"], 2)],
      infos({}),
      "2026-10-19",
    );
    expect(chapitresDuJour(plan, "2026-10-19")).toEqual([{ matiere: "Spé SVT", chapitre: "Méiose" }]);
  });
});
