// Chaîne de rendu Markdown → éléments React.
// Analogie Python : une suite de fonctions appliquées l'une après l'autre au même arbre.
import { createContext, useContext, useEffect, useMemo, useState, type ComponentProps } from "react";
import { Fragment, jsx, jsxs } from "react/jsx-runtime";
import { toJsxRuntime } from "hast-util-to-jsx-runtime";
import { unified } from "unified";
import remarkParse from "remark-parse";
import remarkFrontmatter from "remark-frontmatter";
import remarkGfm from "remark-gfm";
import remarkMath from "remark-math";
import remarkBreaks from "remark-breaks";
import remarkRehype from "remark-rehype";
import rehypeRaw from "rehype-raw";
import rehypeSanitize, { defaultSchema } from "rehype-sanitize";
import rehypeKatex from "rehype-katex";
import type { Root } from "hast";
import "katex/dist/katex.min.css";

import { pretraiter, remarkCallouts, remarkMathBloc, remarkObsidian } from "./obsidian";
import Dessin from "../excalidraw/Dessin";
import { extraireScene, type Scene } from "../excalidraw/scene";
import { api } from "../lib/api";

// Le schéma par défaut limite les classes CSS permises sur certaines balises (a, li…) :
// on retire ces restrictions pour que « className » soit permis partout (ce n'est pas dangereux).
const attributsSansRestrictionDeClasse = Object.fromEntries(
  Object.entries(defaultSchema.attributes ?? {}).map(([balise, liste]) => [
    balise,
    liste.filter((a) => !(Array.isArray(a) && a[0] === "className")),
  ]),
);

// HTML autorisé : celui du Markdown + ce dont ont besoin les callouts, liens Obsidian et pages d'accueil.
// Tout le reste (scripts, iframes, styles en ligne…) est supprimé.
const SCHEMA = {
  ...defaultSchema,
  tagNames: [
    ...(defaultSchema.tagNames ?? []),
    "details", "summary", "mark", "svg", "path", "line", "circle", "rect", "polyline", "polygon", "g",
  ],
  attributes: {
    ...attributsSansRestrictionDeClasse,
    "*": [...(defaultSchema.attributes?.["*"] ?? []), "className", "data*"],
    a: [...(attributsSansRestrictionDeClasse.a ?? []), "dataHref"],
    code: ["className"],
    details: ["open"],
    svg: ["viewBox", "fill", "stroke", "strokeWidth", "strokeLinecap", "strokeLinejoin", "width", "height"],
    path: ["d"],
    line: ["x1", "y1", "x2", "y2"],
    circle: ["cx", "cy", "r"],
    rect: ["x", "y", "width", "height", "rx", "ry"],
    polyline: ["points"],
    polygon: ["points"],
  },
};

const processeur = unified()
  .use(remarkParse)
  .use(remarkFrontmatter, ["yaml"])
  .use(remarkGfm)
  .use(remarkMath)
  .use(remarkMathBloc)
  .use(remarkCallouts)
  .use(remarkObsidian)
  .use(remarkBreaks) // comme Obsidian : un retour à la ligne simple est conservé
  .use(remarkRehype, { allowDangerousHtml: true })
  .use(rehypeRaw)
  .use(rehypeSanitize, SCHEMA)
  .use(rehypeKatex, { throwOnError: false, strict: false });

/** Markdown → arbre HTML (hast) nettoyé. Exporté pour les tests. */
export function versHast(markdown: string): Root {
  const source = pretraiter(markdown);
  // Le texte source est transmis aux plugins (remarkMathBloc en a besoin).
  return processeur.runSync(processeur.parse(source), source) as Root;
}

/** Ce dont les liens ont besoin pour naviguer. */
export interface ContexteNavigation {
  /** Chemin de la note affichée (sert à résoudre les liens relatifs et les homonymes). */
  source: string;
  ouvrirNote: (chemin: string, ancre?: string | null) => void;
}

const Contexte = createContext<ContexteNavigation | null>(null);

export function MarkdownObsidian({ markdown, navigation }: { markdown: string; navigation: ContexteNavigation }) {
  const contenu = useMemo(() => {
    try {
      return toJsxRuntime(versHast(markdown), { Fragment, jsx, jsxs, components: COMPOSANTS });
    } catch (e) {
      return <pre className="message-erreur">Rendu impossible : {String(e)}</pre>;
    }
  }, [markdown]);
  return (
    <Contexte.Provider value={navigation}>
      <div className="markdown">{contenu}</div>
    </Contexte.Provider>
  );
}

const COMPOSANTS = { a: Lien, span: Span, img: ImageMarkdown };

function Lien(props: ComponentProps<"a">) {
  const nav = useContext(Contexte);
  const p = props as Record<string, unknown>;
  const wikilien = p["data-wikilink"] as string | undefined;
  const ancre = (p["data-ancre"] as string | undefined) ?? null;
  const dataHref = p["data-href"] as string | undefined;
  const href = props.href ?? "";
  const [casse, setCasse] = useState(false);

  async function clic(e: React.MouseEvent) {
    e.preventDefault(); // ne jamais laisser la fenêtre de l'app quitter Révisio
    if (!nav) return;
    if (wikilien !== undefined) {
      if (!wikilien) return; // lien vers une section de la même note
      const cible = await api.resoudreLien(wikilien, nav.source);
      if (cible?.genre === "note") nav.ouvrirNote(cible.chemin, ancre);
      else setCasse(true);
    } else if (dataHref) {
      const cible = await api.resoudreLien(dataHref, nav.source);
      if (cible?.genre === "note") nav.ouvrirNote(cible.chemin);
      else setCasse(true);
    } else if (/^https?:\/\//.test(href)) {
      await api.ouvrirLienExterne(href);
    }
  }

  const { className, children } = props;
  const externe = /^https?:\/\//.test(href);
  return (
    <a
      href="#"
      className={[className, casse ? "lien-casse" : "", externe ? "lien-externe" : ""].filter(Boolean).join(" ")}
      title={casse ? "Note introuvable" : externe ? href : undefined}
      onClick={clic}
    >
      {children}
    </a>
  );
}

function Span(props: ComponentProps<"span">) {
  const p = props as Record<string, unknown>;
  const cible = p["data-embed"] as string | undefined;
  if (cible === undefined) return <span {...props} />;
  return <Integration cible={cible} taille={(p["data-taille"] as string | undefined) ?? null} />;
}

const EXTENSIONS_IMAGE = /\.(png|jpe?g|gif|webp|svg|bmp)$/i;

/** `![[...]]` : image du vault, schéma Excalidraw ou autre note. */
function Integration({ cible, taille }: { cible: string; taille: string | null }) {
  const nav = useContext(Contexte);
  if (EXTENSIONS_IMAGE.test(cible)) return <ImageVault cible={cible} largeur={taille} />;
  return <NoteIntegree cible={cible} source={nav?.source ?? ""} />;
}

function NoteIntegree({ cible, source }: { cible: string; source: string }) {
  const nav = useContext(Contexte);
  const [scene, setScene] = useState<Scene | null | undefined>(undefined);
  const [chemin, setChemin] = useState<string | null>(null);
  const schema = /\.excalidraw$/i.test(cible);

  useEffect(() => {
    let actif = true;
    api.resoudreLien(cible, source).then(async (c) => {
      if (!actif || c?.genre !== "note") return;
      setChemin(c.chemin);
      if (schema) {
        const n = await api.note(c.chemin);
        if (actif) setScene(extraireScene(n.contenu));
      }
    });
    return () => {
      actif = false;
    };
  }, [cible, source, schema]);

  const titre = cible.replace(/\.excalidraw$/i, "");
  return (
    <span className="integration">
      <span className="integration-titre">
        {schema ? "Schéma : " : "Note intégrée : "}
        {chemin ? (
          <a href="#" onClick={(e) => (e.preventDefault(), nav?.ouvrirNote(chemin))}>
            {titre}
          </a>
        ) : (
          titre
        )}
      </span>
      {schema && scene !== undefined && <Dessin scene={scene} titre={cible} />}
    </span>
  );
}

/** Image du vault : lue en octets par le cœur Rust puis affichée via une URL « blob: » temporaire. */
function ImageVault({ cible, largeur }: { cible: string; largeur?: string | null }) {
  const nav = useContext(Contexte);
  const [url, setUrl] = useState<string | null>(null);
  const [erreur, setErreur] = useState(false);

  useEffect(() => {
    let actif = true;
    let objet: string | null = null;
    (async () => {
      const c = await api.resoudreLien(cible, nav?.source ?? null);
      if (!c || c.genre !== "fichier") throw new Error("introuvable");
      const octets = await api.lireFichierVault(c.chemin);
      objet = URL.createObjectURL(new Blob([octets], { type: typeMime(c.chemin) }));
      if (actif) setUrl(objet);
    })().catch(() => actif && setErreur(true));
    return () => {
      actif = false;
      if (objet) URL.revokeObjectURL(objet);
    };
  }, [cible, nav?.source]);

  if (erreur) return <span className="lien-casse">Image introuvable : {cible}</span>;
  if (!url) return <span className="texte-doux petit">Chargement de l'image…</span>;
  const l = largeur && /^\d+$/.test(largeur) ? Number(largeur) : undefined;
  return <img src={url} alt={cible} style={l ? { width: l, maxWidth: "100%" } : undefined} />;
}

function ImageMarkdown(props: ComponentProps<"img">) {
  const src = props.src ?? "";
  if (/^(https?:|data:|blob:)/.test(src)) return <img {...props} />;
  return <ImageVault cible={decodeURIComponent(src)} />;
}

function typeMime(chemin: string): string {
  const ext = chemin.split(".").pop()?.toLowerCase();
  return (
    { png: "image/png", jpg: "image/jpeg", jpeg: "image/jpeg", gif: "image/gif", webp: "image/webp", svg: "image/svg+xml", bmp: "image/bmp" }[
      ext ?? ""
    ] ?? "application/octet-stream"
  );
}
