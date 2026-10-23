import { useEffect, useMemo, useRef, useState } from "react";
import { Search, SearchX } from "lucide-react";
import EntetePage from "../composants/EntetePage";
import katex from "katex";
import { api, messageErreur, ResultatRecherche, Statistiques } from "../lib/api";
import { latexEnTexte, TexteMaths } from "../revision/symboles";
import { BADGES } from "./ArbreNotes";
import { TOUCHE_COMMANDE } from "../lib/plateforme";

interface Props {
  onOuvrir: (chemin: string, ancre?: string | null) => void;
  /** Texte gardé quand on quitte l'écran puis qu'on y revient. */
  texte: string;
  onTexte: (t: string) => void;
}

/** Texte ordinaire de l'extrait : mots trouvés surlignés ; un morceau de formule coupé est rendu lisible. */
function Surligne({ texte }: { texte: string }) {
  const lisible = texte.includes("$") || texte.includes("\\") ? latexEnTexte(texte) : texte;
  return (
    <>
      {lisible
        .split(/(\u0001[^\u0002]*\u0002)/)
        .map((m, i) => (m.startsWith("\u0001") ? <mark key={i}>{m.slice(1, -1)}</mark> : <span key={i}>{m}</span>))}
    </>
  );
}

/**
 * L'extrait arrive avec des marqueurs \u0001…\u0002 autour des mots trouvés. Les formules complètes sont rendues
 * avec KaTeX (surlignées en entier si un mot cherché s'y trouve) ; le reste est surligné sans HTML.
 */
function Extrait({ texte }: { texte: string }) {
  return (
    <>
      {texte.split(/(\$\$[^$]+\$\$|\$[^$\n]+\$)/g).map((m, i) => {
        // Le découpage met les formules complètes (capturées) aux indices impairs.
        if (i % 2 === 0) return <Surligne key={i} texte={m} />;
        const formule = m.replace(/^\$+|\$+$/g, "");
        const trouve = formule.includes("\u0001");
        const html = katex.renderToString(formule.replace(/[\u0001\u0002]/g, ""), { throwOnError: false, output: "html" });
        const rendu = <span dangerouslySetInnerHTML={{ __html: html }} />;
        return trouve ? <mark key={i}>{rendu}</mark> : <span key={i}>{rendu}</span>;
      })}
    </>
  );
}

export default function RechercheVue({ onOuvrir, texte, onTexte }: Props) {
  const [matiere, setMatiere] = useState("");
  const [matieres, setMatieres] = useState<string[]>([]);
  const [resultats, setResultats] = useState<ResultatRecherche[]>([]);
  const [erreur, setErreur] = useState<string | null>(null);
  const [selection, setSelection] = useState(0);
  const champ = useRef<HTMLInputElement>(null);

  useEffect(() => {
    champ.current?.focus();
    api.statistiques().then((s: Statistiques) => setMatieres(s.matieres.map((m) => m.matiere)));
  }, []);

  // Recherche pendant la frappe, avec un petit délai pour ne pas lancer une requête par lettre.
  useEffect(() => {
    if (!texte.trim()) {
      setResultats([]);
      return;
    }
    const minuteur = setTimeout(() => {
      api
        .rechercher(texte, matiere || null)
        .then((r) => {
          setResultats(r);
          setSelection(0);
          setErreur(null);
        })
        .catch((e) => setErreur(messageErreur(e)));
    }, 120);
    return () => clearTimeout(minuteur);
  }, [texte, matiere]);

  // Regroupe les passages par note, en gardant l'ordre de pertinence.
  const groupes = useMemo(() => {
    const parNote = new Map<string, ResultatRecherche[]>();
    for (const r of resultats) parNote.set(r.chemin, [...(parNote.get(r.chemin) ?? []), r]);
    return [...parNote.values()];
  }, [resultats]);

  function ouvrir(r: ResultatRecherche) {
    onOuvrir(r.chemin, r.section || null);
  }

  function clavier(e: React.KeyboardEvent) {
    if (e.key === "ArrowDown") {
      e.preventDefault();
      setSelection((s) => Math.min(s + 1, groupes.length - 1));
    } else if (e.key === "ArrowUp") {
      e.preventDefault();
      setSelection((s) => Math.max(s - 1, 0));
    } else if (e.key === "Enter" && groupes[selection]) {
      ouvrir(groupes[selection][0]);
    }
  }

  return (
    <div className="page">
      <EntetePage icone={Search} surTitre="Ressources" titre="Recherche" description="Dans toutes tes notes, sans Internet." />
      <div className="rangee recherche-barre">
        <div className="champ-icone champ-icone-large">
          <Search size={18} className="icone" aria-hidden />
          <input
          ref={champ}
          className="champ"
          type="search"
          placeholder="Chercher dans toutes les notes (sans Internet)…"
          value={texte}
          onChange={(e) => onTexte(e.target.value)}
          onKeyDown={clavier}
          />
        </div>
        <select value={matiere} onChange={(e) => setMatiere(e.target.value)}>
          <option value="">Toutes les matières</option>
          {matieres.map((m) => (
            <option key={m} value={m}>
              {m}
            </option>
          ))}
        </select>
      </div>
      <p className="texte-doux petit">
        Accents et majuscules ignorés · début de mot suffisant · <kbd>↑</kbd> <kbd>↓</kbd> puis <kbd>Entrée</kbd> pour
        ouvrir · <kbd>{TOUCHE_COMMANDE} K</kbd> depuis n'importe où
      </p>

      {erreur && <p className="message-erreur">{erreur}</p>}
      {texte.trim() && !erreur && groupes.length === 0 && (
        <div className="carte etat-vide apparait">
          <SearchX size={30} strokeWidth={1.6} aria-hidden />
          <p>Aucun résultat.</p>
        </div>
      )}

      <ol className="resultats">
        {groupes.map((passages, i) => {
          const n = passages[0];
          return (
            <li key={n.chemin} className={"carte resultat" + (i === selection ? " selectionne" : "")}>
              <button className="resultat-titre" onClick={() => ouvrir(n)}>
                <span>{n.titre}</span>
                {BADGES[n.genre] && <span className={`badge badge-${n.genre}`}>{BADGES[n.genre]}</span>}
              </button>
              <div className="texte-doux petit">{[n.matiere, n.chapitre].filter(Boolean).join(" › ") || "Hors matière"}</div>
              {passages.slice(0, 3).map((p, j) => (
                <button key={j} className="passage" onClick={() => ouvrir(p)}>
                  {p.section && (
                    <span className="passage-section">
                      <TexteMaths texte={p.section} />
                    </span>
                  )}
                  <span className="passage-extrait">
                    <Extrait texte={p.extrait} />
                  </span>
                </button>
              ))}
              {passages.length > 3 && (
                <div className="texte-doux petit">+ {passages.length - 3} autre(s) passage(s)</div>
              )}
            </li>
          );
        })}
      </ol>
    </div>
  );
}
