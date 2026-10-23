// Journal des écritures de Révisio dans le vault, avec annulation.
import { useEffect, useState } from "react";
import { FilePen, FilePlus, Undo2 } from "lucide-react";
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
    return lot === undefined ? <p className="texte-doux petit">Révisio n'a encore rien écrit dans ton vault.</p> : null;

  return (
    <div className="journal-vault">
      <ul>
        {visibles.map((e) => {
          const Icone = e.genre === "creation" ? FilePlus : FilePen;
          const nom = e.chemin.split("/").pop()?.replace(/\.md$/i, "") ?? e.chemin;
          return (
            <li key={e.id} className={e.annuleeLe ? "annulee" : undefined}>
              <Icone size={16} className="icone" aria-hidden />
              <span className="journal-texte">
                {onOuvrir && !e.annuleeLe ? (
                  <a href="#" onClick={(ev) => (ev.preventDefault(), onOuvrir(e.chemin))}>
                    {nom}
                  </a>
                ) : (
                  <strong>{nom}</strong>
                )}
                <span className="texte-doux petit">
                  {e.genre === "creation" ? "Créée" : "Modifiée"}
                  {e.origine === "photos" ? " à partir de photos" : " à la main"} le {DATE.format(e.date)}
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
