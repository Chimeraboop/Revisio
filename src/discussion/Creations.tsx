// Onglet « Créations » : les schémas et graphiques gardés, des plus récents aux plus anciens, avec un lien
// vers la conversation d'origine.
import { useEffect, useState } from "react";
import { ChartLine, MessagesSquare, Network, Pencil, Trash } from "lucide-react";
import { api, messageErreur, type CreationIa } from "../lib/api";
import { Visuel } from "./Messages";

const formatDate = new Intl.DateTimeFormat("fr-FR", { day: "numeric", month: "long", year: "numeric" });

interface Props {
  ouvrirConversation: (id: number) => void;
  ouvrirNote: (chemin: string) => void;
}

export default function Creations({ ouvrirConversation }: Props) {
  const [creations, setCreations] = useState<CreationIa[] | null>(null);
  const [filtre, setFiltre] = useState<"tout" | "diagramme" | "graphique">("tout");
  const [renommage, setRenommage] = useState<{ id: number; titre: string } | null>(null);
  const [aRetirer, setARetirer] = useState<number | null>(null);
  const [erreur, setErreur] = useState<string | null>(null);

  const charger = () =>
    api
      .creationsIa()
      .then(setCreations)
      .catch((e) => setErreur(messageErreur(e)));
  useEffect(() => {
    charger();
  }, []);

  async function renommer() {
    if (!renommage) return;
    try {
      await api.renommerCreation(renommage.id, renommage.titre);
      setRenommage(null);
      charger();
    } catch (e) {
      setErreur(messageErreur(e));
    }
  }

  async function retirer(id: number) {
    try {
      await api.retirerCreation(id);
      setARetirer(null);
      charger();
    } catch (e) {
      setErreur(messageErreur(e));
    }
  }

  const visibles = (creations ?? []).filter((c) => filtre === "tout" || c.genre === filtre);

  return (
    <section className="discussion-creations" aria-label="Créations">
      <div className="rangee">
        <div className="segmente" role="radiogroup" aria-label="Type de création">
          {(
            [
              ["tout", "Tout"],
              ["diagramme", "Schémas"],
              ["graphique", "Graphiques"],
            ] as const
          ).map(([v, nom]) => (
            <button key={v} role="radio" aria-checked={filtre === v} className={"segment" + (filtre === v ? " actif" : "")} onClick={() => setFiltre(v)}>
              {nom}
            </button>
          ))}
        </div>
        <span className="texte-doux petit">
          {creations ? `${visibles.length} création${visibles.length > 1 ? "s" : ""}` : "Chargement…"}
        </span>
      </div>
      {erreur && <p className="message-erreur">{erreur}</p>}
      {creations && creations.length === 0 && (
        <div className="carte etat-vide">
          <Network size={30} strokeWidth={1.6} aria-hidden />
          <p>
            Pas encore de création. Quand l'IA te propose un schéma ou un graphique dans une conversation, clique sur
            « Garder » : il arrivera ici.
          </p>
        </div>
      )}
      <div className="grille-creations">
        {visibles.map((c) => {
          const Icone = c.genre === "diagramme" ? Network : ChartLine;
          return (
            <article key={c.id} className="carte creation">
              <header className="rangee entre">
                {renommage?.id === c.id ? (
                  <form
                    className="rangee"
                    onSubmit={(e) => {
                      e.preventDefault();
                      renommer();
                    }}
                  >
                    <input
                      className="champ"
                      autoFocus
                      value={renommage.titre}
                      onChange={(e) => setRenommage({ id: c.id, titre: e.target.value })}
                    />
                    <button className="bouton bouton-petit" type="submit">
                      OK
                    </button>
                  </form>
                ) : (
                  <h3 className="titre-creation">
                    <Icone size={16} className="icone" aria-hidden /> {c.titre}
                  </h3>
                )}
                <button className="bouton-icone" aria-label="Renommer" onClick={() => setRenommage({ id: c.id, titre: c.titre })}>
                  <Pencil size={14} aria-hidden />
                </button>
              </header>
              <div className="creation-visuel">
                <Visuel genre={c.genre} contenu={c.contenu} />
              </div>
              <footer className="rangee petit texte-doux">
                <span>{formatDate.format(new Date(c.date))}</span>
                {c.conversationId !== null && (
                  <button className="lien-discret" onClick={() => ouvrirConversation(c.conversationId!)}>
                    <MessagesSquare size={13} aria-hidden /> {c.conversationTitre ?? "Conversation"}
                  </button>
                )}
                <span className="espace" />
                {aRetirer === c.id ? (
                  <>
                    <button className="bouton bouton-danger bouton-petit" onClick={() => retirer(c.id)}>
                      Retirer
                    </button>
                    <button className="lien-discret" onClick={() => setARetirer(null)}>
                      Annuler
                    </button>
                  </>
                ) : (
                  <button className="lien-discret" onClick={() => setARetirer(c.id)}>
                    <Trash size={13} aria-hidden /> Retirer
                  </button>
                )}
              </footer>
            </article>
          );
        })}
      </div>
    </section>
  );
}
