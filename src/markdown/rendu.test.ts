import { describe, expect, it } from "vitest";
import { toHtml } from "hast-util-to-html";
import { versHast } from "./rendu";
import { decouperLien, pretraiter } from "./obsidian";

const html = (md: string) => toHtml(versHast(md));

describe("texte à trous corrigé", () => {
  it("surligne une réponse qui contient =, du gras ou une formule", () => {
    const h = html('$A$ : <mark>même $Z$ = **N** + 1</mark> nucléons');
    const marque = h.slice(h.indexOf("<mark>"), h.indexOf("</mark>"));
    expect(marque).toContain("katex");
    expect(marque).toContain("<strong>N</strong>");
    expect(marque).toContain(" = ");
    expect(h).not.toContain("==");
  });
});

describe("callouts Obsidian", () => {
  it("transforme un callout repliable ouvert avec titre et contenu", () => {
    const h = html("> [!info]+ Définition 9 — Discriminant\n> Le réel $\\Delta$ est le **discriminant**.");
    expect(h).toContain('<details class="callout callout-info" data-callout="info" open>');
    expect(h).toContain('<summary class="callout-titre">Définition 9 — Discriminant</summary>');
    expect(h).toContain('<div class="callout-contenu">');
    expect(h).toContain("<strong>discriminant</strong>");
    expect(h).toContain("katex");
  });

  it("garde le LaTeX du titre dans le titre", () => {
    const h = html("> [!example]+ Exemple 1 — $\\Delta>0$\n> Résoudre $3x^2+5x+1=0$");
    const titre = h.slice(h.indexOf("<summary"), h.indexOf("</summary>"));
    expect(titre).toContain("Exemple 1 —");
    expect(titre).toContain("katex");
    expect(titre).not.toContain("Résoudre");
  });

  it("callout sans titre ni repli → div avec titre par défaut", () => {
    const h = html("> [!tip]\n> Contenu");
    expect(h).toContain('<div class="callout callout-tip" data-callout="tip">');
    expect(h).toContain('<div class="callout-titre">Astuce</div>');
  });

  it("$$…$$ sur une ligne de callout est une formule centrée, $…$ reste en ligne", () => {
    const h = html("> [!info]+ Déf\n> Le réel :\n> $$\\Delta = b^2-4ac$$\n> avec $a \\neq 0$");
    expect(h.match(/katex-display/g)?.length).toBe(1);
    expect(h.match(/class="katex"/g)?.length).toBe(2);
  });

  it("une citation normale reste une citation", () => {
    expect(html("> Simple citation")).toContain("<blockquote>");
  });
});

describe("liens et intégrations", () => {
  it("lien [[cible|alias]]", () => {
    const h = html("Voir [[Forme canonique d'un polynôme du second degré|forme canonique]].");
    expect(h).toContain('class="wikilien"');
    expect(h).toContain('data-wikilink="Forme canonique d&#x27;un polynôme du second degré"');
    expect(h).toContain(">forme canonique</a>");
  });

  it("lien dans un titre", () => {
    expect(html("## 1. [[La méiose dans le cycle reproductif]]")).toContain(
      'data-wikilink="La méiose dans le cycle reproductif"',
    );
  });

  it("schéma intégré dans un callout", () => {
    const h = html("> [!example]+ Schéma — Effet d'une mutation\n> ![[La mutation (schéma).excalidraw|800]]");
    expect(h).toContain('data-embed="La mutation (schéma).excalidraw"');
    expect(h).toContain('data-taille="800"');
  });

  it("surlignage ==texte==", () => {
    expect(html("Un ==mot important== ici")).toContain("<mark>mot important</mark>");
  });

  it("découpe cible, ancre et alias", () => {
    expect(decouperLien("Note#Partie 2|vois ça")).toEqual({ cible: "Note", ancre: "Partie 2", alias: "vois ça" });
    expect(decouperLien("#Titre")).toEqual({ cible: "", ancre: "Titre", alias: null });
  });
});

describe("sécurité et nettoyage", () => {
  it("supprime scripts et gestionnaires d'événements", () => {
    const h = html('<script>alert(1)</script><a href="x" onclick="alert(1)">a</a><img src=x onerror="alert(1)">');
    expect(h).not.toContain("<script");
    expect(h).not.toContain("onclick");
    expect(h).not.toContain("onerror");
  });

  it("garde les liens HTML des pages d'accueil", () => {
    const h = html('<a href="Spé SVT/Accueil.md" data-href="Spé SVT/Accueil.md" class="internal-link">SVT</a>');
    expect(h).toContain('data-href="Spé SVT/Accueil.md"');
  });

  it("retire les commentaires %% et les blocs de plugins", () => {
    expect(pretraiter("a %%caché%% b\n```widgets\ntype: clock\n```\nc")).toBe("a  b\n\nc");
  });

  it("ignore le frontmatter", () => {
    expect(html("---\ntags: [x]\n---\n# Titre")).not.toContain("tags");
  });
});
