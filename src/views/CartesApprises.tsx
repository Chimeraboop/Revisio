// Réglage « mes cartes actuelles sont déjà apprises » : utile quand on commence à utiliser
// Révisio en cours d'année, sur des notes déjà sues.
import { useEffect, useState } from "react";
import { BadgeCheck } from "lucide-react";
import { api, messageErreur } from "../lib/api";

export default function CartesApprises() {
  const [jamaisVues, setJamaisVues] = useState<number | null>(null);
  const [confirmation, setConfirmation] = useState(false);
  const [resultat, setResultat] = useState<string | null>(null);

  const compter = () =>
    api
      .resumeRevision()
      .then((r) => setJamaisVues(r.groupes.reduce((n, g) => n + g.nouvellesDisponibles, 0)))
      .catch(() => setJamaisVues(null));

  useEffect(() => {
    compter();
  }, []);

  async function marquer() {
    try {
      const n = await api.marquerCartesApprises();
      setResultat(`${n} carte${n > 1 ? "s" : ""} marquée${n > 1 ? "s" : ""} comme apprise${n > 1 ? "s" : ""}.`);
      setConfirmation(false);
      compter();
    } catch (e) {
      setResultat(messageErreur(e));
    }
  }

  return (
    <section className="carte bloc">
      <h2 className="titre-bloc">
        <BadgeCheck size={18} className="icone" aria-hidden /> Cartes déjà apprises
      </h2>
      <p className="texte-doux petit">
        Les cartes jamais révisées sont considérées comme sues : elles ne sont plus « nouvelles » et une première
        vérification est répartie sur les 30 prochains jours. Les notes que tu ajouteras ensuite resteront nouvelles.
      </p>
      <div className="rangee">
        {!confirmation ? (
          <button className="bouton" disabled={!jamaisVues} onClick={() => setConfirmation(true)}>
            Considérer les {jamaisVues ?? "…"} cartes jamais vues comme apprises
          </button>
        ) : (
          <>
            <span>Confirmer pour {jamaisVues} cartes ?</span>
            <button className="bouton bouton-principal" onClick={marquer}>
              Oui, elles sont apprises
            </button>
            <button className="bouton" onClick={() => setConfirmation(false)}>
              Annuler
            </button>
          </>
        )}
      </div>
      {resultat && <p className="petit">{resultat}</p>}
    </section>
  );
}
