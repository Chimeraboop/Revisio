// Calendrier mensuel du planning : évaluations, chapitres à réviser chaque jour, activité passée.
// Un clic sur un jour le choisit (son détail s'affiche sous le calendrier).
import { useEffect, useMemo, useState } from "react";
import { ChevronLeft, ChevronRight, GraduationCap } from "lucide-react";
import type { Evaluation, JourActivite } from "../lib/api";
import { abregerChapitre, couleurMatiere, dansLeMois, decalerMois, grilleMois, moisDe, nomMois, type Mois } from "./grilleCalendrier";
import type { JourPlan } from "./planning";

interface Props {
  plan: JourPlan[];
  evaluations: Evaluation[];
  activite: JourActivite[];
  aujourdhui: string;
  choisi: string;
  onChoisir: (jour: string) => void;
}

const JOURS = ["Lun", "Mar", "Mer", "Jeu", "Ven", "Sam", "Dim"];
const nomChapitre = (c: string | null) => c ?? "Notes générales";

/** Ce qu'on écrit dans une case : une étiquette par chapitre (ou une seule pour la révision générale). */
export function etiquettesDuJour(jour: JourPlan | undefined): { cle: string; texte: string; matiere: string }[] {
  if (!jour) return [];
  const vues = new Map<string, { cle: string; texte: string; matiere: string }>();
  for (const e of jour.elements) {
    const matiere = e.evaluation.matiere;
    if (e.nature === "revision_generale") {
      vues.set(`rg|${e.evaluation.id}`, { cle: `rg|${e.evaluation.id}`, texte: `Révision générale · ${matiere}`, matiere });
    } else if (e.nature === "jour_j") {
      vues.set(`jj|${e.evaluation.id}`, { cle: `jj|${e.evaluation.id}`, texte: "Dernier coup d'œil", matiere });
    } else {
      const cle = `${matiere}|${e.chapitre ?? ""}`;
      vues.set(cle, { cle, texte: abregerChapitre(nomChapitre(e.chapitre)), matiere });
    }
  }
  return [...vues.values()];
}

export default function Calendrier({ plan, evaluations, activite, aujourdhui, choisi, onChoisir }: Props) {
  const [mois, setMois] = useState<Mois>(() => moisDe(choisi));
  // Choisir un jour d'un autre mois (ex. depuis la liste) affiche ce mois.
  useEffect(() => setMois(moisDe(choisi)), [choisi]);

  const parJour = useMemo(() => new Map(plan.map((j) => [j.jour, j])), [plan]);
  const evalsParJour = useMemo(() => {
    const m = new Map<string, Evaluation[]>();
    for (const ev of evaluations) m.set(ev.jour, [...(m.get(ev.jour) ?? []), ev]);
    return m;
  }, [evaluations]);
  const reponses = useMemo(() => new Map(activite.map((j) => [j.jour, j.revisions])), [activite]);
  const jours = grilleMois(mois);

  return (
    <div className="calendrier">
      <div className="calendrier-entete">
        <button className="bouton bouton-icone" onClick={() => setMois((m) => decalerMois(m, -1))} aria-label="Mois précédent">
          <ChevronLeft size={17} aria-hidden />
        </button>
        <h2 className="calendrier-mois" aria-live="polite">
          {nomMois(mois)}
        </h2>
        <button className="bouton bouton-icone" onClick={() => setMois((m) => decalerMois(m, 1))} aria-label="Mois suivant">
          <ChevronRight size={17} aria-hidden />
        </button>
        <button
          className="bouton"
          onClick={() => {
            setMois(moisDe(aujourdhui));
            onChoisir(aujourdhui);
          }}
        >
          Aujourd'hui
        </button>
        <span className="espace" />
        <span className="legende-calendrier texte-doux petit">
          <span>
            <GraduationCap size={13} aria-hidden /> évaluation
          </span>
          <span>
            <span className="tache-exemple" aria-hidden /> à réviser
          </span>
          <span>
            <span className="activite-exemple" aria-hidden /> réponses faites
          </span>
        </span>
      </div>

      <div className="calendrier-grille" role="grid" aria-label={`Planning de ${nomMois(mois)}`}>
        {JOURS.map((j) => (
          <div key={j} className="calendrier-jour-semaine" role="columnheader">
            {j}
          </div>
        ))}
        {jours.map((jour, i) => {
          const evs = evalsParJour.get(jour) ?? [];
          const taches = etiquettesDuJour(parJour.get(jour));
          const faites = jour <= aujourdhui ? (reponses.get(jour) ?? 0) : 0;
          const classes = [
            "case-jour",
            dansLeMois(jour, mois) ? "" : "hors-mois",
            jour === aujourdhui ? "aujourdhui" : "",
            jour === choisi ? "choisie" : "",
            jour < aujourdhui ? "passee" : "",
            evs.length ? "avec-evaluation" : "",
          ]
            .filter(Boolean)
            .join(" ");
          return (
            <button
              key={jour}
              role="gridcell"
              aria-selected={jour === choisi}
              className={classes}
              style={{ "--i": i } as React.CSSProperties}
              onClick={() => onChoisir(jour)}
              title={[...evs.map((e) => e.titre), ...taches.map((t) => t.texte)].join(" · ") || undefined}
            >
              <span className="case-entete">
                <span className="numero-jour">{Number(jour.slice(8))}</span>
                {faites > 0 && (
                  <span className="activite-jour" title={`${faites} réponse${faites > 1 ? "s" : ""}`}>
                    {faites}
                  </span>
                )}
              </span>
              {evs.map((ev) => (
                <span key={ev.id} className="eval-jour" style={{ "--couleur": couleurMatiere(ev.matiere) } as React.CSSProperties}>
                  <GraduationCap size={12} aria-hidden /> {ev.titre}
                </span>
              ))}
              {taches.slice(0, evs.length ? 2 : 3).map((t) => (
                <span key={t.cle} className="tache-jour" style={{ "--couleur": couleurMatiere(t.matiere) } as React.CSSProperties}>
                  {t.texte}
                </span>
              ))}
              {taches.length > (evs.length ? 2 : 3) && (
                <span className="texte-doux plus-taches">+{taches.length - (evs.length ? 2 : 3)}</span>
              )}
            </button>
          );
        })}
      </div>
    </div>
  );
}
