import { describe, expect, it } from "vitest";
import { EditorSelection, EditorState, type TransactionSpec } from "@codemirror/state";
import { encadre, entourer, inserer, numeroter, prefixer, titre } from "./actions";

/** Applique une action à un texte ; « [ » et « ] » marquent la sélection (ou « | » le curseur). */
function appliquer(texte: string, action: (s: EditorState) => TransactionSpec): string {
  const curseur = texte.indexOf("|");
  const debut = curseur >= 0 ? curseur : texte.indexOf("[");
  const propre = texte.replace("|", "").replace("[", "").replace("]", "");
  const fin = curseur >= 0 ? curseur : texte.indexOf("]") - 1;
  const state = EditorState.create({ doc: propre, selection: EditorSelection.single(debut, fin) });
  const t = state.update(action(state));
  const sel = t.state.selection.main;
  const doc = t.state.doc.toString();
  return sel.empty ? doc.slice(0, sel.head) + "|" + doc.slice(sel.head) : doc.slice(0, sel.from) + "[" + doc.slice(sel.from, sel.to) + "]" + doc.slice(sel.to);
}

describe("actions de l'éditeur", () => {
  it("met en gras puis retire le gras", () => {
    expect(appliquer("La [méiose] produit", (s) => entourer(s, "**"))).toBe("La **[méiose]** produit");
    expect(appliquer("La **[méiose]** produit", (s) => entourer(s, "**"))).toBe("La [méiose] produit");
    expect(appliquer("Texte |", (s) => entourer(s, "$"))).toBe("Texte $|$");
  });

  it("préfixe les lignes (listes, citations) et retire le préfixe", () => {
    expect(appliquer("[un\ndeux]", (s) => prefixer(s, "- "))).toBe("- [un\n- deux]");
    expect(appliquer("[- un\n- deux]", (s) => prefixer(s, "- "))).toBe("[un\ndeux]");
  });

  it("numérote les lignes", () => {
    expect(appliquer("[Prophase\nMétaphase]", numeroter)).toBe("1. [Prophase\n2. Métaphase]");
    expect(appliquer("[1. Prophase\n2. Métaphase]", numeroter)).toBe("[Prophase\nMétaphase]");
  });

  it("change le niveau de titre", () => {
    expect(appliquer("La |méiose", (s) => titre(s, 2))).toBe("## La |méiose");
    expect(appliquer("# La |méiose", (s) => titre(s, 2))).toBe("## La |méiose");
    expect(appliquer("## La |méiose", (s) => titre(s, 2))).toBe("La |méiose");
  });

  it("insère un encadré Définition, ou entoure la sélection", () => {
    expect(appliquer("|", (s) => encadre(s, "info", "Définition"))).toBe("> [!info]+ Définition\n> |");
    expect(appliquer("Intro|", (s) => encadre(s, "info", "Définition"))).toBe("Intro\n\n> [!info]+ Définition\n> |");
    expect(appliquer("[La **méiose** produit\ndes gamètes]", (s) => encadre(s, "info", "Définition"))).toBe(
      "> [!info]+ Définition\n> La **méiose** produit\n> des gamètes|",
    );
  });

  it("insère un texte avec le curseur au bon endroit", () => {
    expect(appliquer("Voir |", (s) => inserer(s, "[[]]", 2))).toBe("Voir [[|]]");
  });
});
