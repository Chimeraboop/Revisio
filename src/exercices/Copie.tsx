// L'épreuve : le sujet, les réponses de l'élève et le chronomètre.
// Le brouillon est gardé dans le navigateur intégré : fermer l'app ne fait pas perdre la copie.
import { useEffect, useRef, useState } from "react";
import { DoorOpen, Send, Timer } from "lucide-react";
import type { QuestionSujet, SujetEnregistre } from "../lib/api";
import { MarkdownObsidian } from "../markdown/rendu";
import Graphique from "./Graphique";

interface Props {
  sujet: SujetEnregistre;
  /** Mode examen : la copie est ramassée automatiquement à la fin du temps. */
  ramassageAutomatique: boolean;
  onRendre: (reponses: string[], dureeMs: number) => Promise<void>;
  onAbandonner: () => void;
}

interface Brouillon {
  reponses: string[];
  debut: number;
}

const cleBrouillon = (id: number) => `revisio-copie-${id}`;

function lireBrouillon(id: number): Brouillon | null {
  try {
    const b = JSON.parse(localStorage.getItem(cleBrouillon(id)) ?? "null") as Brouillon | null;
    return b && Array.isArray(b.reponses) && typeof b.debut === "number" ? b : null;
  } catch {
    return null;
  }
}

export function brouillonExiste(id: number): boolean {
  return lireBrouillon(id) !== null;
}

export function effacerBrouillon(id: number) {
  try {
    localStorage.removeItem(cleBrouillon(id));
  } catch {
    // Stockage indisponible : rien à effacer.
  }
}

export function formatDuree(ms: number): string {
  const s = Math.max(0, Math.round(ms / 1000));
  const h = Math.floor(s / 3600);
  const m = Math.floor((s % 3600) / 60);
  const sec = String(s % 60).padStart(2, "0");
  return h > 0 ? `${h} h ${String(m).padStart(2, "0")} min` : `${m}:${sec}`;
}

export const navigationSujet = { source: "", ouvrirNote: () => {} };

/** Titre lisible d'une partie d'épreuve (« explication » → « Explication »). */
export function titrePartie(p: string): string {
  return p ? p.charAt(0).toUpperCase() + p.slice(1) : "";
}

function ChampReponse({
  q,
  numero,
  valeur,
  onChange,
}: {
  q: QuestionSujet;
  numero: number;
  valeur: string;
  onChange: (v: string) => void;
}) {
  const nom = `Réponse à la question ${numero}`;
  if (q.type_reponse === "choix") {
    return (
      <div className="choix-qcm" role="radiogroup" aria-label={nom}>
        {q.choix.map((c, i) => (
          <button
            key={i}
            role="radio"
            aria-checked={valeur === c}
            className={"choix" + (valeur === c ? " choisi" : "")}
            onClick={() => onChange(c)}
          >
            <kbd>{String.fromCharCode(65 + i)}</kbd>
            <span className="choix-texte">
              <MarkdownObsidian markdown={c} navigation={navigationSujet} />
            </span>
          </button>
        ))}
      </div>
    );
  }
  if (q.type_reponse === "texte") {
    const mots = valeur.split(/\s+/).filter(Boolean).length;
    return (
      <>
        <textarea
          className="champ zone-reponse"
          aria-label={nom}
          rows={q.points >= 6 ? 12 : 6}
          value={valeur}
          onChange={(e) => onChange(e.target.value)}
          placeholder="Rédige ta réponse…"
        />
        <div className="texte-doux petit compteur-mots">
          {mots} mot{mots > 1 ? "s" : ""}
        </div>
      </>
    );
  }
  return (
    <input
      className="champ champ-court"
      aria-label={nom}
      value={valeur}
      onChange={(e) => onChange(e.target.value)}
      placeholder={q.type_reponse === "nombre" ? "Un nombre : 2,5 ou -3/4 ou √2" : "Ta réponse"}
      spellCheck={false}
    />
  );
}

export default function Copie({ sujet, ramassageAutomatique, onRendre, onAbandonner }: Props) {
  const s = sujet.sujet;
  const [brouillon] = useState(() => lireBrouillon(sujet.id));
  const [reponses, setReponses] = useState<string[]>(() =>
    s.questions.map((_, i) => brouillon?.reponses[i] ?? ""),
  );
  const debut = useRef(brouillon?.debut ?? Date.now());
  const [maintenant, setMaintenant] = useState(Date.now());
  const [confirmer, setConfirmer] = useState(false);
  const [envoi, setEnvoi] = useState(false);
  const rendue = useRef(false);

  const dureeMs = sujet.dureeMinutes * 60_000;
  const ecoule = maintenant - debut.current;
  const restant = dureeMs - ecoule;
  const sansReponse = reponses.filter((r) => !r.trim()).length;

  useEffect(() => {
    const t = window.setInterval(() => setMaintenant(Date.now()), 1000);
    return () => window.clearInterval(t);
  }, []);

  useEffect(() => {
    try {
      localStorage.setItem(cleBrouillon(sujet.id), JSON.stringify({ reponses, debut: debut.current }));
    } catch {
      // Pas de stockage : la copie n'est simplement pas gardée en cas de fermeture.
    }
  }, [reponses, sujet.id]);

  async function rendre() {
    if (rendue.current) return;
    rendue.current = true;
    setEnvoi(true);
    try {
      await onRendre(reponses, Math.min(Date.now() - debut.current, ramassageAutomatique ? dureeMs : Infinity));
    } catch {
      rendue.current = false;
      setEnvoi(false);
    }
  }

  // Mode examen : la copie est ramassée quand le temps est écoulé.
  useEffect(() => {
    if (ramassageAutomatique && restant <= 0) rendre();
  }, [ramassageAutomatique, restant <= 0]);

  const modifier = (i: number, v: string) => setReponses((r) => r.map((x, j) => (j === i ? v : x)));
  const total = s.questions.reduce((t, q) => t + q.points, 0);

  return (
    <div className="page copie">
      <div className="copie-entete carte">
        <div>
          <div className="texte-doux petit">
            {sujet.typeNom} · {sujet.matiere}
          </div>
          <strong>{s.titre}</strong>
        </div>
        <div
          className={"chrono" + (restant <= 0 ? " depasse" : restant < 5 * 60_000 ? " bientot" : "")}
          role="timer"
          aria-label="Temps restant"
        >
          <Timer size={18} className="icone" aria-hidden />
          {restant > 0 ? formatDuree(restant) : ramassageAutomatique ? "0:00" : `+${formatDuree(-restant)}`}
          <span className="petit texte-doux"> / {sujet.dureeMinutes} min</span>
        </div>
      </div>

      <section className="carte bloc">
        <p className="petit texte-doux">
          {s.questions.length} question{s.questions.length > 1 ? "s" : ""} · {total} points
          {ramassageAutomatique ? " · la copie est ramassée automatiquement à la fin du temps" : ""}
        </p>
        <div className="enonce">
          <MarkdownObsidian markdown={s.consigne} navigation={navigationSujet} />
        </div>
        {s.contexte && (
          <div className="document-sujet">
            <MarkdownObsidian markdown={s.contexte} navigation={navigationSujet} />
          </div>
        )}
        {s.graphique && <Graphique graphique={s.graphique} />}
      </section>

      {s.questions.map((q, i) => (
        <section key={i} className="carte bloc question-sujet">
          {q.partie && (i === 0 || s.questions[i - 1].partie !== q.partie) && (
            <div className="partie-epreuve">{titrePartie(q.partie)}</div>
          )}
          <div className="rangee entre">
            <strong>Question {i + 1}</strong>
            <span className="texte-doux petit">
              {q.points} point{q.points > 1 ? "s" : ""}
            </span>
          </div>
          <div className="enonce">
            <MarkdownObsidian markdown={q.enonce} navigation={navigationSujet} />
          </div>
          <ChampReponse q={q} numero={i + 1} valeur={reponses[i]} onChange={(v) => modifier(i, v)} />
        </section>
      ))}

      <div className="rangee copie-pied">
        {confirmer && sansReponse > 0 ? (
          <button className="bouton bouton-principal" disabled={envoi} onClick={rendre}>
            <Send size={15} aria-hidden /> Rendre quand même ({sansReponse} sans réponse)
          </button>
        ) : (
          <button
            className="bouton bouton-principal"
            disabled={envoi}
            onClick={() => (sansReponse > 0 ? setConfirmer(true) : rendre())}
          >
            <Send size={15} aria-hidden /> {envoi ? "Correction…" : "Rendre la copie"}
          </button>
        )}
        <button className="bouton" disabled={envoi} onClick={onAbandonner}>
          <DoorOpen size={15} aria-hidden /> Quitter (brouillon gardé)
        </button>
      </div>
    </div>
  );
}
