// Podcasts de révision à deux voix : Gemini écrit le dialogue à partir des notes, la synthèse vocale
// l'enregistre par morceaux, puis le fichier est compressé et s'écoute hors ligne.
import { useEffect, useMemo, useState } from "react";
import { Archive, Headphones, KeyRound, Mic, Play, RotateCcw, Trash, X } from "lucide-react";
import EntetePage from "../composants/EntetePage";
import Message from "../composants/Message";
import { api, ChapitreIa, EtatIa, messageErreur, OptionsPodcast, Podcast } from "../lib/api";
import { useEnLigne, useModeLocal } from "../lib/connexion";
import MessageModeLocal from "../composants/MessageModeLocal";
import { compresserEnOpus } from "../podcasts/compression";
import Lecteur from "../podcasts/Lecteur";
import { formaterTemps } from "../podcasts/temps";
import { ControleTache, lancerTache, oublierTache, patienter, useDerniereTache, verifierArret } from "../lib/taches";
import { BoutonArreter } from "../composants/TachesIa";

interface Props {
  onOuvrirNote: (chemin: string) => void;
  onReglages: () => void;
}

const cle = (c: { matiere: string; chapitre: string | null }) => `${c.matiere}|${c.chapitre ?? ""}`;
const formatDate = new Intl.DateTimeFormat("fr-FR", { day: "numeric", month: "short", hour: "2-digit", minute: "2-digit" });
const LANGUES: Record<OptionsPodcast["langue"], string> = { francais: "Français", allemand: "Allemand", anglais: "Anglais" };
const TONS: Record<OptionsPodcast["ton"], string> = { detendu: "Détendu", scolaire: "Scolaire", dynamique: "Dynamique" };
const DETAILS: Record<OptionsPodcast["detail"], string> = { essentiel: "L'essentiel", approfondi: "Approfondi" };

interface PodcastTermine {
  podcast: Podcast;
  /** Remarque sur la compression (gardé en WAV…). */
  info: string | null;
}

/** Enregistre les morceaux restants, assemble, puis compresse. Sert aussi à reprendre un podcast interrompu.
 *  Tourne en arrière-plan (gestionnaire de tâches) : l'élève peut changer d'écran. */
async function terminerPodcast(depart: Podcast, c: ControleTache): Promise<PodcastTermine> {
  let p = depart;
  let attentes = 0;
  let info: string | null = null;
  while (p.morceauxFaits < p.morceaux) {
    verifierArret(c);
    c.etape(`Enregistrement des voix : morceau ${p.morceauxFaits + 1} sur ${p.morceaux}…`);
    try {
      p = await api.enregistrerMorceauPodcast(p.id, c.id);
    } catch (e) {
      // Quota de la minute de la synthèse vocale : on attend, deux fois au plus.
      if (c.arretee() || !/quota/i.test(messageErreur(e)) || attentes >= 2) throw e;
      attentes++;
      await patienter(c, 65, (s) => `Quota de la minute atteint : l'enregistrement reprend dans ${s} s…`);
    }
  }
  verifierArret(c);
  if (!p.fichier) {
    c.etape("Assemblage…");
    p = await api.assemblerPodcast(p.id);
  }
  if (p.fichier?.endsWith(".wav")) {
    verifierArret(c);
    c.etape("Compression du fichier audio…");
    // Le WAV reste parfaitement lisible : la compression n'est qu'un gain de place.
    try {
      const octets = await compresserEnOpus(await api.lireAudioPodcast(p.id));
      if (octets) p = await api.remplacerAudioPodcast(p.id, octets);
      else info = "Compression indisponible sur ce PC : le podcast est gardé en WAV (plus volumineux).";
    } catch (e) {
      info = `Compression impossible (${messageErreur(e)}) : le podcast est gardé en WAV.`;
    }
  }
  return { podcast: p, info };
}

/** Langue proposée d'office : celle de la matière pour l'allemand et l'anglais. */
function langueDe(matiere: string): OptionsPodcast["langue"] {
  const m = matiere.toLowerCase();
  return m.includes("allemand") ? "allemand" : m.includes("anglais") ? "anglais" : "francais";
}

export default function PodcastsVue({ onOuvrirNote, onReglages }: Props) {
  const enLigne = useEnLigne();
  const modeLocal = useModeLocal();
  const [etat, setEtat] = useState<EtatIa | null>(null);
  const [chapitres, setChapitres] = useState<ChapitreIa[]>([]);
  const [podcasts, setPodcasts] = useState<Podcast[]>([]);
  const [choix, setChoix] = useState("");
  const [options, setOptions] = useState<OptionsPodcast>({ dureeMinutes: 8, ton: "detendu", detail: "essentiel", langue: "francais" });
  const [estimation, setEstimation] = useState<{ appelsScript: number; appelsVoix: number } | null>(null);
  const tache = useDerniereTache("podcast");
  const progression = tache?.statut === "en-cours" ? (tache.etape ?? "Préparation…") : null;
  const [ecoute, setEcoute] = useState<number | null>(null);
  const [aSupprimer, setASupprimer] = useState<number | null>(null);
  const [erreur, setErreur] = useState<string | null>(null);
  const [info, setInfo] = useState<string | null>(null);

  const charger = () => api.podcasts().then(setPodcasts).catch((e) => setErreur(messageErreur(e)));

  useEffect(() => {
    api.etatIa().then(setEtat).catch(() => setEtat(null));
    api.chapitresIa().then(setChapitres).catch((e) => setErreur(messageErreur(e)));
    charger();
  }, []);

  useEffect(() => {
    api.estimerPodcast(options.dureeMinutes).then(setEstimation).catch(() => setEstimation(null));
  }, [options.dureeMinutes]);

  const chapitre = useMemo(() => chapitres.find((c) => cle(c) === choix) ?? null, [chapitres, choix]);
  const matieres = [...new Set(chapitres.map((c) => c.matiere))];
  const disponible = enLigne && !modeLocal && !!etat?.clePresente;

  // La liste suit l'avancement (le podcast apparaît dès que le dialogue est écrit).
  useEffect(() => {
    charger();
  }, [tache?.statut, tache?.etape]);

  // Podcast terminé (ici ou pendant qu'on était ailleurs) : il s'ouvre dans le lecteur.
  useEffect(() => {
    if (tache?.statut === "finie") {
      const { podcast, info } = tache.resultat as PodcastTermine;
      setEcoute(podcast.id);
      setInfo(info);
      oublierTache(tache.id);
    }
  }, [tache?.statut]);

  function lancer(titre: string, action: (c: ControleTache) => Promise<PodcastTermine>) {
    setErreur(null);
    setInfo(null);
    const resume = () => "Ton podcast est prêt à être écouté";
    lancerTache({ genre: "podcast", titre, ecran: "podcasts", resume }, action).catch(() => {});
  }

  const creer = () =>
    chapitre &&
    lancer(`Podcast · ${chapitre.chapitre ?? chapitre.matiere}`, async (c) => {
      c.etape("Gemini écrit le dialogue à partir de tes notes…");
      const p = await api.ecrirePodcast(chapitre.matiere, chapitre.chapitre, options, c.id);
      return terminerPodcast(p, c);
    });

  async function supprimer(id: number) {
    setASupprimer(null);
    if (ecoute === id) setEcoute(null);
    try {
      await api.supprimerPodcast(id);
      charger();
    } catch (e) {
      setErreur(messageErreur(e));
    }
  }

  const choisirChapitre = (valeur: string) => {
    setChoix(valeur);
    const c = chapitres.find((x) => cle(x) === valeur);
    if (c) setOptions((o) => ({ ...o, langue: langueDe(c.matiere) }));
  };

  return (
    <div className="page">
      <EntetePage
        icone={Headphones}
        surTitre="Créer avec l'IA"
        titre="Podcasts"
        description="Deux voix discutent de ton chapitre à partir de tes notes ; le podcast s'écoute ensuite hors ligne."
      />
      {modeLocal && <MessageModeLocal quoi="la création d'un podcast" />}

      <section className="carte bloc apparait" style={{ "--i": 1 } as React.CSSProperties}>
        <h2 className="titre-bloc">
          <Mic size={18} className="icone" aria-hidden /> Nouveau podcast
        </h2>
        <p className="texte-doux petit">Uniquement à partir de tes notes : choisis le chapitre, la durée et le ton.</p>
        <div className="grille-reglages">
          <label htmlFor="podcast-chapitre">Chapitre</label>
          <select id="podcast-chapitre" value={choix} onChange={(e) => choisirChapitre(e.target.value)}>
            <option value="">Choisis…</option>
            {matieres.map((m) => (
              <optgroup key={m} label={m}>
                {chapitres
                  .filter((c) => c.matiere === m)
                  .map((c) => (
                    <option key={cle(c)} value={cle(c)}>
                      {c.chapitre ?? "Notes générales de la matière"}
                    </option>
                  ))}
              </optgroup>
            ))}
          </select>
          <label htmlFor="podcast-duree">Durée</label>
          <select
            id="podcast-duree"
            value={options.dureeMinutes}
            onChange={(e) => setOptions({ ...options, dureeMinutes: Number(e.target.value) })}
          >
            {[4, 8, 12, 16].map((d) => (
              <option key={d} value={d}>
                environ {d} min
              </option>
            ))}
          </select>
          <label htmlFor="podcast-ton">Ton</label>
          <select
            id="podcast-ton"
            value={options.ton}
            onChange={(e) => setOptions({ ...options, ton: e.target.value as OptionsPodcast["ton"] })}
          >
            {Object.entries(TONS).map(([v, n]) => (
              <option key={v} value={v}>
                {n}
              </option>
            ))}
          </select>
          <label htmlFor="podcast-detail">Niveau de détail</label>
          <select
            id="podcast-detail"
            value={options.detail}
            onChange={(e) => setOptions({ ...options, detail: e.target.value as OptionsPodcast["detail"] })}
          >
            {Object.entries(DETAILS).map(([v, n]) => (
              <option key={v} value={v}>
                {n}
              </option>
            ))}
          </select>
          <label htmlFor="podcast-langue">Langue</label>
          <select
            id="podcast-langue"
            value={options.langue}
            onChange={(e) => setOptions({ ...options, langue: e.target.value as OptionsPodcast["langue"] })}
          >
            {Object.entries(LANGUES).map(([v, n]) => (
              <option key={v} value={v}>
                {n}
              </option>
            ))}
          </select>
        </div>
        <div className="rangee bloc-haut">
          <button className="bouton bouton-principal" disabled={!disponible || !chapitre || progression !== null} onClick={creer}>
            <Mic size={16} aria-hidden /> Créer le podcast
          </button>
          <BoutonArreter tache={tache} />
          {estimation && (
            <span className="texte-doux petit">
              {estimation.appelsScript} appel pour le dialogue + {estimation.appelsVoix} pour les voix (quelques minutes)
            </span>
          )}
          {enLigne && etat && !etat.clePresente && (
            <button className="lien-discret petit" onClick={onReglages}>
              <KeyRound size={13} aria-hidden /> Ajouter une clé Gemini
            </button>
          )}
          {!enLigne && <span className="texte-doux petit">Hors ligne : création indisponible, écoute possible.</span>}
        </div>
        {progression && (
          <p className="progression-texte">
            {tache?.titre} : {progression}
          </p>
        )}
        {erreur && <p className="message-erreur">{erreur}</p>}
        {tache?.statut === "erreur" && <p className="message-erreur">{tache.erreur}</p>}
        {tache?.statut === "interrompue" && (
          <Message genre="info">Création interrompue. Tu pourras la reprendre depuis « Mes podcasts ».</Message>
        )}
        {info && <Message genre="info">{info}</Message>}
      </section>

      <section className="carte bloc apparait" style={{ "--i": 2 } as React.CSSProperties}>
        <h2 className="titre-bloc">
          <Headphones size={18} className="icone" aria-hidden /> Mes podcasts
        </h2>
        {podcasts.length === 0 && <p className="texte-doux">Aucun podcast pour l'instant.</p>}
        <ul className="liste-podcasts liste-elements">
          {podcasts.map((p) => {
            const incomplet = !p.fichier;
            return (
              <li key={p.id}>
                <div className="rangee entre">
                  <div className="sujet-infos">
                    <strong>{p.titre}</strong>
                    <span className="texte-doux petit">
                      {p.matiere} › {p.chapitre ?? "notes générales"} · {LANGUES[p.options.langue]} ·{" "}
                      {p.dureeMs ? formaterTemps(p.dureeMs / 1000) : `≈ ${p.options.dureeMinutes} min`} ·{" "}
                      {formatDate.format(new Date(p.date))}
                    </span>
                    {incomplet && (
                      <span className="verdict-presque petit">
                        Enregistrement interrompu ({p.morceauxFaits} morceau{p.morceauxFaits > 1 ? "x" : ""} sur{" "}
                        {p.morceaux})
                      </span>
                    )}
                  </div>
                  <div className="rangee">
                    {incomplet ? (
                      <button
                        className="bouton"
                        disabled={!disponible || progression !== null}
                        onClick={() => lancer(`Podcast · ${p.titre}`, (c) => terminerPodcast(p, c))}
                      >
                        <RotateCcw size={14} aria-hidden /> Reprendre l'enregistrement
                      </button>
                    ) : (
                      <>
                        {p.fichier?.endsWith(".wav") && (
                          <button
                            className="lien-discret petit"
                            disabled={progression !== null}
                            title="Le fichier WAV prend environ 3 Mo par minute ; compressé, environ 0,25 Mo."
                            onClick={() => lancer(`Compression · ${p.titre}`, (c) => terminerPodcast(p, c))}
                          >
                            <Archive size={13} aria-hidden /> Compresser
                          </button>
                        )}
                        <button className="bouton" onClick={() => setEcoute(ecoute === p.id ? null : p.id)}>
                          {ecoute === p.id ? <X size={14} aria-hidden /> : <Play size={14} aria-hidden />}
                          {ecoute === p.id ? "Fermer" : "Écouter"}
                        </button>
                      </>
                    )}
                    {aSupprimer === p.id ? (
                      <>
                        <button className="bouton bouton-danger" onClick={() => supprimer(p.id)}>
                          <Trash size={14} aria-hidden /> Supprimer le podcast
                        </button>
                        <button className="lien-discret petit" onClick={() => setASupprimer(null)}>
                          Annuler
                        </button>
                      </>
                    ) : (
                      <button className="lien-discret petit" onClick={() => setASupprimer(p.id)}>
                        <Trash size={13} aria-hidden /> Supprimer
                      </button>
                    )}
                  </div>
                </div>
                {ecoute === p.id && !incomplet && <Lecteur podcast={p} onOuvrirNote={onOuvrirNote} />}
              </li>
            );
          })}
        </ul>
      </section>
    </div>
  );
}
