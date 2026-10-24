// Arbre des notes : dossiers (à toute profondeur, même vides) et notes. Quand l'écriture est autorisée,
// on y crée, renomme, déplace et supprime notes et dossiers (menu « ⋯ » ou clic droit), avec une corbeille.
import { type MouseEvent as ReactMouseEvent, useEffect, useMemo, useState } from "react";
import {
  ArchiveRestore,
  BookA,
  Compass,
  FilePlus,
  FileText,
  Folder,
  FolderInput,
  FolderPlus,
  House,
  Languages,
  type LucideIcon,
  MoreHorizontal,
  NotebookText,
  PencilLine,
  ScrollText,
  Search,
  Shapes,
  Trash2,
} from "lucide-react";
import { api, Arbre, ElementCorbeille, EtatIndexation, GenreNote, messageErreur, NoteResume } from "../lib/api";
import Fenetre from "../composants/Fenetre";

interface Props {
  cheminActif: string | null;
  onOuvrir: (chemin: string) => void;
  indexation: EtatIndexation | null;
  ecritureAutorisee: boolean;
  onAutoriser: () => void;
  /** Note créée : l'ouvrir en mode édition. */
  onNoteCreee: (chemin: string) => void;
  /** Un élément a changé de place (renommé, déplacé) : `nouveau` remplace `ancien`. */
  onDeplace: (ancien: string, nouveau: string) => void;
  /** Un élément est parti à la corbeille. */
  onSupprime: (chemin: string) => void;
}

const trieur = new Intl.Collator("fr", { numeric: true, sensitivity: "base" });

/** Ordre dans un dossier : accueil et fiche de synthèse d'abord, schémas à la fin. */
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

/** Un dossier de l'arbre. */
interface Noeud {
  nom: string;
  chemin: string;
  dossiers: Noeud[];
  notes: NoteResume[];
}

/** Arbre des dossiers à partir des notes indexées et de la liste des dossiers (même vides). */
export function construireArbre(notes: NoteResume[], dossiers: string[]): Noeud {
  const racine: Noeud = { nom: "", chemin: "", dossiers: [], notes: [] };
  const index = new Map<string, Noeud>([["", racine]]);
  const dossier = (chemin: string): Noeud => {
    const existant = index.get(chemin);
    if (existant) return existant;
    const i = chemin.lastIndexOf("/");
    const parent = dossier(i < 0 ? "" : chemin.slice(0, i));
    const noeud: Noeud = { nom: chemin.slice(i + 1), chemin, dossiers: [], notes: [] };
    parent.dossiers.push(noeud);
    index.set(chemin, noeud);
    return noeud;
  };
  dossiers.forEach(dossier);
  for (const n of notes) {
    const i = n.chemin.lastIndexOf("/");
    dossier(i < 0 ? "" : n.chemin.slice(0, i)).notes.push(n);
  }
  const trier = (n: Noeud) => {
    n.dossiers.sort((a, b) => trieur.compare(a.nom, b.nom));
    n.notes = trierNotes(n.notes);
    n.dossiers.forEach(trier);
  };
  trier(racine);
  return racine;
}

/** Toutes les notes de l'arbre indexé (matières, chapitres, autres dossiers). */
function toutesLesNotes(arbre: Arbre): NoteResume[] {
  return [
    ...arbre.matieres.flatMap((m) => [...m.notes, ...m.chapitres.flatMap((c) => c.notes)]),
    ...arbre.autres.flatMap((g) => g.notes),
  ];
}

type Dialogue =
  | { genre: "note"; dossier: string }
  | { genre: "dossier"; parent: string }
  | { genre: "deplacer"; chemin: string; estDossier: boolean };

interface Menu {
  chemin: string;
  estDossier: boolean;
  x: number;
  y: number;
}

export default function ArbreNotes(props: Props) {
  const { cheminActif, onOuvrir, indexation, ecritureAutorisee } = props;
  const [arbre, setArbre] = useState<Arbre | null>(null);
  const [dossiers, setDossiers] = useState<string[]>([]);
  const [filtre, setFiltre] = useState("");
  const [menu, setMenu] = useState<Menu | null>(null);
  const [renommage, setRenommage] = useState<string | null>(null);
  const [dialogue, setDialogue] = useState<Dialogue | null>(null);
  const [corbeille, setCorbeille] = useState<ElementCorbeille[] | null>(null);
  const [message, setMessage] = useState<{ texte: string; erreur: boolean } | null>(null);
  const [version, setVersion] = useState(0);

  useEffect(() => {
    if (indexation?.enCours) return;
    api.arbre().then(setArbre).catch(() => setArbre(null));
    api.dossiersNotes().then(setDossiers).catch(() => setDossiers([]));
  }, [indexation?.enCours, indexation?.dernier, version]);

  // Le menu se ferme au clic ailleurs ou avec Échap.
  useEffect(() => {
    if (!menu) return;
    const fermer = () => setMenu(null);
    const touche = (e: KeyboardEvent) => e.key === "Escape" && setMenu(null);
    window.addEventListener("click", fermer);
    window.addEventListener("keydown", touche);
    return () => {
      window.removeEventListener("click", fermer);
      window.removeEventListener("keydown", touche);
    };
  }, [menu]);

  const racine = useMemo(() => (arbre ? construireArbre(toutesLesNotes(arbre), dossiers) : null), [arbre, dossiers]);

  if (!arbre || !racine) return <div className="arbre texte-doux petit">Chargement…</div>;

  const f = sansAccents(filtre.trim());
  const garder = (n: NoteResume) => !f || sansAccents(n.titre).includes(f);
  /** Un dossier a-t-il une note qui correspond au filtre ? */
  const contient = (d: Noeud): boolean => d.notes.some(garder) || d.dossiers.some(contient);

  async function agir(action: () => Promise<string | void>, apres?: (r: string | void) => void) {
    setMessage(null);
    try {
      const r = await action();
      apres?.(r);
      setVersion((v) => v + 1);
    } catch (e) {
      setMessage({ texte: messageErreur(e), erreur: true });
    }
  }

  function ouvrirMenu(e: ReactMouseEvent, chemin: string, estDossier: boolean) {
    if (!ecritureAutorisee) return;
    e.preventDefault();
    e.stopPropagation();
    const r = (e.currentTarget as HTMLElement).getBoundingClientRect();
    setMenu({ chemin, estDossier, x: e.type === "contextmenu" ? e.clientX : r.right, y: e.type === "contextmenu" ? e.clientY : r.bottom });
  }

  function supprimer(chemin: string, estDossier: boolean) {
    const nom = chemin.split("/").pop()?.replace(/\.md$/i, "");
    const quoi = estDossier ? `le dossier « ${nom} » et tout son contenu` : `« ${nom} »`;
    if (!window.confirm(`Mettre ${quoi} à la corbeille de Révisio ? Tu pourras le restaurer.`)) return;
    agir(
      () => api.supprimerElement(chemin),
      (r) => {
        setMessage({ texte: String(r), erreur: false });
        props.onSupprime(chemin);
      },
    );
  }

  const boutonActions = (chemin: string, estDossier: boolean) =>
    ecritureAutorisee && (
      <button
        className="actions-arbre"
        aria-label="Actions"
        title="Actions"
        onClick={(e) => ouvrirMenu(e, chemin, estDossier)}
      >
        <MoreHorizontal size={15} aria-hidden />
      </button>
    );

  const champRenommage = (chemin: string, valeur: string) => (
    <input
      className="champ champ-renommage"
      defaultValue={valeur}
      autoFocus
      onFocus={(e) => e.target.select()}
      onClick={(e) => e.stopPropagation()}
      onKeyDown={(e) => {
        if (e.key === "Escape") setRenommage(null);
        if (e.key === "Enter") {
          const nom = (e.target as HTMLInputElement).value;
          setRenommage(null);
          agir(
            () => api.renommerElement(chemin, nom),
            (nouveau) => props.onDeplace(chemin, String(nouveau)),
          );
        }
      }}
      onKeyUp={(e) => e.preventDefault()}
      onBlur={() => setRenommage(null)}
      aria-label="Nouveau nom"
    />
  );

  const ligne = (n: NoteResume) => {
    if (!garder(n)) return null;
    const Icone = ICONES_GENRE[n.genre] ?? FileText;
    return (
      <li key={n.chemin} className="ligne-arbre" onContextMenu={(e) => ouvrirMenu(e, n.chemin, false)}>
        {renommage === n.chemin ? (
          champRenommage(n.chemin, n.titre)
        ) : (
          <button
            className={"note-arbre" + (n.chemin === cheminActif ? " actif" : "")}
            onClick={() => onOuvrir(n.chemin)}
            title={n.titre}
          >
            <Icone size={14} className="icone note-icone" aria-hidden />
            <span className="note-titre">{n.genre === "accueil" ? "Accueil" : n.titre}</span>
            {BADGES[n.genre] && <span className={`badge badge-${n.genre}`}>{BADGES[n.genre]}</span>}
          </button>
        )}
        {boutonActions(n.chemin, false)}
      </li>
    );
  };

  const dossierArbre = (d: Noeud, profondeur: number) => {
    if (f && !contient(d)) return null;
    const contientActif = !!cheminActif?.startsWith(d.chemin + "/");
    return (
      <details key={d.chemin} className={profondeur === 0 ? "matiere" : "chapitre"} open={!!f || contientActif || undefined}>
        <summary onContextMenu={(e) => ouvrirMenu(e, d.chemin, true)}>
          <Folder size={14} className="icone" aria-hidden />
          {renommage === d.chemin ? champRenommage(d.chemin, d.nom) : <span className="nom-dossier">{d.nom}</span>}
          {boutonActions(d.chemin, true)}
        </summary>
        <ul>{d.notes.map(ligne)}</ul>
        {d.dossiers.map((s) => dossierArbre(s, profondeur + 1))}
      </details>
    );
  };

  const element = menu?.chemin ?? "";
  const nomElement = element.split("/").pop() ?? "";

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

      {ecritureAutorisee ? (
        <div className="rangee actions-notes">
          <button
            className="bouton bouton-petit"
            onClick={() => setDialogue({ genre: "note", dossier: dossierDe(cheminActif) })}
          >
            <FilePlus size={14} aria-hidden /> Note
          </button>
          <button className="bouton bouton-petit" onClick={() => setDialogue({ genre: "dossier", parent: "" })}>
            <FolderPlus size={14} aria-hidden /> Dossier
          </button>
        </div>
      ) : (
        <button className="lien-discret actions-notes" onClick={props.onAutoriser}>
          <PencilLine size={13} aria-hidden /> Autoriser l'écriture pour créer et ranger tes notes
        </button>
      )}
      {message && <p className={message.erreur ? "message-erreur petit" : "texte-doux petit"}>{message.texte}</p>}

      <ul>{racine.notes.map(ligne)}</ul>
      {racine.dossiers.map((d) => dossierArbre(d, 0))}

      {ecritureAutorisee && (
        <button className="lien-discret lien-corbeille" onClick={() => api.corbeille().then(setCorbeille)}>
          <Trash2 size={13} aria-hidden /> Corbeille
        </button>
      )}

      {menu && (
        <div className="menu-arbre" style={{ left: menu.x, top: menu.y }} role="menu" onClick={(e) => e.stopPropagation()}>
          {menu.estDossier && (
            <>
              <button role="menuitem" onClick={() => (setMenu(null), setDialogue({ genre: "note", dossier: element }))}>
                <FilePlus size={14} aria-hidden /> Nouvelle note ici
              </button>
              <button role="menuitem" onClick={() => (setMenu(null), setDialogue({ genre: "dossier", parent: element }))}>
                <FolderPlus size={14} aria-hidden /> Nouveau sous-dossier
              </button>
            </>
          )}
          <button role="menuitem" onClick={() => (setMenu(null), setRenommage(element))}>
            <PencilLine size={14} aria-hidden /> Renommer
          </button>
          <button
            role="menuitem"
            onClick={() => (setMenu(null), setDialogue({ genre: "deplacer", chemin: element, estDossier: menu.estDossier }))}
          >
            <FolderInput size={14} aria-hidden /> Déplacer…
          </button>
          <button role="menuitem" className="danger" onClick={() => (setMenu(null), supprimer(element, menu.estDossier))}>
            <Trash2 size={14} aria-hidden /> Mettre à la corbeille
          </button>
          <span className="sr-only">{nomElement}</span>
        </div>
      )}

      {dialogue?.genre === "note" && (
        <DialogueNote
          dossiers={dossiers}
          dossier={dialogue.dossier}
          onFermer={() => setDialogue(null)}
          onCreer={(dossier, titre, modele) =>
            agir(
              () => api.creerNote(dossier, titre, modele),
              (chemin) => {
                setDialogue(null);
                props.onNoteCreee(String(chemin));
              },
            )
          }
        />
      )}
      {dialogue?.genre === "dossier" && (
        <DialogueDossier
          parent={dialogue.parent}
          onFermer={() => setDialogue(null)}
          onCreer={(chemin) => agir(() => api.creerDossier(chemin), () => setDialogue(null))}
        />
      )}
      {dialogue?.genre === "deplacer" && (
        <DialogueDeplacer
          chemin={dialogue.chemin}
          dossiers={dossiers.filter(
            (d) => !dialogue.estDossier || (d !== dialogue.chemin && !d.startsWith(dialogue.chemin + "/")),
          )}
          onFermer={() => setDialogue(null)}
          onDeplacer={(destination) =>
            agir(
              () => api.deplacerElement(dialogue.chemin, destination),
              (nouveau) => {
                setDialogue(null);
                props.onDeplace(dialogue.chemin, String(nouveau));
              },
            )
          }
        />
      )}
      {corbeille && (
        <Fenetre titre="Corbeille de Révisio" onFermer={() => setCorbeille(null)}>
          {corbeille.length === 0 ? (
            <p className="texte-doux">La corbeille est vide.</p>
          ) : (
            <ul className="liste-corbeille">
              {corbeille.map((c) => (
                <li key={c.groupe}>
                  {c.dossier ? <Folder size={15} aria-hidden /> : <FileText size={15} aria-hidden />}
                  <span className="journal-texte">
                    <strong>{c.chemin.split("/").pop()?.replace(/\.md$/i, "")}</strong>
                    <span className="texte-doux petit">
                      {c.chemin}
                      {c.dossier ? `, ${c.fichiers} fichier${c.fichiers > 1 ? "s" : ""}` : ""}, supprimé le{" "}
                      {new Date(c.date).toLocaleString("fr-FR", { dateStyle: "medium", timeStyle: "short" })}
                    </span>
                  </span>
                  <button
                    className="bouton bouton-petit"
                    onClick={() =>
                      agir(
                        () => api.restaurer(c.groupe),
                        (r) => {
                          setMessage({ texte: String(r), erreur: false });
                          api.corbeille().then(setCorbeille);
                        },
                      )
                    }
                  >
                    <ArchiveRestore size={14} aria-hidden /> Restaurer
                  </button>
                </li>
              ))}
            </ul>
          )}
        </Fenetre>
      )}
    </div>
  );
}

/** Dossier d'une note (ou chaîne vide pour la racine). */
function dossierDe(chemin: string | null): string {
  if (!chemin) return "";
  const i = chemin.lastIndexOf("/");
  return i < 0 ? "" : chemin.slice(0, i);
}

const MODELES: [string, string, string][] = [
  ["cours", "Note de cours", "Un encadré Définition (ses mots en gras deviennent des textes à trous) et des notes liées."],
  ["synthese", "Fiche de synthèse", "Le résumé du chapitre : tous ses mots en gras deviennent des textes à trous."],
  ["vocabulaire", "Vocabulaire", "Une ligne « * mot = traduction » par mot : chacune devient une carte."],
  ["vide", "Note vide", "Une page blanche."],
];

function ChoixDossier({ dossiers, valeur, onChange }: { dossiers: string[]; valeur: string; onChange: (d: string) => void }) {
  return (
    <select id="dossier" value={valeur} onChange={(e) => onChange(e.target.value)}>
      <option value="">(racine du dossier de notes)</option>
      {dossiers.map((d) => (
        <option key={d} value={d}>
          {d.split("/").join(" › ")}
        </option>
      ))}
    </select>
  );
}

function DialogueNote({
  dossiers,
  dossier: initial,
  onFermer,
  onCreer,
}: {
  dossiers: string[];
  dossier: string;
  onFermer: () => void;
  onCreer: (dossier: string, titre: string, modele: string) => void;
}) {
  const [titre, setTitre] = useState("");
  const [dossier, setDossier] = useState(initial);
  const [modele, setModele] = useState("cours");
  const creer = () => titre.trim() && onCreer(dossier, titre, modele);
  return (
    <Fenetre
      titre="Nouvelle note"
      onFermer={onFermer}
      actions={
        <>
          <button className="bouton" onClick={onFermer}>
            Annuler
          </button>
          <button className="bouton bouton-principal" disabled={!titre.trim()} onClick={creer}>
            <FilePlus size={15} aria-hidden /> Créer
          </button>
        </>
      }
    >
      <label htmlFor="titre-note">Titre</label>
      <input
        id="titre-note"
        className="champ"
        value={titre}
        placeholder="Par exemple : La photosynthèse"
        onChange={(e) => setTitre(e.target.value)}
        onKeyDown={(e) => e.key === "Enter" && creer()}
      />
      <label htmlFor="dossier">Dossier (matière › chapitre)</label>
      <ChoixDossier dossiers={dossiers} valeur={dossier} onChange={setDossier} />
      <fieldset className="choix-modele">
        <legend>Modèle</legend>
        {MODELES.map(([id, nom, aide]) => (
          <label key={id} className={"option-modele" + (modele === id ? " choisie" : "")}>
            <input type="radio" name="modele" value={id} checked={modele === id} onChange={() => setModele(id)} />
            <span>
              <strong>{nom}</strong>
              <span className="texte-doux petit">{aide}</span>
            </span>
          </label>
        ))}
      </fieldset>
    </Fenetre>
  );
}

function DialogueDossier({ parent, onFermer, onCreer }: { parent: string; onFermer: () => void; onCreer: (chemin: string) => void }) {
  const [nom, setNom] = useState("");
  const creer = () => nom.trim() && onCreer(parent ? `${parent}/${nom}` : nom);
  return (
    <Fenetre
      titre={parent ? `Nouveau dossier dans « ${parent.split("/").pop()} »` : "Nouveau dossier"}
      onFermer={onFermer}
      actions={
        <>
          <button className="bouton" onClick={onFermer}>
            Annuler
          </button>
          <button className="bouton bouton-principal" disabled={!nom.trim()} onClick={creer}>
            <FolderPlus size={15} aria-hidden /> Créer
          </button>
        </>
      }
    >
      <label htmlFor="nom-dossier-arbre">Nom</label>
      <input
        id="nom-dossier-arbre"
        className="champ"
        value={nom}
        placeholder={parent ? "Par exemple : Chapitre 1 - La cellule" : "Par exemple : SVT"}
        onChange={(e) => setNom(e.target.value)}
        onKeyDown={(e) => e.key === "Enter" && creer()}
      />
      <p className="texte-doux petit">
        {parent
          ? "Un dossier dans une matière est un chapitre."
          : "Un dossier à la racine est une matière ; crée ensuite un dossier par chapitre à l'intérieur."}
      </p>
    </Fenetre>
  );
}

function DialogueDeplacer({
  chemin,
  dossiers,
  onFermer,
  onDeplacer,
}: {
  chemin: string;
  dossiers: string[];
  onFermer: () => void;
  onDeplacer: (destination: string) => void;
}) {
  const actuel = dossierDe(chemin);
  const [destination, setDestination] = useState(actuel);
  return (
    <Fenetre
      titre={`Déplacer « ${chemin.split("/").pop()?.replace(/\.md$/i, "")} »`}
      onFermer={onFermer}
      actions={
        <>
          <button className="bouton" onClick={onFermer}>
            Annuler
          </button>
          <button className="bouton bouton-principal" disabled={destination === actuel} onClick={() => onDeplacer(destination)}>
            <FolderInput size={15} aria-hidden /> Déplacer
          </button>
        </>
      }
    >
      <label htmlFor="dossier">Vers le dossier</label>
      <ChoixDossier dossiers={dossiers} valeur={destination} onChange={setDestination} />
      <p className="texte-doux petit">La progression de révision et les liens vers cette note sont conservés.</p>
    </Fenetre>
  );
}
