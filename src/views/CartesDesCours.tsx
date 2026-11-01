// Cartes des cours créées par l'IA en arrière-plan (cœur Rust, `cartes_ia.rs`) : avancement, pause, échecs.
import { useEffect, useState } from "react";
import { listen } from "@tauri-apps/api/event";
import { BookCheck, RotateCcw } from "lucide-react";
import { api, BilanCartesIa, messageErreur, Reglages } from "../lib/api";

interface Props {
  onReglagesModifies: (r: Reglages) => void;
}

export default function CartesDesCours({ onReglagesModifies }: Props) {
  const [bilan, setBilan] = useState<BilanCartesIa | null>(null);
  const [erreur, setErreur] = useState<string | null>(null);

  const charger = () => {
    api.bilanCartesIa().then(setBilan).catch((e) => setErreur(messageErreur(e)));
  };

  useEffect(() => {
    charger();
    // Chaque étape du travail en arrière-plan prévient l'interface.
    const arret = listen("cartes-ia", charger);
    return () => {
      arret.then((f) => f());
    };
  }, []);

  async function basculer() {
    if (!bilan) return;
    try {
      onReglagesModifies(await api.definirCartesIaAuto(!bilan.actif));
      charger();
    } catch (e) {
      setErreur(messageErreur(e));
    }
  }

  async function reessayer() {
    try {
      await api.reessayerCartesIa();
      charger();
    } catch (e) {
      setErreur(messageErreur(e));
    }
  }

  if (!bilan) return null;
  const { etat } = bilan;
  const pourcentage = bilan.notes ? Math.round((100 * bilan.faites) / bilan.notes) : 100;

  return (
    <section className="carte bloc apparait cartes-des-cours" style={{ "--i": 1 } as React.CSSProperties}>
      <div className="rangee">
        <h2 className="titre-bloc">
          <BookCheck size={18} className="icone" aria-hidden /> Cartes de tes cours
        </h2>
        <span className="espace" />
        <label className="case">
          <input type="checkbox" checked={bilan.actif} onChange={basculer} />
          Créer automatiquement
        </label>
      </div>
      <p className="texte-doux petit">
        Quand tu ajoutes un cours (ou qu'une note s'allonge nettement), l'IA en tire des cartes question / réponse et
        quelques QCM, chacune justifiée par une phrase de ta note. Les textes à trous et listes à remettre dans l'ordre de
        cette note quittent alors tes révisions (ton historique est gardé). Le vocabulaire et les schémas ne changent pas.
      </p>
      <div className="progression bloc-haut" role="progressbar" aria-valuenow={pourcentage} aria-valuemin={0} aria-valuemax={100}>
        <div className="progression-barre" style={{ width: `${pourcentage}%` }} />
      </div>
      <p className="petit">
        {bilan.faites} note{bilan.faites > 1 ? "s" : ""} sur {bilan.notes} · {bilan.cartes} carte{bilan.cartes > 1 ? "s" : ""}{" "}
        créée{bilan.cartes > 1 ? "s" : ""}
        {bilan.aFaire > 0 && ` · ${bilan.aFaire} en attente`}
      </p>
      {bilan.actif && etat.enCours && (
        <p className="petit progression-texte">
          En cours : {etat.enCours}
          {etat.cartesCreees > 0 && ` · ${etat.cartesCreees} carte${etat.cartesCreees > 1 ? "s" : ""} depuis le début`}
          {" "}(les chapitres de tes prochaines évaluations d'abord)
        </p>
      )}
      {bilan.actif && etat.attente && <p className="petit texte-doux">En attente : {etat.attente} Révisio réessaiera tout seul.</p>}
      {!bilan.actif && bilan.aFaire > 0 && (
        <p className="petit texte-doux">En pause : coche « Créer automatiquement » pour reprendre.</p>
      )}
      {bilan.erreurs.length > 0 && (
        <details className="petit">
          <summary>
            {bilan.erreurs.length} note{bilan.erreurs.length > 1 ? "s" : ""} sans cartes à cause d'une erreur
          </summary>
          <ul className="liste-retour">
            {bilan.erreurs.map((e) => (
              <li key={e.chemin}>
                <strong>{e.titre}</strong> : {e.erreur}
              </li>
            ))}
          </ul>
          <button className="bouton bouton-petit" onClick={reessayer}>
            <RotateCcw size={14} aria-hidden /> Réessayer
          </button>
        </details>
      )}
      {erreur && <p className="message-erreur">{erreur}</p>}
    </section>
  );
}
