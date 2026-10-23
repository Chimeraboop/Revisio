// Lecteur de podcast hors ligne : lecture/pause, ±15 s, vitesse, reprise à l'endroit où on s'était arrêté,
// et transcription qui suit la voix (un clic sur une réplique y saute).
import { useEffect, useRef, useState } from "react";
import { ExternalLink, Pause, Play, RotateCcw, RotateCw } from "lucide-react";
import { api, Podcast } from "../lib/api";
import { formaterTemps, repliqueEnCours } from "./temps";
import { typeAudio } from "./compression";

interface Props {
  podcast: Podcast;
  onOuvrirNote: (chemin: string) => void;
}

const VITESSES = [0.75, 1, 1.25, 1.5, 1.75];

export default function Lecteur({ podcast, onOuvrirNote }: Props) {
  const audio = useRef<HTMLAudioElement>(null);
  const [url, setUrl] = useState<string | null>(null);
  const [erreur, setErreur] = useState<string | null>(null);
  const [lecture, setLecture] = useState(false);
  const [temps, setTemps] = useState(podcast.positionMs / 1000);
  const [duree, setDuree] = useState((podcast.dureeMs ?? 0) / 1000);
  const [vitesse, setVitesse] = useState(1);
  const [suivre, setSuivre] = useState(true);
  const derniereSauvegarde = useRef(0);
  const liste = useRef<HTMLOListElement>(null);

  useEffect(() => {
    let lien: string | null = null;
    api
      .lireAudioPodcast(podcast.id)
      .then((octets) => {
        lien = URL.createObjectURL(new Blob([octets], { type: typeAudio(podcast.fichier ?? "") }));
        setUrl(lien);
      })
      .catch((e) => setErreur(String(e)));
    return () => {
      if (lien) URL.revokeObjectURL(lien);
    };
  }, [podcast.id, podcast.fichier]);

  const sauver = (secondes: number) => {
    derniereSauvegarde.current = Date.now();
    api.positionPodcast(podcast.id, Math.round(secondes * 1000)).catch(() => {});
  };

  // Position gardée en quittant l'écran.
  useEffect(() => () => {
    if (audio.current) sauver(audio.current.currentTime);
  }, []);

  useEffect(() => {
    if (audio.current) audio.current.playbackRate = vitesse;
  }, [vitesse]);

  const courante = repliqueEnCours(podcast.repliques, temps * 1000);

  useEffect(() => {
    if (!suivre || courante < 0) return;
    liste.current?.children[courante]?.scrollIntoView({ block: "nearest", behavior: "smooth" });
  }, [courante, suivre]);

  const aller = (secondes: number) => {
    if (!audio.current) return;
    audio.current.currentTime = Math.max(0, Math.min(secondes, duree || secondes));
    setTemps(audio.current.currentTime);
  };

  const basculer = () => {
    const a = audio.current;
    if (!a) return;
    if (a.paused) a.play().catch((e) => setErreur(String(e)));
    else a.pause();
  };

  const source = (id: string) => podcast.extraits.find((e) => e.id === id);

  return (
    <div className="lecteur">
      {erreur && <p className="message-erreur">Lecture impossible : {erreur}</p>}
      {url && (
        <audio
          ref={audio}
          src={url}
          preload="auto"
          onLoadedMetadata={(e) => {
            const a = e.currentTarget;
            if (Number.isFinite(a.duration)) setDuree(a.duration);
            a.playbackRate = vitesse;
            // Reprise là où on s'était arrêté (sauf tout à la fin).
            const reprise = podcast.positionMs / 1000;
            if (reprise > 0 && reprise < (Number.isFinite(a.duration) ? a.duration : Infinity) - 5) a.currentTime = reprise;
          }}
          onTimeUpdate={(e) => {
            const t = e.currentTarget.currentTime;
            setTemps(t);
            if (Date.now() - derniereSauvegarde.current > 10_000) sauver(t);
          }}
          onPlay={() => setLecture(true)}
          onPause={(e) => {
            setLecture(false);
            sauver(e.currentTarget.currentTime);
          }}
          onEnded={() => sauver(0)}
        />
      )}
      <div className="commandes-lecteur">
        <button className="bouton" disabled={!url} onClick={() => aller(temps - 15)} aria-label="Reculer de 15 secondes">
          <RotateCcw size={15} aria-hidden /> 15 s
        </button>
        <button className="bouton bouton-principal bouton-lecture" disabled={!url} onClick={basculer}>
          {lecture ? <Pause size={16} aria-hidden /> : <Play size={16} aria-hidden />}
          {lecture ? "Pause" : temps > 0 ? "Reprendre" : "Écouter"}
        </button>
        <button className="bouton" disabled={!url} onClick={() => aller(temps + 15)} aria-label="Avancer de 15 secondes">
          15 s <RotateCw size={15} aria-hidden />
        </button>
        <input
          className="barre-lecture"
          type="range"
          aria-label="Position dans le podcast"
          min={0}
          max={Math.max(duree, 1)}
          step={1}
          value={Math.min(temps, duree || temps)}
          disabled={!url}
          onChange={(e) => aller(Number(e.target.value))}
        />
        <span className="temps-lecture">
          {formaterTemps(temps)} / {formaterTemps(duree)}
        </span>
        <select aria-label="Vitesse de lecture" value={vitesse} onChange={(e) => setVitesse(Number(e.target.value))}>
          {VITESSES.map((v) => (
            <option key={v} value={v}>
              {String(v).replace(".", ",")}×
            </option>
          ))}
        </select>
      </div>

      <div className="rangee entre bloc-haut">
        <strong>Transcription</strong>
        <label className="case petit">
          <input type="checkbox" checked={suivre} onChange={(e) => setSuivre(e.target.checked)} />
          Suivre la lecture
        </label>
      </div>
      <ol className="transcription" ref={liste}>
        {podcast.repliques.map((r, i) => {
          const s = r.source ? source(r.source) : undefined;
          return (
            <li
              key={i}
              className={"replique voix-" + r.voix + (i === courante ? " en-cours" : "")}
              onClick={() => r.debutMs !== null && aller(r.debutMs / 1000)}
            >
              <strong>{r.nom}</strong> {r.texte}
              {s && (
                <button
                  className="lien-discret petit lien-source"
                  title={`${s.titreNote}${s.section ? ` › ${s.section}` : ""}`}
                  onClick={(e) => {
                    e.stopPropagation();
                    onOuvrirNote(s.noteChemin);
                  }}
                >
                  <ExternalLink size={12} aria-hidden /> {s.titreNote}
                </button>
              )}
            </li>
          );
        })}
      </ol>
    </div>
  );
}
