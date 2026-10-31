// Choix des notes sur lesquelles l'IA s'appuie dans une conversation : matières entières, chapitres
// ou notes précises (arbre des notes, avec une recherche par titre).
import { useEffect, useMemo, useState } from "react";
import { ChevronDown, Search } from "lucide-react";
import Fenetre from "../composants/Fenetre";
import { api, messageErreur, type Arbre, type NoteResume, type SourceConversation } from "../lib/api";

/** Notes qui n'apprennent rien à l'IA (pages d'accueil, dessins seuls). */
const utile = (n: NoteResume) => n.genre !== "accueil" && n.genre !== "schema";

const cleChapitre = (matiere: string, chapitre: string | null) => `c|${matiere}|${chapitre ?? ""}`;
const cleNote = (chemin: string) => `n|${chemin}`;

/** Sources enregistrées → cases cochées. */
function versCles(sources: SourceConversation[]): Set<string> {
  return new Set(sources.map((s) => (s.note ? cleNote(s.note) : cleChapitre(s.matiere ?? "", s.chapitre ?? null))));
}

/** Cases cochées → sources (chapitres entiers, ou notes seules si leur chapitre n'est pas coché). */
function versSources(cles: Set<string>, arbre: Arbre): SourceConversation[] {
  const sources: SourceConversation[] = [];
  for (const m of arbre.matieres) {
    const groupes: [string | null, NoteResume[]][] = [[null, m.notes], ...m.chapitres.map((c) => [c.nom, c.notes] as [string, NoteResume[]])];
    for (const [chapitre, notes] of groupes) {
      if (cles.has(cleChapitre(m.nom, chapitre))) sources.push({ matiere: m.nom, chapitre });
      else for (const n of notes) if (cles.has(cleNote(n.chemin))) sources.push({ note: n.chemin });
    }
  }
  for (const g of arbre.autres) for (const n of g.notes) if (cles.has(cleNote(n.chemin))) sources.push({ note: n.chemin });
  return sources;
}

/** « 2 chapitres, 3 notes » (résumé des sources d'une conversation). */
export function resumeSources(sources: SourceConversation[]): string {
  const notes = sources.filter((s) => s.note).length;
  const chapitres = sources.length - notes;
  const parties = [];
  if (chapitres) parties.push(`${chapitres} chapitre${chapitres > 1 ? "s" : ""}`);
  if (notes) parties.push(`${notes} note${notes > 1 ? "s" : ""}`);
  return parties.length ? parties.join(", ") : "aucune note";
}

interface Props {
  titre: string;
  sources: SourceConversation[];
  onValider: (sources: SourceConversation[]) => void;
  onFermer: () => void;
}

export default function ChoixSources({ titre, sources, onValider, onFermer }: Props) {
  const [arbre, setArbre] = useState<Arbre | null>(null);
  const [erreur, setErreur] = useState<string | null>(null);
  const [cles, setCles] = useState(() => versCles(sources));
  const [ouverts, setOuverts] = useState<Set<string>>(new Set());
  const [filtre, setFiltre] = useState("");

  useEffect(() => {
    api.arbre().then(setArbre).catch((e) => setErreur(messageErreur(e)));
  }, []);

  const basculer = (liste: string[], coche: boolean) =>
    setCles((c) => {
      const n = new Set(c);
      liste.forEach((k) => (coche ? n.add(k) : n.delete(k)));
      return n;
    });
  const deplier = (k: string) =>
    setOuverts((o) => {
      const n = new Set(o);
      if (n.has(k)) n.delete(k);
      else n.add(k);
      return n;
    });

  const recherche = filtre.trim().toLowerCase();
  const garder = (n: NoteResume) => utile(n) && (!recherche || n.titre.toLowerCase().includes(recherche));
  const choisies = useMemo(() => (arbre ? versSources(cles, arbre) : []), [cles, arbre]);

  /** Une ligne de notes (chapitre ou notes générales) avec sa case et ses notes dépliables. Appelée comme une
   *  fonction (et non comme un composant) pour ne pas recréer les cases à chaque clic. */
  function groupe(matiere: string | null, chapitre: string | null, notes: NoteResume[], nom: string) {
    const visibles = notes.filter(garder);
    if (!visibles.length) return null;
    const cleGroupe = matiere !== null ? cleChapitre(matiere, chapitre) : null;
    const entier = cleGroupe !== null && cles.has(cleGroupe);
    const nbNotes = visibles.filter((n) => cles.has(cleNote(n.chemin))).length;
    const k = `${matiere}|${chapitre}|${nom}`;
    const ouvert = ouverts.has(k) || !!recherche;
    return (
      <div className="source-groupe" key={k}>
        <div className="source-ligne">
          {cleGroupe ? (
            <label className="case">
              <input type="checkbox" checked={entier} onChange={(e) => basculer([cleGroupe], e.target.checked)} />
              <span>{nom}</span>
            </label>
          ) : (
            <span className="source-nom">{nom}</span>
          )}
          <span className="texte-doux petit">{!entier && nbNotes > 0 ? `${nbNotes} note${nbNotes > 1 ? "s" : ""} choisie${nbNotes > 1 ? "s" : ""}` : ""}</span>
          <button className="lien-discret petit" aria-expanded={ouvert} onClick={() => deplier(k)}>
            {visibles.length} note{visibles.length > 1 ? "s" : ""}
            <ChevronDown size={14} className={"icone chevron" + (ouvert ? " ouvert" : "")} aria-hidden />
          </button>
        </div>
        {ouvert && (
          <div className="source-notes">
            {visibles.map((n) => (
              <label key={n.chemin} className="case">
                <input
                  type="checkbox"
                  checked={entier || cles.has(cleNote(n.chemin))}
                  disabled={entier}
                  onChange={(e) => basculer([cleNote(n.chemin)], e.target.checked)}
                />
                <span>{n.titre}</span>
              </label>
            ))}
          </div>
        )}
      </div>
    );
  }

  return (
    <Fenetre
      titre={titre}
      onFermer={onFermer}
      actions={
        <>
          <span className="texte-doux petit espace">L'IA s'appuiera sur : {resumeSources(choisies)}</span>
          <button className="bouton" onClick={() => setCles(new Set())} disabled={!cles.size}>
            Tout décocher
          </button>
          <button className="bouton bouton-principal" onClick={() => onValider(choisies)}>
            Valider
          </button>
        </>
      }
    >
      <p className="texte-doux petit">
        Coche des chapitres entiers ou déplie-les pour choisir des notes précises. Sans rien cocher, l'IA explique avec
        ses connaissances générales (tout sera marqué « hors de tes notes »).
      </p>
      <label className="champ-recherche">
        <Search size={15} className="icone" aria-hidden />
        <input className="champ" placeholder="Chercher une note…" value={filtre} onChange={(e) => setFiltre(e.target.value)} />
      </label>
      {erreur && <p className="message-erreur">{erreur}</p>}
      {!arbre && !erreur && <p className="progression-texte">Chargement des notes…</p>}
      <div className="choix-sources">
        {arbre?.matieres.map((m) => {
          const groupes = [{ chapitre: null as string | null, notes: m.notes, nom: "Notes générales" }].concat(
            m.chapitres.map((c) => ({ chapitre: c.nom, notes: c.notes, nom: c.nom })),
          );
          const visibles = groupes.filter((g) => g.notes.some(garder));
          if (!visibles.length) return null;
          const clesMatiere = visibles.map((g) => cleChapitre(m.nom, g.chapitre));
          const nb = clesMatiere.filter((k) => cles.has(k)).length;
          return (
            <section key={m.nom} className="source-matiere">
              <label className="case source-titre-matiere">
                <input
                  type="checkbox"
                  checked={nb === clesMatiere.length}
                  ref={(el) => {
                    if (el) el.indeterminate = nb > 0 && nb < clesMatiere.length;
                  }}
                  onChange={(e) => basculer(clesMatiere, e.target.checked)}
                />
                <strong>{m.nom}</strong>
              </label>
              {visibles.map((g) => groupe(m.nom, g.chapitre, g.notes, g.nom))}
            </section>
          );
        })}
        {arbre && arbre.autres.some((g) => g.notes.some(garder)) && (
          <section className="source-matiere">
            <strong className="source-titre-matiere">Hors matière</strong>
            {arbre.autres.map((g) => groupe(null, null, g.notes, g.nom))}
          </section>
        )}
      </div>
    </Fenetre>
  );
}
