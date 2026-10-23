// Rapport de progression : affiché ici, enregistré en Markdown (hors du vault) ou imprimé / enregistré en PDF.
import { open } from "@tauri-apps/plugin-dialog";
import { useEffect, useState } from "react";
import { ArrowLeft, FileDown, FolderOpen, Printer } from "lucide-react";
import { api, messageErreur } from "../lib/api";
import { MarkdownObsidian } from "../markdown/rendu";
import { nomFichier, rapportMarkdown } from "../rapport/rapport";
import { jourLocal } from "../revision/stats";

interface Props {
  onRetour: () => void;
}

export default function RapportVue({ onRetour }: Props) {
  const [markdown, setMarkdown] = useState<string | null>(null);
  const [message, setMessage] = useState<{ texte: string; chemin?: string; erreur?: boolean } | null>(null);

  useEffect(() => {
    const maintenant = new Date();
    Promise.all([api.tableauDeBord(), api.evaluations(jourLocal(maintenant)), api.dernieresNotes(), api.carnetErreurs(null, false)])
      .then(([tableau, evaluations, notes, erreurs]) =>
        setMarkdown(
          rapportMarkdown({
            maintenant,
            cartes: tableau.cartes,
            jours: tableau.jours,
            faibles: tableau.faibles,
            evaluations,
            notes,
            erreurs,
          }),
        ),
      )
      .catch((e) => setMessage({ texte: messageErreur(e), erreur: true }));
  }, []);

  async function enregistrer(choisirDossier: boolean) {
    if (!markdown) return;
    try {
      let dossier: string | null = null;
      if (choisirDossier) {
        const choix = await open({ directory: true, title: "Dossier où enregistrer le rapport (hors du vault)" });
        if (typeof choix !== "string") return;
        dossier = choix;
      }
      const chemin = await api.enregistrerExport(nomFichier(new Date()), markdown, dossier);
      setMessage({ texte: `Rapport enregistré : ${chemin}`, chemin });
    } catch (e) {
      setMessage({ texte: messageErreur(e), erreur: true });
    }
  }

  return (
    <div className="page rapport">
      <div className="rangee entre sans-impression">
        <button className="lien-discret" onClick={onRetour}>
          <ArrowLeft size={14} aria-hidden /> Tableau de bord
        </button>
        <div className="rangee">
          <button className="bouton bouton-principal" disabled={!markdown} onClick={() => enregistrer(false)}>
            <FileDown size={15} aria-hidden /> Enregistrer en Markdown
          </button>
          <button className="bouton" disabled={!markdown} onClick={() => enregistrer(true)}>
            <FolderOpen size={15} aria-hidden /> Choisir le dossier…
          </button>
          <button className="bouton" disabled={!markdown} onClick={() => window.print()}>
            <Printer size={15} aria-hidden /> Imprimer ou enregistrer en PDF
          </button>
        </div>
      </div>
      {message && (
        <p className={"sans-impression " + (message.erreur ? "message-erreur" : "texte-doux petit")}>
          {message.texte}{" "}
          {message.chemin && (
            <button className="lien-discret" onClick={() => api.montrerFichier(message.chemin!)}>
              Montrer dans l'Explorateur
            </button>
          )}
        </p>
      )}
      <p className="texte-doux petit sans-impression">
        Pour un PDF, choisis l'imprimante « Microsoft Print to PDF » ou « Enregistrer au format PDF ».
      </p>
      {markdown ? (
        <article className="carte bloc rapport-contenu">
          <MarkdownObsidian markdown={markdown} navigation={{ source: "", ouvrirNote: () => {} }} />
        </article>
      ) : (
        !message && <p className="texte-doux">Préparation du rapport…</p>
      )}
    </div>
  );
}
