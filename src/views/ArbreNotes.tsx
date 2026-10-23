import { useEffect, useMemo, useState } from "react";
import {
  BookA,
  Compass,
  FileText,
  Folder,
  House,
  Languages,
  type LucideIcon,
  NotebookText,
  ScrollText,
  Search,
  Shapes,
} from "lucide-react";
import { api, Arbre, EtatIndexation, GenreNote, NoteResume } from "../lib/api";

interface Props {
  cheminActif: string | null;
  onOuvrir: (chemin: string) => void;
  indexation: EtatIndexation | null;
}

const trieur = new Intl.Collator("fr", { numeric: true, sensitivity: "base" });

/** Ordre dans un chapitre : fiche de synthèse d'abord, schémas à la fin. */
const RANG: Partial<Record<GenreNote, number>> = { accueil: 0, fiche_synthese: 1, schema: 9 };

function trierNotes(notes: NoteResume[]) {
  return [...notes].sort((a, b) => (RANG[a.genre] ?? 5) - (RANG[b.genre] ?? 5) || trieur.compare(a.titre, b.titre));
}

export const BADGES: Partial<Record<GenreNote, string>> = {
  fiche_synthese: "Fiche",
  schema: "Schéma",
  vocabulaire: "Vocab",
  methodologie: "Méthode",
  reference: "Référence",
  etude_lineaire: "Étude",
};

/** Icône de chaque genre de note dans la liste. */
const ICONES_GENRE: Partial<Record<GenreNote, LucideIcon>> = {
  fiche_synthese: ScrollText,
  schema: Shapes,
  vocabulaire: Languages,
  methodologie: Compass,
  reference: BookA,
  etude_lineaire: NotebookText,
  accueil: House,
};

function sansAccents(t: string) {
  return t.normalize("NFD").replace(/\p{Diacritic}/gu, "").toLowerCase();
}

export default function ArbreNotes({ cheminActif, onOuvrir, indexation }: Props) {
  const [arbre, setArbre] = useState<Arbre | null>(null);
  const [filtre, setFiltre] = useState("");

  useEffect(() => {
    if (!indexation?.enCours) api.arbre().then(setArbre).catch(() => setArbre(null));
  }, [indexation?.enCours, indexation?.dernier]);

  const f = sansAccents(filtre.trim());
  const garder = (n: NoteResume) => !f || sansAccents(n.titre).includes(f);

  const matieres = useMemo(
    () => [...(arbre?.matieres ?? [])].sort((a, b) => trieur.compare(a.nom, b.nom)),
    [arbre],
  );

  if (!arbre) return <div className="arbre texte-doux petit">Chargement…</div>;

  const ligne = (n: NoteResume) => (
    <li key={n.chemin}>
      <button
        className={"note-arbre" + (n.chemin === cheminActif ? " actif" : "")}
        onClick={() => onOuvrir(n.chemin)}
        title={n.titre}
      >
        {(() => {
          const Icone = ICONES_GENRE[n.genre] ?? FileText;
          return <Icone size={14} className="icone note-icone" aria-hidden />;
        })()}
        <span className="note-titre">{n.genre === "accueil" ? "Accueil de la matière" : n.titre}</span>
        {BADGES[n.genre] && <span className={`badge badge-${n.genre}`}>{BADGES[n.genre]}</span>}
      </button>
    </li>
  );

  // Avec un filtre, tout est déplié pour montrer les résultats.
  const ouvert = (contientActif: boolean) => !!f || contientActif;

  return (
    <div className="arbre">
      <div className="champ-icone">
        <Search size={15} className="icone" aria-hidden />
        <input
          className="champ"
          type="search"
          placeholder="Filtrer les notes…"
          value={filtre}
          onChange={(e) => setFiltre(e.target.value)}
        />
      </div>
      {matieres.map((m) => {
        const notes = trierNotes(m.notes.filter(garder));
        const chapitres = [...m.chapitres]
          .sort((a, b) => trieur.compare(a.nom, b.nom))
          .map((c) => ({ ...c, notes: trierNotes(c.notes.filter(garder)) }))
          .filter((c) => c.notes.length);
        if (f && !notes.length && !chapitres.length) return null;
        const actifIci = !!cheminActif && cheminActif.startsWith(m.nom + "/");
        return (
          <details key={m.nom} className="matiere" open={ouvert(actifIci)}>
            <summary>
              <Folder size={14} className="icone" aria-hidden /> {m.nom}
            </summary>
            <ul>{notes.map(ligne)}</ul>
            {chapitres.map((c) => (
              <details key={c.nom} className="chapitre" open={ouvert(!!cheminActif?.startsWith(`${m.nom}/${c.nom}/`))}>
                <summary>{c.nom}</summary>
                <ul>{c.notes.map(ligne)}</ul>
              </details>
            ))}
          </details>
        );
      })}
      {arbre.autres.length > 0 && (
        <details className="matiere" open={!!f}>
          <summary>Autres dossiers</summary>
          {[...arbre.autres]
            .sort((a, b) => trieur.compare(a.nom, b.nom))
            .map((g) => {
              const notes = trierNotes(g.notes.filter(garder));
              if (!notes.length) return null;
              return (
                <details key={g.nom} className="chapitre" open={!!f}>
                  <summary>{g.nom}</summary>
                  <ul>{notes.map(ligne)}</ul>
                </details>
              );
            })}
        </details>
      )}
    </div>
  );
}
