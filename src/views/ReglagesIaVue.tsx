// Réglages de l'IA : clé Gemini (rangée par Windows), modèles, limites d'utilisation.
import { useEffect, useState } from "react";
import { CircleCheck, PlugZap, Save, Sparkles, Trash, WifiOff } from "lucide-react";
import { api, EtatIa, messageErreur, ModeleIa, Reglages, ReglagesIa } from "../lib/api";
import { useEnLigne } from "../lib/connexion";

interface Props {
  reglages: Reglages;
  onChange: (r: Reglages) => void;
}

/** Voix prédéfinies de la synthèse vocale de Gemini (doc ai.google.dev, octobre 2026). */
const VOIX: [string, string][] = [
  ["Kore", "ferme"],
  ["Puck", "enjouée"],
  ["Charon", "informative"],
  ["Zephyr", "lumineuse"],
  ["Fenrir", "vive"],
  ["Leda", "jeune"],
  ["Orus", "ferme"],
  ["Aoede", "légère"],
  ["Callirrhoe", "décontractée"],
  ["Autonoe", "lumineuse"],
  ["Enceladus", "soufflée"],
  ["Iapetus", "claire"],
  ["Umbriel", "décontractée"],
  ["Algieba", "douce"],
  ["Despina", "douce"],
  ["Erinome", "claire"],
  ["Algenib", "rocailleuse"],
  ["Rasalgethi", "informative"],
  ["Laomedeia", "enjouée"],
  ["Achernar", "douce"],
  ["Alnilam", "ferme"],
  ["Schedar", "égale"],
  ["Gacrux", "mûre"],
  ["Pulcherrima", "directe"],
  ["Achird", "amicale"],
  ["Zubenelgenubi", "décontractée"],
  ["Vindemiatrix", "délicate"],
  ["Sadachbia", "animée"],
  ["Sadaltager", "savante"],
  ["Sulafat", "chaleureuse"],
];

export default function ReglagesIaVue({ reglages, onChange }: Props) {
  const enLigne = useEnLigne();
  const [etat, setEtat] = useState<EtatIa | null>(null);
  const [cle, setCle] = useState("");
  const [modeles, setModeles] = useState<ModeleIa[] | null>(null);
  const [message, setMessage] = useState<{ texte: string; erreur: boolean } | null>(null);
  const [enCours, setEnCours] = useState(false);

  const rafraichir = () => api.etatIa().then(setEtat).catch(() => setEtat(null));

  useEffect(() => {
    rafraichir();
  }, []);

  // La liste des modèles vient de Google (jamais écrite en dur dans l'app).
  useEffect(() => {
    if (etat?.clePresente && enLigne && modeles === null) {
      api.listerModelesIa().then(setModeles).catch(() => setModeles([]));
    }
  }, [etat?.clePresente, enLigne]);

  async function action(f: () => Promise<string | void>) {
    setEnCours(true);
    setMessage(null);
    try {
      const r = await f();
      if (r) setMessage({ texte: r, erreur: false });
    } catch (e) {
      setMessage({ texte: messageErreur(e), erreur: true });
    } finally {
      setEnCours(false);
      rafraichir();
    }
  }

  async function changer(modif: Partial<ReglagesIa>) {
    await action(async () => {
      onChange(
        await api.definirReglagesIa({
          modeleIa: reglages.modeleIa,
          modeleIaRapide: reglages.modeleIaRapide,
          iaParMinute: reglages.iaParMinute,
          iaParJour: reglages.iaParJour,
          modeleTts: reglages.modeleTts,
          voixA: reglages.voixA,
          voixB: reglages.voixB,
          voixVideo: reglages.voixVideo,
          ...modif,
        }),
      );
    });
  }

  const choixModele = (valeur: string, champ: "modeleIa" | "modeleIaRapide", id: string) =>
    modeles && modeles.length > 0 ? (
      <select id={id} value={valeur} onChange={(e) => changer({ [champ]: e.target.value })}>
        {!modeles.some((m) => m.id === valeur) && <option value={valeur}>{valeur}</option>}
        {modeles.map((m) => (
          <option key={m.id} value={m.id} title={m.description}>
            {m.nom} ({m.id})
          </option>
        ))}
      </select>
    ) : (
      <input
        id={id}
        className="champ"
        defaultValue={valeur}
        onBlur={(e) => e.target.value !== valeur && changer({ [champ]: e.target.value.trim() })}
      />
    );

  const choixVoix = (valeur: string, champ: "voixA" | "voixB" | "voixVideo", id: string) => (
    <select id={id} value={valeur} onChange={(e) => changer({ [champ]: e.target.value })}>
      {!VOIX.some(([v]) => v === valeur) && <option value={valeur}>{valeur}</option>}
      {VOIX.map(([v, d]) => (
        <option key={v} value={v}>
          {v} — {d}
        </option>
      ))}
    </select>
  );

  return (
    <section className="carte bloc">
      <div className="rangee entre">
        <h2 className="titre-bloc">
          <Sparkles size={18} className="icone" aria-hidden /> Intelligence artificielle (Gemini)
        </h2>
        <span className={"indicateur" + (enLigne ? "" : " hors-ligne")}>
          <span className="point" aria-hidden /> {enLigne ? "En ligne" : "Hors ligne : IA indisponible"}
        </span>
      </div>
      <p className="texte-doux petit">
        Niveau gratuit de Gemini : Google peut utiliser les extraits envoyés pour améliorer ses produits. Seuls des
        extraits de tes notes de cours sont envoyés, jamais ta clé ni tes statistiques.
      </p>

      <div className="grille-reglages">
        <span>Clé API</span>
        <span className="rangee">
          {etat?.clePresente ? (
            <>
              <span className="indicateur">
                <CircleCheck size={14} className="verdict-juste" aria-hidden /> Enregistrée dans Windows
              </span>
              <button className="bouton" disabled={enCours} onClick={() => action(() => api.supprimerCleIa())}>
                <Trash size={14} aria-hidden /> Supprimer
              </button>
            </>
          ) : (
            <span className="texte-doux">Aucune</span>
          )}
        </span>
        <label htmlFor="cle">{etat?.clePresente ? "Remplacer la clé" : "Ajouter ta clé"}</label>
        <span className="rangee">
          <input
            id="cle"
            className="champ"
            type="password"
            autoComplete="off"
            placeholder="Colle ta clé AI Studio"
            value={cle}
            onChange={(e) => setCle(e.target.value)}
          />
          <button
            className="bouton bouton-principal"
            disabled={!cle.trim() || enCours}
            onClick={() =>
              action(async () => {
                const r = await api.enregistrerCleIa(cle);
                setCle("");
                setModeles(null);
                return r;
              })
            }
          >
            <Save size={15} aria-hidden /> Enregistrer
          </button>
        </span>

        <label htmlFor="modele">Modèle principal (questions, corrections)</label>
        {choixModele(reglages.modeleIa, "modeleIa", "modele")}
        <label htmlFor="modele-rapide">Modèle rapide (tâches simples)</label>
        {choixModele(reglages.modeleIaRapide, "modeleIaRapide", "modele-rapide")}

        <label htmlFor="modele-tts">Modèle de voix (podcasts, vidéos)</label>
        <input
          id="modele-tts"
          className="champ"
          list="modeles-tts"
          defaultValue={reglages.modeleTts}
          onBlur={(e) => e.target.value.trim() !== reglages.modeleTts && changer({ modeleTts: e.target.value.trim() })}
        />
        <datalist id="modeles-tts">
          {(modeles ?? [])
            .filter((m) => m.id.includes("tts"))
            .map((m) => (
              <option key={m.id} value={m.id} />
            ))}
        </datalist>
        <label htmlFor="voix-a">Voix qui pose les questions</label>
        {choixVoix(reglages.voixA, "voixA", "voix-a")}
        <label htmlFor="voix-b">Voix qui explique</label>
        {choixVoix(reglages.voixB, "voixB", "voix-b")}
        <label htmlFor="voix-video">Voix des vidéos</label>
        {choixVoix(reglages.voixVideo, "voixVideo", "voix-video")}

        <label htmlFor="par-minute">Appels maximum par minute</label>
        <input
          id="par-minute"
          className="champ champ-court"
          type="number"
          min={1}
          max={60}
          value={reglages.iaParMinute}
          onChange={(e) => changer({ iaParMinute: Number(e.target.value) || 1 })}
        />
        <label htmlFor="par-jour">Appels maximum par jour</label>
        <input
          id="par-jour"
          className="champ champ-court"
          type="number"
          min={1}
          max={5000}
          value={reglages.iaParJour}
          onChange={(e) => changer({ iaParJour: Number(e.target.value) || 1 })}
        />
      </div>

      <div className="rangee bloc-haut">
        <button className="bouton" disabled={!etat?.clePresente || !enLigne || enCours} onClick={() => action(api.testerIa)}>
          {enLigne ? <PlugZap size={15} aria-hidden /> : <WifiOff size={15} aria-hidden />} Tester la connexion à Gemini
        </button>
        {etat && (
          <span className="texte-doux petit">
            Aujourd'hui : {etat.appelsAujourdhui} appel{etat.appelsAujourdhui > 1 ? "s" : ""} sur {reglages.iaParJour} ·{" "}
            {etat.jetonsAujourdhui.toLocaleString("fr-FR")} jetons
          </span>
        )}
      </div>
      {message && <p className={message.erreur ? "message-erreur" : "petit"}>{message.texte}</p>}
    </section>
  );
}
