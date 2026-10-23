// Vidéos de révision : Gemini prépare un diaporama et sa narration à partir des notes, la synthèse vocale
// enregistre la narration diapositive par diapositive, puis Révisio monte la vidéo (MP4) sur le PC.
// Elle se regarde ensuite hors ligne, ou se partage (fichier dans le dossier de données de Révisio).
import { useEffect, useMemo, useState } from "react";
import { Clapperboard, Film, FolderOpen, KeyRound, Play, RotateCcw, Trash, Wand, X } from "lucide-react";
import EntetePage from "../composants/EntetePage";
import { api, ChapitreIa, EtatIa, messageErreur, OptionsVideo, Video } from "../lib/api";
import { useEnLigne, useModeLocal } from "../lib/connexion";
import MessageModeLocal from "../composants/MessageModeLocal";
import { formaterTemps } from "../podcasts/temps";
import LecteurVideo from "../videos/LecteurVideo";
import { monterVideo } from "../videos/montage";
import Message from "../composants/Message";
import { ControleTache, lancerTache, oublierTache, patienter, useDerniereTache, verifierArret } from "../lib/taches";
import { BoutonArreter } from "../composants/TachesIa";

interface Props {
  onOuvrirNote: (chemin: string) => void;
  onReglages: () => void;
}

const cle = (c: { matiere: string; chapitre: string | null }) => `${c.matiere}|${c.chapitre ?? ""}`;
const formatDate = new Intl.DateTimeFormat("fr-FR", { day: "numeric", month: "short", hour: "2-digit", minute: "2-digit" });
const LANGUES: Record<OptionsVideo["langue"], string> = { francais: "Français", allemand: "Allemand", anglais: "Anglais" };

function langueDe(matiere: string): OptionsVideo["langue"] {
  const m = matiere.toLowerCase();
  return m.includes("allemand") ? "allemand" : m.includes("anglais") ? "anglais" : "francais";
}

/** Enregistre les narrations qui manquent, puis monte la vidéo. Sert aussi à reprendre une vidéo interrompue.
 *  Tourne en arrière-plan (gestionnaire de tâches) : l'élève peut changer d'écran. */
async function terminerVideo(depart: Video, c: ControleTache): Promise<Video> {
  let v = depart;
  let attentes = 0;
  while (v.faites < v.diapos.length) {
    verifierArret(c);
    c.etape(`Enregistrement de la narration : diapositive ${v.faites + 1} sur ${v.diapos.length}…`);
    try {
      v = await api.enregistrerNarrationVideo(v.id, c.id);
    } catch (e) {
      // La synthèse vocale gratuite n'accepte que quelques appels par minute : on attend une minute, deux fois
      // au plus (au-delà, c'est sans doute le quota du jour : « Reprendre » plus tard).
      if (c.arretee() || !/quota/i.test(messageErreur(e)) || attentes >= 2) throw e;
      attentes++;
      await patienter(c, 65, (s) => `Quota de la minute atteint : la narration reprend dans ${s} s…`);
    }
  }
  if (!v.fichier) {
    verifierArret(c);
    c.etape("Montage de la vidéo…");
    const montage = await monterVideo(v, await api.narrationVideo(v.id), (f) => {
      verifierArret(c);
      c.etape(`Montage de la vidéo : ${Math.round(f * 100)} %`);
    });
    verifierArret(c);
    c.etape("Enregistrement du fichier…");
    v = await api.enregistrerFichierVideo(v.id, montage.octets, montage.dureeMs);
  }
  return v;
}

export default function VideosVue({ onOuvrirNote, onReglages }: Props) {
  const enLigne = useEnLigne();
  const modeLocal = useModeLocal();
  const [etat, setEtat] = useState<EtatIa | null>(null);
  const [chapitres, setChapitres] = useState<ChapitreIa[]>([]);
  const [videos, setVideos] = useState<Video[]>([]);
  const [choix, setChoix] = useState("");
  const [options, setOptions] = useState<OptionsVideo>({ diapos: 8, langue: "francais" });
  const tache = useDerniereTache("video");
  const progression = tache?.statut === "en-cours" ? (tache.etape ?? "Préparation…") : null;
  const [regarde, setRegarde] = useState<number | null>(null);
  const [aSupprimer, setASupprimer] = useState<number | null>(null);
  const [erreur, setErreur] = useState<string | null>(null);

  const charger = () => api.videos().then(setVideos).catch((e) => setErreur(messageErreur(e)));

  useEffect(() => {
    api.etatIa().then(setEtat).catch(() => setEtat(null));
    api.chapitresIa().then(setChapitres).catch((e) => setErreur(messageErreur(e)));
    charger();
  }, []);

  const chapitre = useMemo(() => chapitres.find((c) => cle(c) === choix) ?? null, [chapitres, choix]);
  const matieres = [...new Set(chapitres.map((c) => c.matiere))];
  const disponible = enLigne && !modeLocal && !!etat?.clePresente;

  // La liste suit l'avancement (la vidéo apparaît dès que les diapositives sont prêtes).
  useEffect(() => {
    charger();
  }, [tache?.statut, tache?.etape]);

  // Vidéo terminée (ici ou pendant qu'on était ailleurs) : elle s'ouvre dans le lecteur.
  useEffect(() => {
    if (tache?.statut === "finie") {
      setRegarde((tache.resultat as Video).id);
      oublierTache(tache.id);
    }
  }, [tache?.statut]);

  function lancer(titre: string, action: (c: ControleTache) => Promise<Video>) {
    setErreur(null);
    lancerTache({ genre: "video", titre, ecran: "videos", resume: () => "Ta vidéo est prête" }, action).catch(() => {});
  }

  const creer = () =>
    chapitre &&
    lancer(`Vidéo · ${chapitre.chapitre ?? chapitre.matiere}`, async (c) => {
      c.etape("Gemini prépare les diapositives à partir de tes notes…");
      const v = await api.ecrireVideo(chapitre.matiere, chapitre.chapitre, options, c.id);
      return terminerVideo(v, c);
    });

  async function supprimer(id: number) {
    setASupprimer(null);
    if (regarde === id) setRegarde(null);
    try {
      await api.supprimerVideo(id);
      charger();
    } catch (e) {
      setErreur(messageErreur(e));
    }
  }

  async function montrer(id: number) {
    try {
      await api.montrerFichier(await api.cheminVideo(id));
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
        icone={Clapperboard}
        surTitre="Créer avec l'IA"
        titre="Vidéos"
        description="Un diaporama commenté de ton chapitre, monté sur ton PC en MP4, à regarder hors ligne ou à partager."
      />
      {modeLocal && <MessageModeLocal quoi="la création d'une vidéo" />}

      <section className="carte bloc apparait" style={{ "--i": 1 } as React.CSSProperties}>
        <h2 className="titre-bloc">
          <Film size={18} className="icone" aria-hidden /> Nouvelle vidéo
        </h2>
        <p className="texte-doux petit">Uniquement à partir de tes notes, avec tes schémas quand ils s'y prêtent.</p>
        <div className="grille-reglages">
          <label htmlFor="video-chapitre">Chapitre</label>
          <select id="video-chapitre" value={choix} onChange={(e) => choisirChapitre(e.target.value)}>
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
          <label htmlFor="video-diapos">Diapositives</label>
          <select
            id="video-diapos"
            value={options.diapos}
            onChange={(e) => setOptions({ ...options, diapos: Number(e.target.value) })}
          >
            {[5, 8, 10, 12].map((n) => (
              <option key={n} value={n}>
                {n} diapositives (≈ {Math.round(n * 0.5)} à {Math.round(n * 0.75)} min)
              </option>
            ))}
          </select>
          <label htmlFor="video-langue">Langue</label>
          <select
            id="video-langue"
            value={options.langue}
            onChange={(e) => setOptions({ ...options, langue: e.target.value as OptionsVideo["langue"] })}
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
            <Wand size={16} aria-hidden /> Créer la vidéo
          </button>
          <BoutonArreter tache={tache} />
          <span className="texte-doux petit">
            1 appel pour les diapositives + {options.diapos} pour la narration (quelques minutes)
          </span>
          {enLigne && etat && !etat.clePresente && (
            <button className="lien-discret petit" onClick={onReglages}>
              <KeyRound size={13} aria-hidden /> Ajouter une clé Gemini
            </button>
          )}
          {!enLigne && <span className="texte-doux petit">Hors ligne : création indisponible, lecture possible.</span>}
        </div>
        {progression && (
          <p className="progression-texte">
            {tache?.titre} : {progression}
          </p>
        )}
        {erreur && <p className="message-erreur">{erreur}</p>}
        {tache?.statut === "erreur" && <p className="message-erreur">{tache.erreur}</p>}
        {tache?.statut === "interrompue" && (
          <Message genre="info">Création interrompue. Tu pourras la reprendre depuis « Mes vidéos ».</Message>
        )}
      </section>

      <section className="carte bloc apparait" style={{ "--i": 2 } as React.CSSProperties}>
        <h2 className="titre-bloc">
          <Clapperboard size={18} className="icone" aria-hidden /> Mes vidéos
        </h2>
        {videos.length === 0 && <p className="texte-doux">Aucune vidéo pour l'instant.</p>}
        <ul className="liste-podcasts liste-elements">
          {videos.map((v) => {
            const narrationsManquantes = v.faites < v.diapos.length;
            return (
              <li key={v.id}>
                <div className="rangee entre">
                  <div className="sujet-infos">
                    <strong>{v.titre}</strong>
                    <span className="texte-doux petit">
                      {v.matiere} › {v.chapitre ?? "notes générales"} · {v.diapos.length} diapositives ·{" "}
                      {LANGUES[v.options.langue]}
                      {v.dureeMs ? ` · ${formaterTemps(v.dureeMs / 1000)}` : ""} · {formatDate.format(new Date(v.date))}
                    </span>
                    {!v.fichier && (
                      <span className="verdict-presque petit">
                        {narrationsManquantes
                          ? `Narration interrompue (${v.faites} diapositive${v.faites > 1 ? "s" : ""} sur ${v.diapos.length})`
                          : "Narration prête, vidéo pas encore montée"}
                      </span>
                    )}
                  </div>
                  <div className="rangee">
                    {!v.fichier ? (
                      <button
                        className="bouton"
                        disabled={(narrationsManquantes && !disponible) || progression !== null}
                        onClick={() => lancer(`Vidéo · ${v.titre}`, (c) => terminerVideo(v, c))}
                      >
                        {narrationsManquantes ? <RotateCcw size={14} aria-hidden /> : <Clapperboard size={14} aria-hidden />}
                        {narrationsManquantes ? "Reprendre" : "Monter la vidéo"}
                      </button>
                    ) : (
                      <>
                        <button className="lien-discret petit" onClick={() => montrer(v.id)}>
                          <FolderOpen size={13} aria-hidden /> Montrer le fichier
                        </button>
                        <button className="bouton" onClick={() => setRegarde(regarde === v.id ? null : v.id)}>
                          {regarde === v.id ? <X size={14} aria-hidden /> : <Play size={14} aria-hidden />}
                          {regarde === v.id ? "Fermer" : "Regarder"}
                        </button>
                      </>
                    )}
                    {aSupprimer === v.id ? (
                      <>
                        <button className="bouton bouton-danger" onClick={() => supprimer(v.id)}>
                          <Trash size={14} aria-hidden /> Supprimer la vidéo
                        </button>
                        <button className="lien-discret petit" onClick={() => setASupprimer(null)}>
                          Annuler
                        </button>
                      </>
                    ) : (
                      <button className="lien-discret petit" onClick={() => setASupprimer(v.id)}>
                        <Trash size={13} aria-hidden /> Supprimer
                      </button>
                    )}
                  </div>
                </div>
                {regarde === v.id && v.fichier && <LecteurVideo video={v} onOuvrirNote={onOuvrirNote} />}
              </li>
            );
          })}
        </ul>
      </section>
    </div>
  );
}
