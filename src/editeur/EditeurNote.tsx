// Éditeur de notes : Markdown coloré (CodeMirror), barre d'outils, aperçu en direct, complétion des
// [[liens]], images collées ou déposées, enregistrement automatique (une séance = une entrée du journal).
import { useEffect, useMemo, useRef, useState } from "react";
import { EditorState, StateField } from "@codemirror/state";
import { Decoration, type DecorationSet, EditorView, keymap, placeholder, type ViewUpdate } from "@codemirror/view";
import { defaultKeymap, history, historyKeymap, indentWithTab } from "@codemirror/commands";
import { markdown, markdownLanguage } from "@codemirror/lang-markdown";
import { HighlightStyle, syntaxHighlighting } from "@codemirror/language";
import { autocompletion, type CompletionContext, type CompletionResult } from "@codemirror/autocomplete";
import { search, searchKeymap } from "@codemirror/search";
import { tags } from "@lezer/highlight";
import {
  Bold,
  Check,
  Columns2,
  Eye,
  Heading1,
  Heading2,
  Highlighter,
  ImagePlus,
  Italic,
  Link2,
  List,
  ListOrdered,
  LoaderCircle,
  MessageSquareQuote,
  PencilLine,
  Sigma,
  Table,
  TriangleAlert,
} from "lucide-react";
import { open } from "@tauri-apps/plugin-dialog";
import { api, messageErreur } from "../lib/api";
import { MarkdownObsidian, type ContexteNavigation } from "../markdown/rendu";
import { TOUCHE_COMMANDE } from "../lib/plateforme";
import { ENCADRES, TABLEAU, encadre, entourer, inserer, numeroter, prefixer, titre } from "./actions";

interface Props {
  chemin: string;
  titre: string;
  /** Matière › chapitre, affiché au-dessus du titre. */
  fil: string;
  /** Contenu de la note au moment d'ouvrir l'éditeur. */
  contenu: string;
  navigation: ContexteNavigation;
  /** Fin de l'édition ; `historique` = entrée du journal de cette séance (pour « Annuler »), s'il y a eu un enregistrement. */
  onFermer: (historique: number | null) => void;
}

type Vue = "ecrire" | "les-deux" | "apercu";
type Statut = { genre: "enregistre" | "en-cours" | "modifie" | "erreur"; texte?: string };

const DELAI_ENREGISTREMENT = 1200;

/** Couleurs du Markdown, tirées des variables du thème de l'app (clair et sombre). */
const coloration = HighlightStyle.define([
  { tag: tags.heading1, fontSize: "1.45em", fontWeight: "700" },
  { tag: tags.heading2, fontSize: "1.25em", fontWeight: "700" },
  { tag: tags.heading3, fontSize: "1.1em", fontWeight: "650" },
  { tag: [tags.heading4, tags.heading5, tags.heading6], fontWeight: "650" },
  { tag: tags.strong, fontWeight: "700" },
  { tag: tags.emphasis, fontStyle: "italic" },
  { tag: tags.strikethrough, textDecoration: "line-through" },
  { tag: [tags.link, tags.url], color: "var(--accent)" },
  { tag: tags.quote, color: "var(--texte-doux)" },
  { tag: tags.monospace, fontFamily: "Consolas, 'Cascadia Mono', monospace", fontSize: "0.92em" },
  { tag: [tags.processingInstruction, tags.meta, tags.contentSeparator], color: "var(--texte-doux)" },
  { tag: tags.list, color: "var(--texte-doux)" },
]);

const theme = EditorView.theme({
  "&": { height: "100%", fontSize: "15px", backgroundColor: "var(--fond-panneau)", color: "var(--texte)" },
  "&.cm-focused": { outline: "none" },
  ".cm-scroller": { fontFamily: "inherit", lineHeight: "1.65" },
  ".cm-content": { padding: "18px 22px 40vh", maxWidth: "820px", caretColor: "var(--texte)" },
  ".cm-cursor": { borderLeftColor: "var(--texte)" },
  "&.cm-focused .cm-selectionBackground, .cm-selectionBackground, ::selection": {
    backgroundColor: "color-mix(in srgb, var(--accent) 22%, transparent) !important",
  },
  ".cm-placeholder": { color: "var(--texte-doux)" },
  ".cm-tooltip": { border: "1px solid var(--bordure)", backgroundColor: "var(--fond-panneau)", borderRadius: "8px" },
  ".cm-tooltip-autocomplete > ul > li[aria-selected]": { backgroundColor: "var(--fond-survol)", color: "var(--texte)" },
  ".cm-panels": { backgroundColor: "var(--fond-panneau)", color: "var(--texte)" },
});

/** L'en-tête YAML de la note (entre deux « --- » au début) s'affiche en petit et en gris : sinon le
 * Markdown le prendrait pour un grand titre souligné. */
const decorationEnTete = Decoration.line({ class: "cm-entete-yaml" });
const enTeteYaml = StateField.define<DecorationSet>({
  create: (state) => decorerEnTete(state),
  update: (deco, tr) => (tr.docChanged ? decorerEnTete(tr.state) : deco),
  provide: (champ) => EditorView.decorations.from(champ),
});

function decorerEnTete(state: EditorState): DecorationSet {
  const doc = state.doc;
  if (doc.lines < 2 || doc.line(1).text.trim() !== "---") return Decoration.none;
  for (let n = 2; n <= Math.min(doc.lines, 200); n++) {
    if (doc.line(n).text.trim() === "---") {
      const lignes = [];
      for (let i = 1; i <= n; i++) lignes.push(decorationEnTete.range(doc.line(i).from));
      return Decoration.set(lignes);
    }
  }
  return Decoration.none;
}

/** Complétion des liens : après « [[ », propose les titres des notes. */
function completionLiens(titres: () => string[]) {
  return (ctx: CompletionContext): CompletionResult | null => {
    const avant = ctx.matchBefore(/\[\[[^\]\n|#]*/);
    if (!avant) return null;
    const suite = ctx.state.sliceDoc(ctx.pos, ctx.pos + 2);
    return {
      from: avant.from + 2,
      options: titres().map((t) => ({ label: t, apply: suite === "]]" ? t : `${t}]]`, type: "text" })),
      validFor: /^[^\]\n|#]*$/,
    };
  };
}

/** Octets d'une image collée/déposée → pièce jointe, renvoie le texte à insérer. */
async function enregistrerImage(chemin: string, fichier: Blob): Promise<string> {
  const nom = await api.ajouterImage(chemin, new Uint8Array(await fichier.arrayBuffer()));
  return `![[${nom}]]`;
}

export default function EditeurNote({ chemin, titre: titreNote, fil, contenu, navigation, onFermer }: Props) {
  const conteneur = useRef<HTMLDivElement>(null);
  const vue = useRef<EditorView | null>(null);
  /** Dernier contenu enregistré sur le disque (sert à vérifier que la note n'a pas changé ailleurs). */
  const base = useRef(contenu);
  const historique = useRef<number | null>(null);
  const minuterie = useRef<number | undefined>(undefined);
  const enCours = useRef<Promise<void> | null>(null);
  const titres = useRef<string[]>([]);
  const [texte, setTexte] = useState(contenu);
  const [apercu, setApercu] = useState(contenu);
  const [statut, setStatut] = useState<Statut>({ genre: "enregistre" });
  const [mode, setMode] = useState<Vue>(() => (localStorage.getItem("revisio-vue-editeur") as Vue) || "les-deux");
  const [menuEncadres, setMenuEncadres] = useState(false);

  useEffect(() => {
    try {
      localStorage.setItem("revisio-vue-editeur", mode);
    } catch {
      /* stockage indisponible : sans importance */
    }
  }, [mode]);

  // Titres des notes pour la complétion des liens.
  useEffect(() => {
    api
      .arbre()
      .then((a) => {
        const notes = [...a.matieres.flatMap((m) => [...m.notes, ...m.chapitres.flatMap((c) => c.notes)]), ...a.autres.flatMap((g) => g.notes)];
        titres.current = [...new Set(notes.map((n) => n.chemin.split("/").pop()!.replace(/\.md$/i, "")))].sort((x, y) =>
          x.localeCompare(y, "fr"),
        );
      })
      .catch(() => {});
  }, []);

  async function enregistrer(): Promise<void> {
    window.clearTimeout(minuterie.current);
    const v = vue.current;
    if (!v) return;
    const actuel = v.state.doc.toString();
    if (actuel === base.current) {
      setStatut((s) => (s.genre === "erreur" ? s : { genre: "enregistre" }));
      return;
    }
    // Un seul enregistrement à la fois : le suivant attend le précédent.
    if (enCours.current) await enCours.current;
    setStatut({ genre: "en-cours" });
    const travail = api
      .enregistrerNote(chemin, actuel, base.current)
      .then((id) => {
        historique.current = id;
        base.current = actuel;
        setStatut(v.state.doc.toString() === actuel ? { genre: "enregistre" } : { genre: "modifie" });
      })
      .catch((e) => setStatut({ genre: "erreur", texte: messageErreur(e) }))
      .finally(() => {
        enCours.current = null;
      });
    enCours.current = travail;
    return travail;
  }

  // Création de l'éditeur (une fois par note).
  useEffect(() => {
    if (!conteneur.current) return;
    const surChangement = (u: ViewUpdate) => {
      if (!u.docChanged) return;
      const t = u.state.doc.toString();
      setTexte(t);
      setStatut((s) => (s.genre === "erreur" ? s : { genre: "modifie" }));
      window.clearTimeout(minuterie.current);
      minuterie.current = window.setTimeout(() => enregistrer(), DELAI_ENREGISTREMENT);
    };
    const insererImages = (fichiers: File[], v: EditorView, position?: number) => {
      const images = fichiers.filter((f) => f.type.startsWith("image/"));
      if (!images.length) return false;
      if (position !== undefined) v.dispatch({ selection: { anchor: position } });
      Promise.all(images.map((f) => enregistrerImage(chemin, f)))
        .then((liens) => v.dispatch(inserer(v.state, liens.join("\n"))))
        .catch((e) => setStatut({ genre: "erreur", texte: messageErreur(e) }));
      return true;
    };
    const v = new EditorView({
      parent: conteneur.current,
      state: EditorState.create({
        doc: contenu,
        extensions: [
          history(),
          search(),
          markdown({ base: markdownLanguage }),
          syntaxHighlighting(coloration),
          enTeteYaml,
          autocompletion({ override: [completionLiens(() => titres.current)], icons: false }),
          EditorView.lineWrapping,
          placeholder("Écris ta note ici… (Markdown : **gras**, # Titre, - liste, [[lien]])"),
          theme,
          keymap.of([
            { key: "Mod-s", preventDefault: true, run: () => (enregistrer(), true) },
            { key: "Mod-b", run: (w) => (w.dispatch(entourer(w.state, "**")), true) },
            { key: "Mod-i", run: (w) => (w.dispatch(entourer(w.state, "*")), true) },
            { key: "Mod-k", run: (w) => (w.dispatch(inserer(w.state, "[[]]", 2)), true) },
            indentWithTab,
            ...defaultKeymap,
            ...historyKeymap,
            ...searchKeymap,
          ]),
          EditorView.updateListener.of(surChangement),
          EditorView.domEventHandlers({
            paste: (e, w) => {
              const fichiers = [...(e.clipboardData?.files ?? [])];
              if (insererImages(fichiers, w)) {
                e.preventDefault();
                return true;
              }
              return false;
            },
            drop: (e, w) => {
              const fichiers = [...(e.dataTransfer?.files ?? [])];
              const position = w.posAtCoords({ x: e.clientX, y: e.clientY }) ?? undefined;
              if (insererImages(fichiers, w, position)) {
                e.preventDefault();
                return true;
              }
              return false;
            },
          }),
        ],
      }),
    });
    vue.current = v;
    v.focus();
    return () => {
      // En quittant (autre note, autre écran) : les dernières modifications sont enregistrées.
      window.clearTimeout(minuterie.current);
      const actuel = v.state.doc.toString();
      if (actuel !== base.current) api.enregistrerNote(chemin, actuel, base.current).catch(() => {});
      v.destroy();
      vue.current = null;
    };
  }, [chemin]);

  // L'aperçu suit le texte avec un léger retard (le rendu Markdown coûte un peu).
  useEffect(() => {
    const t = window.setTimeout(() => setApercu(texte), 250);
    return () => window.clearTimeout(t);
  }, [texte]);

  const agir = (action: (s: EditorState) => Parameters<EditorView["dispatch"]>[0]) => {
    const v = vue.current;
    if (!v) return;
    v.dispatch(action(v.state));
    v.focus();
  };

  async function choisirImage() {
    const choix = await open({
      multiple: false,
      title: "Ajouter une image à la note",
      filters: [{ name: "Images", extensions: ["png", "jpg", "jpeg", "gif", "webp"] }],
    });
    if (typeof choix !== "string") return;
    try {
      const nom = await api.importerImage(chemin, choix);
      agir((s) => inserer(s, `![[${nom}]]`));
    } catch (e) {
      setStatut({ genre: "erreur", texte: messageErreur(e) });
    }
  }

  async function recharger() {
    try {
      const n = await api.note(chemin);
      base.current = n.contenu;
      const v = vue.current;
      v?.dispatch({ changes: { from: 0, to: v.state.doc.length, insert: n.contenu } });
      base.current = n.contenu;
      setStatut({ genre: "enregistre" });
    } catch (e) {
      setStatut({ genre: "erreur", texte: messageErreur(e) });
    }
  }

  async function terminer() {
    await enregistrer();
    if (enCours.current) await enCours.current;
    onFermer(historique.current);
  }

  const outils = useMemo(
    () => [
      { icone: Bold, titre: `Gras (${TOUCHE_COMMANDE} B)`, action: (s: EditorState) => entourer(s, "**") },
      { icone: Italic, titre: `Italique (${TOUCHE_COMMANDE} I)`, action: (s: EditorState) => entourer(s, "*") },
      { icone: Highlighter, titre: "Surligner", action: (s: EditorState) => entourer(s, "==") },
      { icone: Heading1, titre: "Grand titre", action: (s: EditorState) => titre(s, 1) },
      { icone: Heading2, titre: "Sous-titre", action: (s: EditorState) => titre(s, 2) },
      { icone: List, titre: "Liste à puces", action: (s: EditorState) => prefixer(s, "- ") },
      { icone: ListOrdered, titre: "Liste numérotée (étapes)", action: numeroter },
      { icone: Link2, titre: `Lien vers une note (${TOUCHE_COMMANDE} K)`, action: (s: EditorState) => inserer(s, "[[]]", 2) },
      { icone: Sigma, titre: "Formule (LaTeX)", action: (s: EditorState) => entourer(s, "$") },
      { icone: Table, titre: "Tableau", action: (s: EditorState) => inserer(s, TABLEAU, 2) },
    ],
    [],
  );

  return (
    <div className="editeur">
      <header className="entete-edition">
        <div className="entete-edition-titre">
          <span className="fil texte-doux petit">{fil}</span>
          <h1>{titreNote}</h1>
        </div>
        <span className={`statut-editeur statut-${statut.genre}`} role="status">
          {statut.genre === "en-cours" && <LoaderCircle size={13} className="tourne" aria-hidden />}
          {statut.genre === "enregistre" && <Check size={13} aria-hidden />}
          {statut.genre === "erreur" && <TriangleAlert size={13} aria-hidden />}
          {statut.genre === "enregistre" ? "Enregistré" : statut.genre === "en-cours" ? "Enregistrement…" : statut.genre === "modifie" ? "Modifié" : "Non enregistré"}
        </span>
        <span className="segmente" role="group" aria-label="Affichage">
          {(
            [
              ["ecrire", PencilLine, "Écrire"],
              ["les-deux", Columns2, "Écrire et voir"],
              ["apercu", Eye, "Aperçu"],
            ] as const
          ).map(([id, Icone, nom]) => (
            <button key={id} className={"segment" + (mode === id ? " actif" : "")} title={nom} aria-label={nom} aria-pressed={mode === id} onClick={() => setMode(id)}>
              <Icone size={15} aria-hidden />
            </button>
          ))}
        </span>
        <button className="bouton bouton-principal bouton-petit" onClick={terminer}>
          <Check size={14} aria-hidden /> Terminé
        </button>
      </header>
      <div className="barre-editeur" role="toolbar" aria-label="Mise en forme">
        {outils.map(({ icone: Icone, titre: t, action }) => (
          <button key={t} className="outil-editeur" title={t} aria-label={t} onMouseDown={(e) => e.preventDefault()} onClick={() => agir(action)}>
            <Icone size={16} aria-hidden />
          </button>
        ))}
        <span className="menu-encadres">
          <button
            className="outil-editeur avec-texte"
            title="Encadré (Définition, Exemple…)"
            aria-haspopup="menu"
            aria-expanded={menuEncadres}
            onMouseDown={(e) => e.preventDefault()}
            onClick={() => setMenuEncadres((m) => !m)}
          >
            <MessageSquareQuote size={16} aria-hidden /> Encadré
          </button>
          {menuEncadres && (
            <span className="liste-encadres" role="menu">
              {ENCADRES.map(([type, nom]) => (
                <button
                  key={type}
                  role="menuitem"
                  className={`encadre-${type}`}
                  onMouseDown={(e) => e.preventDefault()}
                  onClick={() => {
                    setMenuEncadres(false);
                    agir((s) => encadre(s, type, nom));
                  }}
                >
                  {nom}
                </button>
              ))}
            </span>
          )}
        </span>
        <button className="outil-editeur" title="Ajouter une image (ou colle-la directement)" aria-label="Ajouter une image" onClick={choisirImage}>
          <ImagePlus size={16} aria-hidden />
        </button>
      </div>

      {statut.genre === "erreur" && (
        <p className="message-erreur">
          {statut.texte}{" "}
          <button className="lien-discret" onClick={recharger}>
            Recharger la note
          </button>
        </p>
      )}

      <div className={`editeur-corps vue-${mode}`}>
        <div className="editeur-code" ref={conteneur} />
        {mode !== "ecrire" && (
          <div className="editeur-apercu">
            <MarkdownObsidian markdown={apercu} navigation={navigation} />
          </div>
        )}
      </div>
    </div>
  );
}
