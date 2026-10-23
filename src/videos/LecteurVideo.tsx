// Lecteur de vidéo hors ligne : la vidéo, puis la liste des diapositives (un clic y saute) avec le texte
// de la narration et la note d'où vient chaque diapositive.
import { useEffect, useRef, useState } from "react";
import { ExternalLink } from "lucide-react";
import { api, Video } from "../lib/api";
import { formaterTemps } from "../podcasts/temps";
import { TexteMaths } from "../revision/symboles";
import { segments, typeVideo } from "./montage";

interface Props {
  video: Video;
  onOuvrirNote: (chemin: string) => void;
}

export default function LecteurVideo({ video, onOuvrirNote }: Props) {
  const lecteur = useRef<HTMLVideoElement>(null);
  const [url, setUrl] = useState<string | null>(null);
  const [erreur, setErreur] = useState<string | null>(null);
  const [temps, setTemps] = useState(0);
  const [texte, setTexte] = useState(false);
  const segs = segments(video.diapos);
  const courante = segs.filter((s) => s.debut <= temps * 1000).length - 1;

  useEffect(() => {
    let lien: string | null = null;
    api
      .lireFichierVideo(video.id)
      .then((octets) => {
        lien = URL.createObjectURL(new Blob([octets], { type: typeVideo(video.fichier ?? "") }));
        setUrl(lien);
      })
      .catch((e) => setErreur(String(e)));
    return () => {
      if (lien) URL.revokeObjectURL(lien);
    };
  }, [video.id, video.fichier]);

  const aller = (ms: number) => {
    if (!lecteur.current) return;
    lecteur.current.currentTime = ms / 1000;
    setTemps(ms / 1000);
  };

  return (
    <div className="lecteur">
      {erreur && <p className="message-erreur">Lecture impossible : {erreur}</p>}
      {url && (
        <video
          ref={lecteur}
          className="video-revision"
          src={url}
          controls
          preload="auto"
          onTimeUpdate={(e) => setTemps(e.currentTarget.currentTime)}
          onError={() => setErreur("ce format de vidéo n'est pas lisible ici.")}
        />
      )}
      <div className="rangee entre bloc-haut">
        <strong>Diapositives</strong>
        <label className="case petit">
          <input type="checkbox" checked={texte} onChange={(e) => setTexte(e.target.checked)} />
          Afficher le texte de la narration
        </label>
      </div>
      <ol className="transcription">
        {video.diapos.map((d, i) => {
          const s = video.extraits.find((e) => e.id === d.source);
          return (
            <li key={i} className={"replique" + (i === courante ? " en-cours" : "")} onClick={() => aller(segs[i].debut)}>
              <span className="texte-doux petit temps-lecture">{formaterTemps(segs[i].debut / 1000)}</span>{" "}
              <strong>
                <TexteMaths texte={d.titre} />
              </strong>
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
              {texte && <p className="texte-doux petit narration-diapo">{d.narration}</p>}
            </li>
          );
        })}
      </ol>
    </div>
  );
}
