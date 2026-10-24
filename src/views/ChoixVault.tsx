import { useState } from "react";
import { FolderClosed, FolderOpen, FolderPlus } from "lucide-react";
import { open } from "@tauri-apps/plugin-dialog";
import { api, messageErreur, Reglages, VaultDetecte } from "../lib/api";

interface Props {
  cheminActuel: string | null;
  /** Vaults qu'Obsidian connaît sur ce PC, du plus récemment ouvert au plus ancien. */
  suggestions: VaultDetecte[];
  onChoisi: (r: Reglages) => void;
}

/** Choix du dossier de notes (guide de démarrage et réglages) : un vault Obsidian, n'importe quel
 * dossier de fichiers Markdown, ou un nouveau dossier créé par Révisio. */
export default function ChoixVault({ cheminActuel, suggestions, onChoisi }: Props) {
  const [erreur, setErreur] = useState<string | null>(null);
  const [creation, setCreation] = useState(false);
  const [nom, setNom] = useState("Mes cours");
  const [emplacement, setEmplacement] = useState<string | null>(null);
  const autres = suggestions.filter((v) => v.chemin !== cheminActuel);

  async function valider(chemin: string) {
    setErreur(null);
    try {
      onChoisi(await api.definirVault(chemin));
    } catch (e) {
      setErreur(messageErreur(e));
    }
  }

  async function parcourir() {
    const choix = await open({ directory: true, title: "Choisir le dossier de tes notes" });
    if (typeof choix === "string") await valider(choix);
  }

  async function choisirEmplacement() {
    const choix = await open({ directory: true, title: "Où créer ton dossier de notes ?" });
    if (typeof choix === "string") setEmplacement(choix);
  }

  async function creer() {
    if (!emplacement) return;
    setErreur(null);
    try {
      onChoisi(await api.creerDossierNotes(emplacement, nom));
      setCreation(false);
    } catch (e) {
      setErreur(messageErreur(e));
    }
  }

  return (
    <div className="choix-vault">
      {cheminActuel && (
        <p>
          Dossier actuel : <code>{cheminActuel}</code>
        </p>
      )}
      {autres.length > 0 && (
        <div className="vaults-detectes">
          <span className="texte-doux petit">
            {cheminActuel ? "Autres vaults trouvés dans Obsidian :" : "Vaults trouvés dans Obsidian :"}
          </span>
          {autres.map((v) => (
            <button key={v.chemin} className="vault-detecte" onClick={() => valider(v.chemin)}>
              <FolderClosed size={18} className="icone" aria-hidden />
              <span>
                <strong>{v.nom}</strong>
                <code>{v.chemin}</code>
              </span>
            </button>
          ))}
        </div>
      )}
      <div className="rangee">
        <button className={"bouton" + (autres.length === 0 && !cheminActuel ? " bouton-principal" : "")} onClick={parcourir}>
          <FolderOpen size={15} aria-hidden /> {autres.length > 0 || cheminActuel ? "Choisir un autre dossier…" : "Choisir un dossier de notes…"}
        </button>
        {!creation && (
          <button className="bouton" onClick={() => setCreation(true)}>
            <FolderPlus size={15} aria-hidden /> Créer un nouveau dossier de notes
          </button>
        )}
      </div>

      {creation && (
        <div className="creation-dossier">
          <label htmlFor="nom-dossier">Nom du dossier</label>
          <input id="nom-dossier" className="champ" value={nom} onChange={(e) => setNom(e.target.value)} />
          <span className="rangee">
            <button className="bouton" onClick={choisirEmplacement}>
              <FolderOpen size={15} aria-hidden /> {emplacement ? "Changer l'emplacement…" : "Choisir l'emplacement…"}
            </button>
            {emplacement && <code className="petit">{emplacement}</code>}
          </span>
          <span className="rangee">
            <button className="bouton bouton-principal" disabled={!emplacement || !nom.trim()} onClick={creer}>
              <FolderPlus size={15} aria-hidden /> Créer
            </button>
            <button className="bouton" onClick={() => setCreation(false)}>
              Annuler
            </button>
          </span>
          <p className="texte-doux petit">
            Révisio crée le dossier avec une note d'accueil, et pourra y écrire tes notes. Ce sont de simples fichiers
            Markdown, lisibles partout (et par Obsidian si tu l'utilises un jour).
          </p>
        </div>
      )}

      {erreur && <p className="message-erreur">{erreur}</p>}
      <p className="texte-doux petit">
        N'importe quel dossier de fichiers Markdown (.md) convient, avec ou sans Obsidian. Pour un dossier existant,
        Révisio ne modifie rien tant que tu ne l'as pas autorisé. Ses propres données sont rangées ailleurs.
      </p>
    </div>
  );
}
