// Actions de la barre d'outils de l'éditeur (gras, titres, listes, encadrés…) : de simples transformations
// du texte Markdown, testables sans écran (EditorState de CodeMirror).
import { EditorSelection, type EditorState, type TransactionSpec } from "@codemirror/state";

/** Entoure la sélection (gras **…**, italique *…*, surligné ==…==, formule $…$) ; retire les marques si
 * la sélection est déjà entourée. Sans sélection, insère les marques et place le curseur au milieu. */
export function entourer(state: EditorState, avant: string, apres = avant): TransactionSpec {
  return state.changeByRange((plage) => {
    const texte = state.sliceDoc(plage.from, plage.to);
    const gauche = state.sliceDoc(plage.from - avant.length, plage.from);
    const droite = state.sliceDoc(plage.to, plage.to + apres.length);
    if (gauche === avant && droite === apres) {
      return {
        changes: [
          { from: plage.from - avant.length, to: plage.from, insert: "" },
          { from: plage.to, to: plage.to + apres.length, insert: "" },
        ],
        range: EditorSelection.range(plage.from - avant.length, plage.to - avant.length),
      };
    }
    return {
      changes: { from: plage.from, to: plage.to, insert: avant + texte + apres },
      range: EditorSelection.range(plage.from + avant.length, plage.to + avant.length),
    };
  });
}

/** Lignes touchées par la sélection (numéros de ligne, du premier au dernier). */
function lignesSelectionnees(state: EditorState): number[] {
  const numeros = new Set<number>();
  for (const plage of state.selection.ranges) {
    const debut = state.doc.lineAt(plage.from).number;
    const fin = state.doc.lineAt(plage.to).number;
    for (let n = debut; n <= fin; n++) numeros.add(n);
  }
  return [...numeros].sort((a, b) => a - b);
}

/** Met (ou retire, si toutes les lignes l'ont déjà) un préfixe au début des lignes : « - », « > »… */
export function prefixer(state: EditorState, prefixe: string): TransactionSpec {
  const lignes = lignesSelectionnees(state).map((n) => state.doc.line(n));
  const tousPrefixes = lignes.every((l) => l.text.startsWith(prefixe));
  return {
    changes: lignes.map((l) =>
      tousPrefixes ? { from: l.from, to: l.from + prefixe.length, insert: "" } : { from: l.from, insert: prefixe },
    ),
  };
}

/** Liste numérotée 1. 2. 3. sur les lignes choisies (ou retire la numérotation). */
export function numeroter(state: EditorState): TransactionSpec {
  const lignes = lignesSelectionnees(state).map((n) => state.doc.line(n));
  const motif = /^\d+\.\s/;
  const tous = lignes.every((l) => motif.test(l.text));
  return {
    changes: lignes.map((l, i) => {
      const m = l.text.match(motif);
      return tous && m ? { from: l.from, to: l.from + m[0].length, insert: "" } : { from: l.from, insert: `${i + 1}. ` };
    }),
  };
}

/** Titre de niveau 1 à 3 : remplace le niveau existant, ou le retire si c'est déjà le même. */
export function titre(state: EditorState, niveau: number): TransactionSpec {
  const marque = "#".repeat(niveau) + " ";
  return {
    changes: lignesSelectionnees(state).map((n) => {
      const l = state.doc.line(n);
      const existant = l.text.match(/^#{1,6}\s/);
      if (existant && existant[0] === marque) return { from: l.from, to: l.from + marque.length, insert: "" };
      return { from: l.from, to: l.from + (existant?.[0].length ?? 0), insert: marque };
    }),
  };
}

/** Encadrés Obsidian proposés (le type et le titre affiché). Les mots en gras d'une « Définition »
 * deviennent des textes à trous. */
export const ENCADRES: [type: string, titre: string][] = [
  ["info", "Définition"],
  ["example", "Exemple"],
  ["abstract", "Propriété"],
  ["note", "Contexte"],
  ["success", "Application"],
  ["warning", "Points de vigilance"],
  ["quote", "Citation"],
  ["tip", "Notes liées"],
];

/** Encadré (callout) : entoure les lignes sélectionnées, ou en insère un vide sur une nouvelle ligne. */
export function encadre(state: EditorState, type: string, nom: string): TransactionSpec {
  const entete = `> [!${type}]+ ${nom}`;
  const plage = state.selection.main;
  if (!plage.empty) {
    const debut = state.doc.lineAt(plage.from);
    const fin = state.doc.lineAt(plage.to);
    const corps = state
      .sliceDoc(debut.from, fin.to)
      .split("\n")
      .map((l) => `> ${l}`)
      .join("\n");
    const insert = `${entete}\n${corps}`;
    return { changes: { from: debut.from, to: fin.to, insert }, selection: { anchor: debut.from + insert.length } };
  }
  const ligne = state.doc.lineAt(plage.head);
  const avant = ligne.text.trim() ? "\n\n" : "";
  const insert = `${avant}${entete}\n> `;
  const position = ligne.text.trim() ? ligne.to : ligne.from;
  return {
    changes: { from: position, to: ligne.text.trim() ? position : ligne.to, insert },
    selection: { anchor: position + insert.length },
  };
}

/** Insère un texte à la place de la sélection ; `curseur` = position du curseur dans ce texte. */
export function inserer(state: EditorState, texte: string, curseur = texte.length): TransactionSpec {
  const plage = state.selection.main;
  return { changes: { from: plage.from, to: plage.to, insert: texte }, selection: { anchor: plage.from + curseur } };
}

export const TABLEAU = "| Colonne 1 | Colonne 2 |\n| --- | --- |\n|  |  |\n";
