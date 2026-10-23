// Extensions « Obsidian » pour le rendu Markdown (plugins remark, qui travaillent sur l'arbre mdast).
//
// - callouts  `> [!info]+ Titre`  → <details class="callout"> (repliable) ou <div class="callout">
// - liens     `[[cible#ancre|alias]]` → <a data-wikilink="cible">
// - intégrations `![[image.png|300]]`, `![[schéma.excalidraw]]` → <span data-embed="…">
// - surlignage `==texte==` → <mark>
import type { Root, Blockquote, Paragraph, PhrasingContent, Text, RootContent } from "mdast";
import { visit, SKIP } from "unist-util-visit";

/** Avant l'analyse : retire les commentaires %%…%% et les blocs de plugins Obsidian sans rendu possible. */
export function pretraiter(markdown: string): string {
  return markdown
    .replace(/\r\n/g, "\n")
    .replace(/%%[\s\S]*?%%/g, "")
    .replace(/```(widgets|search-bar|dataview|dataviewjs|tasks|button)\n[\s\S]*?```/g, "");
}

const RE_ENTETE_CALLOUT = /^\[!([\w-]+)\]([+-]?)[ \t]*/;

const TITRES_PAR_DEFAUT: Record<string, string> = {
  info: "Info",
  abstract: "Résumé",
  summary: "Résumé",
  tip: "Astuce",
  example: "Exemple",
  note: "Remarque",
  question: "Question",
  success: "Réussite",
  warning: "Attention",
  danger: "Danger",
  quote: "Citation",
};

export function remarkCallouts() {
  return (arbre: Root) => {
    visit(arbre, "blockquote", (bq: Blockquote) => {
      const premier = bq.children[0];
      if (!premier || premier.type !== "paragraph") return;
      const debut = premier.children[0];
      if (!debut || debut.type !== "text") return;
      const m = RE_ENTETE_CALLOUT.exec(debut.value);
      if (!m) return;

      const type = m[1].toLowerCase();
      const repli = m[2]; // "+" ouvert, "-" fermé, "" non repliable
      debut.value = debut.value.slice(m[0].length);

      // Le titre va jusqu'au premier retour à la ligne du premier paragraphe.
      const titre: PhrasingContent[] = [];
      const suite: PhrasingContent[] = [];
      let dansTitre = true;
      for (const enfant of premier.children) {
        if (!dansTitre) {
          suite.push(enfant);
        } else if (enfant.type === "break") {
          dansTitre = false;
        } else if (enfant.type === "text" && enfant.value.includes("\n")) {
          const i = enfant.value.indexOf("\n");
          if (i > 0) titre.push({ type: "text", value: enfant.value.slice(0, i) });
          const reste = enfant.value.slice(i + 1);
          if (reste) suite.push({ type: "text", value: reste });
          dansTitre = false;
        } else {
          titre.push(enfant);
        }
      }
      const titreVide = titre.every((n) => n.type === "text" && !n.value.trim());
      const titreFinal: PhrasingContent[] = titreVide
        ? [{ type: "text", value: TITRES_PAR_DEFAUT[type] ?? type }]
        : titre;

      const contenu: RootContent[] = [];
      if (suite.length) contenu.push({ type: "paragraph", children: suite });
      contenu.push(...bq.children.slice(1));

      const repliable = repli === "+" || repli === "-";
      bq.data = {
        hName: repliable ? "details" : "div",
        hProperties: {
          className: ["callout", `callout-${type}`],
          dataCallout: type,
          ...(repli === "+" ? { open: true } : {}),
        },
      };
      const enTete: Paragraph = {
        type: "paragraph",
        children: titreFinal,
        data: { hName: repliable ? "summary" : "div", hProperties: { className: ["callout-titre"] } },
      };
      const corps: Blockquote = {
        type: "blockquote",
        children: contenu as Blockquote["children"],
        data: { hName: "div", hProperties: { className: ["callout-contenu"] } },
      };
      bq.children = contenu.length ? [enTete, corps] : [enTete];
    });
  };
}

/**
 * Dans Obsidian, `$$…$$` est toujours une formule centrée, même écrit sur une seule ligne
 * (remark-math le traiterait comme une formule en ligne).
 */
export function remarkMathBloc() {
  return (arbre: Root, fichier: { value?: unknown }) => {
    const source = String(fichier.value ?? "");
    visit(arbre, "inlineMath", (noeud) => {
      const debut = noeud.position?.start.offset;
      if (debut === undefined || !source.startsWith("$$", debut)) return;
      noeud.data = {
        ...noeud.data,
        hProperties: { ...(noeud.data?.hProperties ?? {}), className: ["language-math", "math-display"] },
      };
    });
  };
}

const RE_OBSIDIAN =/(!?)\[\[([^[\]]+?)\]\]|==([^=\n]+?)==/g;

/** Découpe « cible#ancre|alias » en morceaux. */
export function decouperLien(interieur: string) {
  const [cibleAncre, ...aliasParts] = interieur.split("|");
  const alias = aliasParts.length ? aliasParts.join("|").trim() : null;
  const i = cibleAncre.search(/[#^]/);
  const cible = (i >= 0 ? cibleAncre.slice(0, i) : cibleAncre).trim();
  const ancre = i >= 0 ? cibleAncre.slice(i).replace(/^#/, "").trim() : null;
  return { cible, ancre, alias };
}

export function remarkObsidian() {
  return (arbre: Root) => {
    visit(arbre, "text", (noeud: Text, index, parent) => {
      if (!parent || index === undefined) return;
      if (parent.type === "link" || parent.type === "linkReference") return;
      const valeur = noeud.value;
      RE_OBSIDIAN.lastIndex = 0;
      if (!RE_OBSIDIAN.test(valeur)) return;
      RE_OBSIDIAN.lastIndex = 0;

      const morceaux: PhrasingContent[] = [];
      let dernier = 0;
      for (const m of valeur.matchAll(RE_OBSIDIAN)) {
        if (m.index! > dernier) morceaux.push({ type: "text", value: valeur.slice(dernier, m.index) });
        dernier = m.index! + m[0].length;

        if (m[3] !== undefined) {
          morceaux.push({
            type: "emphasis",
            children: [{ type: "text", value: m[3] }],
            data: { hName: "mark" },
          });
          continue;
        }
        const { cible, ancre, alias } = decouperLien(m[2]);
        if (m[1] === "!") {
          morceaux.push({
            type: "emphasis",
            children: [],
            data: {
              hName: "span",
              hProperties: { className: ["embed"], dataEmbed: cible, dataTaille: alias ?? undefined },
            },
          });
        } else {
          const libelle = alias ?? (cible ? (ancre ? `${cible} › ${ancre}` : cible) : ancre ?? "");
          morceaux.push({
            type: "link",
            url: "#",
            children: [{ type: "text", value: libelle }],
            data: {
              hProperties: { className: ["wikilien"], dataWikilink: cible, dataAncre: ancre ?? undefined },
            },
          });
        }
      }
      if (dernier < valeur.length) morceaux.push({ type: "text", value: valeur.slice(dernier) });
      parent.children.splice(index, 1, ...(morceaux as never[]));
      return [SKIP, index + morceaux.length];
    });
  };
}
