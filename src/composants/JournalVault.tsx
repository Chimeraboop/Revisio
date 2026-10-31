// Journal des écritures de Révisio dans le vault, avec annulation.
import { useEffect, useState } from "react";
import { FilePen, FilePlus, FolderInput, FolderPlus, type LucideIcon, Trash2, Undo2 } from "lucide-react";
import { api, EntreeHistorique, messageErreur } from "../lib/api";

interface Props {
  /** Seulement les écritures de ce lot de photos. */
  lot?: number;
  /** Change quand il faut relire le journal (nouvelle écriture). */
  version?: unknown;
  /** Nombre maximum de lignes affichées. */
  limite?: number;
  onOuvrir?: (chemin: string) => void;
}

const ICONES: Record<EntreeHistorique["genre"], LucideIcon> = {
  creation: FilePlus,
  modification: FilePen,
  dossier: FolderPlus,
  deplacement: FolderInput,
  suppression: Trash2,
  suppression_dossier: Trash2,
};

const ACTIONS: Record<EntreeHistorique["genre"], string> = {
  creation: "Créée",
  modification: "Modifiée",
  dossier: "Dossier créé",
  deplacement: "Déplacée ou renommée",
  suppression: "Mise à la corbeille",
  suppression_dossier: "Dossier mis à la corbeille",
};

const DATE = new Intl.DateTimeFormat("fr-FR", { day: "numeric", month: "short", hour: "2-digit", minute: "2-digit" });

export default function JournalVault({ lot, version, limite = 30, onOuvrir }: Props) {
  const [entrees, setEntrees] = useState<EntreeHistorique[] | null>(null);
  const [message, setMessage] = useState<{ texte: string; erreur: boolean } | null>(null);

  const relire = () =>
    api
      .historiqueVault()
      .then(setEntrees)
      .catch((e) => setMessage({ texte: messageErreur(e), erreur: true }));

  useEffect(() => {
    relire();
  }, [version]);

  async function annuler(id: number) {
    setMessage(null);
    try {
      setMessage({ texte: await api.annulerEcriture(id), erreur: false });
    } catch (e) {
      setMessage({ texte: messageErreur(e), erreur: true });
    }
    relire();
  }

  const visibles = (entrees ?? []).filter((e) => lot === undefined || e.lotId === lot).slice(0, limite);
  if (entrees && visibles.length === 0)
    return lot === undefined ? <p className="texte-doux petit">Révisio n'a encore rien écrit dans ton dossier de notes.</p> : null;

  return (
    <div className="journal-vault">
      <ul>
        {visibles.map((e) => {
          const Icone = ICONES[e.genre] ?? FilePen;
          const nom = e.chemin.split("/").pop()?.replace(/\.md$/i, "") ?? e.chemin;
          return (
            <li key={e.id} className={e.annuleeLe ? "annulee" : undefined}>
              <Icone size={16} className="icone" aria-hidden />
              <span className="journal-texte">
                {onOuvrir && !e.annuleeLe && (e.genre === "creation" || e.genre === "modification") ? (
                  <a href="#" onClick={(ev) => (ev.preventDefault(), onOuvrir(e.chemin))}>
                    {nom}
                  </a>
                ) : (
                  <strong>{nom}</strong>
                )}
                <span className="texte-doux petit">
                  {ACTIONS[e.genre] ?? "Modifiée"}
                  {e.cheminNouveau ? ` vers ${e.cheminNouveau}` : ""}
                  {e.origine === "photos" ? " à partir de photos" : e.origine === "discussion" ? " depuis une discussion avec l'IA" : " à la main"} le {DATE.format(e.date)}
                  {e.raison ? ` : ${e.raison}` : ""}
                  {e.annuleeLe ? ", annulée" : ""}
                </span>
              </span>
              {!e.annuleeLe && (
                <button className="bouton bouton-petit" onClick={() => annuler(e.id)}>
                  <Undo2 size={14} aria-hidden /> Annuler
                </button>
              )}
            </li>
          );
        })}
      </ul>
      {message && <p className={message.erreur ? "message-erreur" : "petit"}>{message.texte}</p>}
    </div>
  );
}
