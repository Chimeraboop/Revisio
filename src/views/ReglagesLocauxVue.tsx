// Réglages de l'IA locale (Ollama) : état d'Ollama, mode d'utilisation, modèle, téléchargement, essai.
// Ollama fait tourner un modèle d'IA sur ce PC : rien ne part sur Internet, mais c'est plus lent que Gemini.
import { listen } from "@tauri-apps/api/event";
import { useEffect, useState } from "react";
import { Cpu, Download, FlaskConical, Square, Trash } from "lucide-react";
import { api, AvancementTelechargement, EtatIaLocale, messageErreur, ModeIaLocale, Reglages } from "../lib/api";
import { modeleInstalle } from "../lib/moteurIa";

interface Props {
  reglages: Reglages;
  onChange: (r: Reglages) => void;
}

const MODES: Record<ModeIaLocale, [string, string]> = {
  jamais: ["Jamais", "Gemini seulement, comme avant."],
  secours: ["En secours", "Gemini d'abord ; l'IA locale prend le relais sans Internet, sans clé ou quand le quota est épuisé."],
  toujours: ["Toujours (tâches simples)", "Rien n'est envoyé à Google pour ces tâches, mais c'est plus lent."],
};

/** Modèles conseillés pour une carte graphique de 6 Go (bibliothèque Ollama, octobre 2026). */
const SUGGESTIONS: [string, string][] = [
  ["qwen3.5:4b", "3,3 Go · tient entièrement dans une carte graphique de 6 Go · le plus rapide"],
  ["ministral-3:8b", "6 Go · Mistral (français) · meilleur en français, plus lent"],
  ["ministral-3:3b", "3 Go · Mistral, léger"],
  ["qwen3.5:9b", "6,6 Go · plus précis, plus lent"],
];

const go = (octets: number) => `${(octets / 1e9).toLocaleString("fr-FR", { maximumFractionDigits: 1 })} Go`;

export default function ReglagesLocauxVue({ reglages, onChange }: Props) {
  const [etat, setEtat] = useState<EtatIaLocale | null>(null);
  const [aTelecharger, setATelecharger] = useState("qwen3.5:4b");
  const [avancement, setAvancement] = useState<AvancementTelechargement | null>(null);
  const [aSupprimer, setASupprimer] = useState<string | null>(null);
  const [message, setMessage] = useState<{ texte: string; erreur: boolean } | null>(null);
  const [enCours, setEnCours] = useState(false);

  const rafraichir = () =>
    api
      .etatIaLocale()
      .then(setEtat)
      .catch(() => setEtat(null));

  useEffect(() => {
    rafraichir();
    const arret = listen<AvancementTelechargement>("ia-locale-telechargement", (e) => setAvancement(e.payload));
    return () => {
      arret.then((f) => f());
    };
  }, []);

  async function action(f: () => Promise<string | void>) {
    setEnCours(true);
    setMessage(null);
    try {
      const texte = await f();
      if (texte) setMessage({ texte, erreur: false });
    } catch (e) {
      setMessage({ texte: messageErreur(e), erreur: true });
    } finally {
      setEnCours(false);
      rafraichir();
    }
  }

  const changer = (modif: Partial<Pick<Reglages, "iaLocale" | "modeleLocal" | "adresseOllama">>) =>
    action(async () => {
      const r = { iaLocale: reglages.iaLocale, modeleLocal: reglages.modeleLocal, adresseOllama: reglages.adresseOllama, ...modif };
      onChange(await api.definirReglagesLocaux(r.iaLocale, r.modeleLocal, r.adresseOllama));
    });

  async function telecharger() {
    const modele = aTelecharger.trim();
    setAvancement({ modele, statut: "démarrage", total: 0, fait: 0 });
    await action(async () => {
      try {
        await api.telechargerModeleLocal(modele);
      } finally {
        setAvancement(null);
      }
      // Premier modèle installé : on le choisit d'office.
      if (!reglages.modeleLocal) onChange(await api.definirReglagesLocaux(reglages.iaLocale, modele, reglages.adresseOllama));
      return `Modèle « ${modele} » installé.`;
    });
  }

  const actif = !!etat?.version;
  const modeles = etat?.modeles ?? [];
  const telechargementEnCours = avancement !== null || !!etat?.telechargement;
  const choisiAbsent = !!reglages.modeleLocal && actif && !modeleInstalle(reglages.modeleLocal, modeles.map((m) => m.nom));
  const pourcent = avancement && avancement.total > 0 ? Math.floor((avancement.fait * 100) / avancement.total) : null;

  return (
    <section className="carte bloc">
      <div className="rangee entre">
        <h2 className="titre-bloc">
          <Cpu size={18} className="icone" aria-hidden /> IA locale (Ollama)
        </h2>
        <span className={"indicateur" + (actif || etat === null ? "" : " hors-ligne")}>
          <span className="point" aria-hidden /> {etat === null ? "…" : actif ? `Ollama ${etat.version}` : "Ollama ne répond pas"}
        </span>
      </div>
      <p className="texte-doux petit">
        Ollama fait tourner une IA directement sur ce PC : elle marche sans Internet et rien n'est envoyé à Google, mais
        elle est plus lente et moins précise que Gemini. Révisio s'en sert pour les tâches simples : questions sur un
        chapitre, correction des réponses ouvertes, méthode Feynman et cartes mentales. Les exercices type bac, les
        questions sur les schémas, les podcasts et les vidéos restent sur Gemini.
      </p>

      {!actif && etat !== null && (
        <p className="petit">
          Ollama n'est pas lancé ou pas installé ({etat.erreur}).{" "}
          <button className="lien-discret" onClick={() => api.ouvrirLienExterne("https://ollama.com/download")}>
            Télécharger Ollama (site officiel)
          </button>{" "}
          ·{" "}
          <button className="lien-discret" onClick={rafraichir}>
            Revérifier
          </button>
        </p>
      )}

      <div className="grille-reglages">
        <label htmlFor="ia-locale-mode">Utiliser l'IA locale</label>
        <select
          id="ia-locale-mode"
          value={reglages.iaLocale}
          disabled={enCours}
          onChange={(e) => changer({ iaLocale: e.target.value as ModeIaLocale })}
        >
          {Object.entries(MODES).map(([v, [n]]) => (
            <option key={v} value={v}>
              {n}
            </option>
          ))}
        </select>
        <span />
        <span className="texte-doux petit">{MODES[reglages.iaLocale]?.[1]}</span>

        <label htmlFor="ia-locale-modele">Modèle</label>
        <span className="rangee">
          <select
            id="ia-locale-modele"
            value={reglages.modeleLocal}
            disabled={enCours || modeles.length === 0}
            onChange={(e) => changer({ modeleLocal: e.target.value })}
          >
            <option value="">{modeles.length === 0 ? "Aucun modèle installé" : "Choisis…"}</option>
            {choisiAbsent && <option value={reglages.modeleLocal}>{reglages.modeleLocal} (pas installé)</option>}
            {modeles.map((m) => (
              <option key={m.nom} value={m.nom}>
                {m.nom} — {go(m.taille)}
                {m.parametres ? ` · ${m.parametres} paramètres` : ""}
              </option>
            ))}
          </select>
          {reglages.modeleLocal && modeleInstalle(reglages.modeleLocal, modeles.map((m) => m.nom)) && (
            <>
              {aSupprimer === reglages.modeleLocal ? (
                <>
                  <button
                    className="bouton bouton-danger"
                    disabled={enCours}
                    onClick={() => {
                      const m = reglages.modeleLocal;
                      setASupprimer(null);
                      action(async () => {
                        await api.supprimerModeleLocal(m);
                        onChange(await api.lireReglages());
                        return `Modèle « ${m} » supprimé du PC.`;
                      });
                    }}
                  >
                    <Trash size={14} aria-hidden /> Supprimer du PC
                  </button>
                  <button className="lien-discret petit" onClick={() => setASupprimer(null)}>
                    Annuler
                  </button>
                </>
              ) : (
                <button className="lien-discret petit" onClick={() => setASupprimer(reglages.modeleLocal)}>
                  <Trash size={13} aria-hidden /> Supprimer…
                </button>
              )}
            </>
          )}
        </span>

        <label htmlFor="ia-locale-telecharger">Télécharger un modèle</label>
        <span className="rangee">
          <input
            id="ia-locale-telecharger"
            className="champ"
            list="ia-locale-suggestions"
            value={aTelecharger}
            disabled={telechargementEnCours}
            onChange={(e) => setATelecharger(e.target.value)}
          />
          <datalist id="ia-locale-suggestions">
            {SUGGESTIONS.map(([m, d]) => (
              <option key={m} value={m}>
                {d}
              </option>
            ))}
          </datalist>
          {telechargementEnCours ? (
            <button className="bouton" onClick={() => api.annulerTelechargementLocal()}>
              <Square size={13} aria-hidden /> Arrêter
            </button>
          ) : (
            <button className="bouton" disabled={!actif || enCours || !aTelecharger.trim()} onClick={telecharger}>
              <Download size={15} aria-hidden /> Télécharger
            </button>
          )}
        </span>
      </div>
      <ul className="petit texte-doux">
        {SUGGESTIONS.map(([m, d]) => (
          <li key={m}>
            <button className="lien-discret" disabled={telechargementEnCours} onClick={() => setATelecharger(m)}>
              {m}
            </button>{" "}
            — {d}
          </li>
        ))}
      </ul>
      {avancement && (
        <div className="bloc-haut">
          <div className="petit">
            Téléchargement de {avancement.modele} : {avancement.statut}
            {pourcent !== null && ` — ${pourcent} % (${go(avancement.fait)} sur ${go(avancement.total)})`}
          </div>
          {pourcent !== null && (
            <div className="progression petite-progression">
              <div className="progression-barre" style={{ width: `${pourcent}%` }} />
            </div>
          )}
        </div>
      )}

      <details className="petit bloc-haut">
        <summary className="lien-discret">Adresse d'Ollama (avancé)</summary>
        <input
          className="champ"
          aria-label="Adresse d'Ollama"
          defaultValue={reglages.adresseOllama}
          onBlur={(e) => e.target.value.trim() !== reglages.adresseOllama && changer({ adresseOllama: e.target.value })}
        />
        <span className="texte-doux"> Seules les adresses de ce PC sont acceptées.</span>
      </details>

      <div className="rangee bloc-haut">
        <button
          className="bouton"
          disabled={!actif || !reglages.modeleLocal || choisiAbsent || enCours}
          onClick={() => action(api.testerIaLocale)}
        >
          <FlaskConical size={15} aria-hidden />
          {enCours ? "Essai en cours… (le premier chargement prend jusqu'à une minute)" : "Tester l'IA locale"}
        </button>
      </div>
      {message && <p className={message.erreur ? "message-erreur" : "petit"}>{message.texte}</p>}
    </section>
  );
}
