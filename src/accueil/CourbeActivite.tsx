// Activité des derniers jours : courbe du nombre de réponses par jour, dans un bloc sombre.
// La ligne se dessine à l'affichage ; au survol d'un jour, une bulle donne le détail.
import { useState } from "react";
import type { JourActivite } from "../lib/api";
import { formaterDuree } from "../revision/stats";

const L = 760;
const H = 220;
const MARGE = { gauche: 28, droite: 28, haut: 36, bas: 34 };

const formatJour = new Intl.DateTimeFormat("fr-FR", { weekday: "short", day: "numeric" });
const formatLong = new Intl.DateTimeFormat("fr-FR", { weekday: "long", day: "numeric", month: "long" });

function date(jour: string) {
  const [a, m, j] = jour.split("-").map(Number);
  return new Date(a, m - 1, j);
}

/** Coordonnées des points (exportée pour les tests). */
export function points(jours: JourActivite[]): { x: number; y: number }[] {
  const max = Math.max(5, ...jours.map((j) => j.revisions));
  const largeur = L - MARGE.gauche - MARGE.droite;
  const hauteur = H - MARGE.haut - MARGE.bas;
  return jours.map((j, i) => ({
    x: MARGE.gauche + (jours.length > 1 ? (i * largeur) / (jours.length - 1) : largeur / 2),
    y: MARGE.haut + hauteur - (j.revisions / max) * hauteur,
  }));
}

export default function CourbeActivite({ jours }: { jours: JourActivite[] }) {
  const [survol, setSurvol] = useState<number | null>(null);
  if (jours.length === 0) return null;
  const pts = points(jours);
  const ligne = pts.map((p, i) => `${i ? "L" : "M"}${p.x.toFixed(1)},${p.y.toFixed(1)}`).join(" ");
  const bas = H - MARGE.bas;
  const aire = `${ligne} L${pts.at(-1)!.x.toFixed(1)},${bas} L${pts[0].x.toFixed(1)},${bas} Z`;
  const pas = pts.length > 1 ? pts[1].x - pts[0].x : L;
  const j = survol !== null ? jours[survol] : null;

  return (
    <div className="courbe" onMouseLeave={() => setSurvol(null)}>
      <svg viewBox={`0 0 ${L} ${H}`} role="img" aria-label="Nombre de réponses par jour">
        <defs>
          <linearGradient id="degrade-courbe" x1="0" x2="0" y1="0" y2="1">
            <stop offset="0%" stopColor="var(--trait-graphique)" stopOpacity="0.28" />
            <stop offset="100%" stopColor="var(--trait-graphique)" stopOpacity="0" />
          </linearGradient>
        </defs>
        <line x1={MARGE.gauche} x2={L - MARGE.droite} y1={bas} y2={bas} className="courbe-base" />
        <path d={aire} fill="url(#degrade-courbe)" className="courbe-aire" />
        <path d={ligne} pathLength={1} className="courbe-ligne" />
        {survol !== null && <line x1={pts[survol].x} x2={pts[survol].x} y1={MARGE.haut - 10} y2={bas} className="courbe-repere" />}
        {pts.map((p, i) => (
          <g key={jours[i].jour} className="courbe-point" style={{ "--i": i } as React.CSSProperties}>
            <circle cx={p.x} cy={p.y} r={survol === i ? 6 : 4} />
            {jours[i].revisions > 0 && (
              <text x={p.x} y={p.y - 12} textAnchor="middle" className="courbe-valeur">
                {jours[i].revisions}
              </text>
            )}
            <text x={p.x} y={H - 10} textAnchor="middle" className="courbe-jour">
              {formatJour.format(date(jours[i].jour)).replace(".", "")}
            </text>
            {/* Zone de survol : toute la colonne du jour */}
            <rect x={p.x - pas / 2} y={0} width={pas} height={H} fill="transparent" onMouseEnter={() => setSurvol(i)} />
          </g>
        ))}
      </svg>
      {j && survol !== null && (
        <div
          className="bulle-courbe"
          style={{ left: `${(pts[survol].x / L) * 100}%`, top: `${(pts[survol].y / H) * 100}%` }}
          role="status"
        >
          <strong>{formatLong.format(date(j.jour))}</strong>
          <span>
            {j.revisions} réponse{j.revisions > 1 ? "s" : ""}
            {j.revisions > 0 && ` · ${Math.round((j.justes / j.revisions) * 100)} % justes`}
          </span>
          {j.dureeMs > 0 && <span>{formaterDuree(j.dureeMs)} de révision</span>}
        </div>
      )}
    </div>
  );
}
