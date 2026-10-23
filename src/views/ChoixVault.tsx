import { useState } from "react";
import { FolderClosed, FolderOpen } from "lucide-react";
import { open } from "@tauri-apps/plugin-dialog";
import { api, messageErreur, Reglages, VaultDetecte } from "../lib/api";

interface Props {
  cheminActuel: string | null;
  /** Vaults qu'Obsidian connaît sur ce PC, du plus récemment ouvert au plus ancien. */
  suggestions: VaultDetecte[];
  onChoisi: (r: Reglages) => void;
}

/** Sélection du dossier du vault (guide de démarrage et réglages). */
export default function ChoixVault({ cheminActuel, suggestions, onChoisi }: Props) {
  const [erreur, setErreur] = useState<string | null>(null);
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
    const choix = await open({ directory: true, title: "Choisir le dossier du vault Obsidian" });
    if (typeof choix === "string") await valider(choix);
  }

  return (
    <div className="choix-vault">
      {cheminActuel && (
        <p>
          Vault actuel : <code>{cheminActuel}</code>
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
          <FolderOpen size={15} aria-hidden /> {autres.length > 0 || cheminActuel ? "Choisir un autre dossier…" : "Choisir le dossier…"}
        </button>
      </div>
      {erreur && <p className="message-erreur">{erreur}</p>}
      <p className="texte-doux petit">
        Révisio lit ce dossier sans jamais rien y écrire. Ses propres données sont rangées ailleurs.
      </p>
    </div>
  );
}
