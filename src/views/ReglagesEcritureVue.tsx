// Réglages : écriture dans le vault (désactivée par défaut) et journal des écritures.
import { useState } from "react";
import { FilePen } from "lucide-react";
import { api, messageErreur, Reglages } from "../lib/api";
import JournalVault from "../composants/JournalVault";

interface Props {
  reglages: Reglages;
  onChange: (r: Reglages) => void;
}

export default function ReglagesEcritureVue({ reglages, onChange }: Props) {
  const [erreur, setErreur] = useState<string | null>(null);

  async function basculer(autorisee: boolean) {
    setErreur(null);
    try {
      onChange(await api.definirEcritureAutorisee(autorisee));
    } catch (e) {
      setErreur(messageErreur(e));
    }
  }

  return (
    <section className="carte bloc">
      <h2 className="titre-bloc">
        <FilePen size={18} className="icone" aria-hidden /> Écriture dans le vault
      </h2>
      <p className="texte-doux petit">
        Par défaut, Révisio lit tes notes sans jamais les modifier. Si tu l'autorises, il peut créer des notes à partir
        de tes photos de cours et tu peux corriger une note avec le bouton Modifier. Chaque écriture est sauvegardée et
        peut être annulée ci-dessous ; une note existante n'est jamais changée par l'IA sans ton accord.
      </p>
      <label className="rangee case-a-cocher">
        <input type="checkbox" checked={reglages.ecritureAutorisee} onChange={(e) => basculer(e.target.checked)} />
        Autoriser Révisio à écrire dans mon vault
      </label>
      {erreur && <p className="message-erreur">{erreur}</p>}
      <h3 className="sous-titre-bloc">Dernières écritures</h3>
      <JournalVault limite={15} version={reglages.ecritureAutorisee} />
    </section>
  );
}
