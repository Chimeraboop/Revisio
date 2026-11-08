// Créer des notes depuis le téléphone : écrire une note à la main, ou envoyer des photos / PDF de cours
// que le PC transforme en notes avec l'IA (la clé de l'IA reste sur le PC).
import { useEffect, useMemo, useRef, useState } from "react";
import { ArrowLeft, Camera, FilePlus2, Loader2, Send } from "lucide-react";
import { dossiersDeNotes } from "./donnees";
import { FICHIERS_MAX, etatDuDepot } from "./depot";
import { creerNoteMobile, depotsLocaux, deposerPhotos, reglages, useMagasin, type DepotLocal } from "./magasin";

const NOUVEAU_DOSSIER = "\u0000nouveau";
const DATE = new Intl.DateTimeFormat("fr-FR", { day: "numeric", month: "long", hour: "2-digit", minute: "2-digit" });

function Retour({ onClick }: { onClick: () => void }) {
  return (
    <button className="lien-discret retour-mobile" onClick={onClick}>
      <ArrowLeft size={16} aria-hidden /> Toutes les notes
    </button>
  );
}

/** Écrire une note : dossier (existant ou nouveau), titre, texte. */
export function NouvelleNote({ onRetour, onCreee }: { onRetour: () => void; onCreee: (chemin: string) => void }) {
  const { modele } = useMagasin();
  const dossiers = useMemo(() => (modele ? dossiersDeNotes(modele) : []), [modele]);
  const [dossier, setDossier] = useState(dossiers[0] ?? NOUVEAU_DOSSIER);
  const [nouveau, setNouveau] = useState("");
  const [titre, setTitre] = useState("");
  const [corps, setCorps] = useState("");
  const [erreur, setErreur] = useState<string | null>(null);
  const ecritureAutorisee = reglages().ecritureAutorisee;

  const creer = async () => {
    setErreur(null);
    try {
      onCreee(await creerNoteMobile(dossier === NOUVEAU_DOSSIER ? nouveau : dossier, titre, corps));
    } catch (e) {
      setErreur(e instanceof Error ? e.message : String(e));
    }
  };

  return (
    <div className="notes-mobile creer-mobile">
      <Retour onClick={onRetour} />
      <h2 className="titre-bloc">Nouvelle note</h2>
      {!ecritureAutorisee && (
        <p className="message-erreur">
          L'écriture dans ton dossier de notes est désactivée sur le PC : la note restera ici tant que tu ne l'as pas autorisée
          (Réglages de Révisio sur le PC).
        </p>
      )}
      <label className="champ-mobile">
        Dossier
        <select value={dossier} onChange={(e) => setDossier(e.target.value)}>
          {dossiers.map((d) => (
            <option key={d} value={d}>
              {d}
            </option>
          ))}
          <option value={NOUVEAU_DOSSIER}>Nouveau dossier…</option>
        </select>
      </label>
      {dossier === NOUVEAU_DOSSIER && (
        <label className="champ-mobile">
          Nom du nouveau dossier (ex. : Histoire-Géo/Chapitre 3)
          <input value={nouveau} onChange={(e) => setNouveau(e.target.value)} autoCapitalize="sentences" />
        </label>
      )}
      <label className="champ-mobile">
        Titre
        <input value={titre} onChange={(e) => setTitre(e.target.value)} autoCapitalize="sentences" />
      </label>
      <label className="champ-mobile">
        Texte (Markdown : **gras**, listes, [[lien vers une note]])
        <textarea rows={12} value={corps} onChange={(e) => setCorps(e.target.value)} />
      </label>
      {erreur && <p className="message-erreur">{erreur}</p>}
      <button className="bouton bouton-principal" disabled={!titre.trim() || !corps.trim()} onClick={() => void creer()}>
        <FilePlus2 size={16} aria-hidden /> Créer la note
      </button>
      <p className="texte-doux petit">Elle apparaît ici tout de suite et rejoint ton dossier de notes sur le PC à la prochaine synchronisation.</p>
    </div>
  );
}

/** Envoyer des photos ou PDF de cours au PC, et suivre ce qu'il en fait. */
export function DepotPhotos({ onRetour }: { onRetour: () => void }) {
  const { modele, connecte } = useMagasin();
  const entree = useRef<HTMLInputElement>(null);
  const [choisis, setChoisis] = useState<File[]>([]);
  const [avancement, setAvancement] = useState<{ fait: number; total: number } | null>(null);
  const [erreur, setErreur] = useState<string | null>(null);
  const [depots, setDepots] = useState<DepotLocal[]>([]);
  const pc = modele?.contenu.depots ?? [];

  useEffect(() => {
    void depotsLocaux().then(setDepots);
  }, []);

  const choisir = (liste: FileList | null) => {
    setErreur(null);
    const fichiers = [...(liste ?? [])];
    if (fichiers.length > FICHIERS_MAX) setErreur(`${FICHIERS_MAX} fichiers au plus à la fois : les premiers sont gardés.`);
    setChoisis(fichiers.slice(0, FICHIERS_MAX));
  };

  const envoyer = async () => {
    setErreur(null);
    try {
      await deposerPhotos(choisis, (fait, total) => setAvancement({ fait, total }));
      setChoisis([]);
      if (entree.current) entree.current.value = "";
      setDepots(await depotsLocaux());
    } catch (e) {
      setErreur(e instanceof Error ? e.message : String(e));
    } finally {
      setAvancement(null);
    }
  };

  return (
    <div className="notes-mobile creer-mobile">
      <Retour onClick={onRetour} />
      <h2 className="titre-bloc">Photos de cours</h2>
      <p className="texte-doux petit">
        Prends en photo ton cours (ou choisis un PDF) : ton PC en fait des notes avec l'IA, rangées dans le bon chapitre, puis te les renvoie.
        Il faut que Révisio soit ouvert sur le PC. Les photos d'un même cours vont ensemble ({FICHIERS_MAX} au plus).
      </p>
      {!connecte && <p className="message-erreur">Connecte d'abord ton Google Drive (onglet Synchro).</p>}
      <input ref={entree} className="cache-mobile" type="file" accept="image/*,application/pdf" multiple onChange={(e) => choisir(e.target.files)} />
      <button className="bouton" disabled={!!avancement} onClick={() => entree.current?.click()}>
        <Camera size={16} aria-hidden /> Prendre ou choisir des photos
      </button>
      {choisis.length > 0 && (
        <>
          <ul className="fichiers-mobile">
            {choisis.map((f, i) => (
              <li key={`${f.name}-${i}`}>{f.name}</li>
            ))}
          </ul>
          <button className="bouton bouton-principal" disabled={!!avancement || !connecte} onClick={() => void envoyer()}>
            {avancement ? <Loader2 size={16} className="tourne-sans-fin" aria-hidden /> : <Send size={16} aria-hidden />}{" "}
            {avancement ? `Envoi ${avancement.fait} / ${avancement.total}…` : `Envoyer ${choisis.length} fichier${choisis.length > 1 ? "s" : ""} au PC`}
          </button>
        </>
      )}
      {erreur && <p className="message-erreur">{erreur}</p>}
      {depots.length > 0 && (
        <>
          <h3 className="titre-bloc">Mes envois</h3>
          <ul className="depots-mobile">
            {depots.map((d) => {
              const etat = etatDuDepot(pc.find((p) => p.id === d.id));
              return (
                <li key={d.id} className={etat.probleme ? "probleme" : ""}>
                  <strong>
                    {DATE.format(d.date)} · {d.n} fichier{d.n > 1 ? "s" : ""}
                  </strong>
                  <span className="texte-doux petit">{etat.texte}</span>
                </li>
              );
            })}
          </ul>
        </>
      )}
    </div>
  );
}
