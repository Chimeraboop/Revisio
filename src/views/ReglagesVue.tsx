import { api, InfosApp, messageErreur, ModeReponse, Reglages, ReglagesRevision } from "../lib/api";
import { Check, Database, FolderOpen, Layers, Palette, RotateCcw, Settings } from "lucide-react";
import EntetePage from "../composants/EntetePage";
import { ACCENTS, type Accent, type Theme } from "../lib/theme";
import CartesApprises from "./CartesApprises";
import ReglagesIaVue from "./ReglagesIaVue";
import ReglagesLocauxVue from "./ReglagesLocauxVue";
import ReglagesEcritureVue from "./ReglagesEcritureVue";
import ChoixVault from "./ChoixVault";
import { useState } from "react";
import { NOM_SYSTEME } from "../lib/plateforme";

interface Props {
  reglages: Reglages;
  infos: InfosApp;
  onChange: (r: Reglages) => void;
}

export default function ReglagesVue({ reglages, infos, onChange }: Props) {
  const [erreur, setErreur] = useState<string | null>(null);

  async function changerRevision(modif: Partial<ReglagesRevision>) {
    try {
      onChange(
        await api.definirReglagesRevision({
          nouvellesParJour: reglages.nouvellesParJour,
          entrelacement: reglages.entrelacement,
          vocabVersFrancais: reglages.vocabVersFrancais,
          vocabDepuisFrancais: reglages.vocabDepuisFrancais,
          dureeSeance: reglages.dureeSeance,
          nouvellesParJourLangues: reglages.nouvellesParJourLangues,
          dureeSeanceLangues: reglages.dureeSeanceLangues,
          ...modif,
        }),
      );
      setErreur(null);
    } catch (e) {
      setErreur(messageErreur(e));
    }
  }

  async function changerAccent(accent: Accent) {
    try {
      onChange(await api.definirAccent(accent));
    } catch (e) {
      setErreur(messageErreur(e));
    }
  }

  async function changerTheme(theme: Theme) {
    try {
      onChange(await api.definirTheme(theme));
    } catch (e) {
      setErreur(messageErreur(e));
    }
  }

  return (
    <div className="page">
      <EntetePage
        icone={Settings}
        titre="Réglages"
        description="Ton dossier de notes, l'apparence, les séances de révision et l'intelligence artificielle."
      />

      <section className="carte bloc">
        <h2 className="titre-bloc">
          <FolderOpen size={18} className="icone" aria-hidden /> Dossier de notes
        </h2>
        <ChoixVault cheminActuel={reglages.cheminVault} suggestions={infos.vaultsDetectes} onChoisi={onChange} />
      </section>

      <ReglagesEcritureVue reglages={reglages} onChange={onChange} />

      <section className="carte bloc">
        <h2 className="titre-bloc">
          <Palette size={18} className="icone" aria-hidden /> Apparence
        </h2>
        <label className="rangee">
          Thème
          <select value={reglages.theme} onChange={(e) => changerTheme(e.target.value as Theme)}>
            <option value="systeme">Comme {NOM_SYSTEME}</option>
            <option value="clair">Clair</option>
            <option value="sombre">Sombre</option>
          </select>
        </label>
        <div className="rangee bloc-haut">
          <span>Couleur d'accent</span>
          <div className="nuancier" role="radiogroup" aria-label="Couleur d'accent">
            {(Object.keys(ACCENTS) as Accent[]).map((a) => (
              <button
                key={a}
                role="radio"
                aria-checked={reglages.accent === a}
                title={ACCENTS[a].nom}
                className={"pastille-couleur" + (reglages.accent === a ? " choisie" : "")}
                style={{ "--teinte": ACCENTS[a].clair, "--teinte-sombre": ACCENTS[a].sombre } as React.CSSProperties}
                onClick={() => changerAccent(a)}
              >
                {reglages.accent === a && <Check size={14} strokeWidth={3} aria-hidden />}
                <span className="sr-only">{ACCENTS[a].nom}</span>
              </button>
            ))}
          </div>
          <span className="texte-doux petit">{ACCENTS[reglages.accent]?.nom}</span>
        </div>
        {erreur && <p className="message-erreur">{erreur}</p>}
      </section>

      <section className="carte bloc">
        <h2 className="titre-bloc">
          <Layers size={18} className="icone" aria-hidden /> Révision
        </h2>
        <div className="grille-reglages">
          <span className="sous-titre">Séance « Langues » (tout le vocabulaire)</span>
          <span />
          <label htmlFor="nouvelles-langues">Nouvelles cartes par jour</label>
          <input
            id="nouvelles-langues"
            className="champ champ-court"
            type="number"
            min={0}
            max={200}
            value={reglages.nouvellesParJourLangues}
            onChange={(e) => changerRevision({ nouvellesParJourLangues: Math.max(0, Number(e.target.value) || 0) })}
          />
          <label htmlFor="duree-langues">Durée de la séance du jour (minutes)</label>
          <input
            id="duree-langues"
            className="champ champ-court"
            type="number"
            min={5}
            max={180}
            step={5}
            value={reglages.dureeSeanceLangues}
            onChange={(e) => changerRevision({ dureeSeanceLangues: Number(e.target.value) || 10 })}
          />
          <span className="sous-titre">Séance « Autres matières »</span>
          <span />
          <label htmlFor="nouvelles">Nouvelles cartes par jour</label>
          <input
            id="nouvelles"
            className="champ champ-court"
            type="number"
            min={0}
            max={200}
            value={reglages.nouvellesParJour}
            onChange={(e) => changerRevision({ nouvellesParJour: Math.max(0, Number(e.target.value) || 0) })}
          />
          <label htmlFor="duree">Durée de la séance du jour (minutes)</label>
          <input
            id="duree"
            className="champ champ-court"
            type="number"
            min={5}
            max={180}
            step={5}
            value={reglages.dureeSeance}
            onChange={(e) => changerRevision({ dureeSeance: Number(e.target.value) || 20 })}
          />
          <span className="sous-titre">Pour les deux séances</span>
          <span />
          <label htmlFor="entrelacement">Mélanger matières et chapitres (entrelacement)</label>
          <input
            id="entrelacement"
            type="checkbox"
            checked={reglages.entrelacement}
            onChange={(e) => changerRevision({ entrelacement: e.target.checked })}
          />
          <label htmlFor="vers">Vocabulaire langue étrangère → français</label>
          <select
            id="vers"
            value={reglages.vocabVersFrancais}
            onChange={(e) => changerRevision({ vocabVersFrancais: e.target.value as ModeReponse })}
          >
            <option value="retourner">Retourner la carte et m'auto-noter</option>
            <option value="taper">Taper la réponse</option>
          </select>
          <label htmlFor="depuis">Vocabulaire français → langue étrangère</label>
          <select
            id="depuis"
            value={reglages.vocabDepuisFrancais}
            onChange={(e) => changerRevision({ vocabDepuisFrancais: e.target.value as ModeReponse })}
          >
            <option value="retourner">Retourner la carte et m'auto-noter</option>
            <option value="taper">Taper la réponse</option>
          </select>
        </div>
      </section>

      <ReglagesIaVue reglages={reglages} onChange={onChange} />

      <ReglagesLocauxVue reglages={reglages} onChange={onChange} />

      <CartesApprises />

      <section className="carte bloc">
        <h2 className="titre-bloc">
          <Database size={18} className="icone" aria-hidden /> Données de l'application
        </h2>
        <p>
          Base, cache et réglages : <code>{infos.dossierDonnees}</code>
        </p>
        <p className="texte-doux petit">Version {infos.version}</p>
        <div className="rangee bloc-haut">
          <button
            className="bouton"
            onClick={() =>
              api
                .definirAccueilTermine(false)
                .then(onChange)
                .catch((e) => setErreur(messageErreur(e)))
            }
          >
            <RotateCcw size={15} aria-hidden /> Revoir le guide de démarrage
          </button>
        </div>
      </section>
    </div>
  );
}
