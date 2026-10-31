// Graphique d'un sujet (analyse de graphique) : courbes tracées à partir des données du sujet,
// avec une grille fine pour lire les valeurs. Les valeurs exactes (infobulle au survol) ne sont
// montrées qu'à la correction : pendant l'épreuve, il faut les lire sur le graphique.
import { useState } from "react";
import type { GraphiqueSujet } from "../lib/api";

const LARGEUR = 640;
const HAUTEUR = 360;
const MARGE = { haut: 16, droite: 20, bas: 52, gauche: 64 };

/** Graduations « rondes » (1, 2, 5 × 10^n) couvrant [min, max]. */
export function graduations(min: number, max: number, nombre = 8): number[] {
  if (min === max) {
    min -= 1;
    max += 1;
  }
  const brut = (max - min) / nombre;
  const puissance = 10 ** Math.floor(Math.log10(brut));
  const pas = [1, 2, 5, 10].map((m) => m * puissance).find((p) => p >= brut) ?? brut;
  const debut = Math.floor(min / pas) * pas;
  const fin = Math.ceil(max / pas) * pas;
  const liste: number[] = [];
  for (let v = debut; v <= fin + pas / 2; v += pas) liste.push(Number(v.toPrecision(12)));
  return liste;
}

const format = new Intl.NumberFormat("fr-FR", { maximumFractionDigits: 4 });

export default function Graphique({
  graphique,
  valeursAuSurvol = false,
  lisse = false,
}: {
  graphique: GraphiqueSujet;
  valeursAuSurvol?: boolean;
  /** Courbe d'une fonction (beaucoup de points calculés) : les points ne sont pas dessinés, seulement
   *  repérables au survol. */
  lisse?: boolean;
}) {
  const [survol, setSurvol] = useState<{ x: number; y: number; serie: string; px: number; py: number } | null>(null);
  const points = graphique.series.flatMap((s) => s.points);
  if (points.length === 0) return null;

  const xs = points.map((p) => p[0]);
  const ys = points.map((p) => p[1]);
  const gx = graduations(Math.min(...xs), Math.max(...xs));
  // L'axe vertical part de 0 quand les valeurs sont positives (lecture plus honnête).
  const gy = graduations(Math.min(0, ...ys), Math.max(...ys));
  const [x0, x1] = [gx[0], gx[gx.length - 1]];
  const [y0, y1] = [gy[0], gy[gy.length - 1]];
  const largeur = LARGEUR - MARGE.gauche - MARGE.droite;
  const hauteur = HAUTEUR - MARGE.haut - MARGE.bas;
  const px = (x: number) => MARGE.gauche + ((x - x0) / (x1 - x0 || 1)) * largeur;
  const py = (y: number) => MARGE.haut + hauteur - ((y - y0) / (y1 - y0 || 1)) * hauteur;

  return (
    <figure className="graphique graphique-sujet">
      <figcaption>
        <strong>{graphique.titre}</strong>
        {graphique.series.length > 1 && (
          <span className="legende">
            {graphique.series.map((s, i) => (
              <span key={s.nom} className={`legende-serie serie-${(i % 2) + 1}`}>
                <span className="pastille" /> {s.nom}
              </span>
            ))}
          </span>
        )}
      </figcaption>
      <svg viewBox={`0 0 ${LARGEUR} ${HAUTEUR}`} role="img" aria-label={graphique.titre}>
        {gy.map((v) => (
          <g key={`y${v}`}>
            <line className="grille" x1={MARGE.gauche} x2={LARGEUR - MARGE.droite} y1={py(v)} y2={py(v)} />
            <text className="graduation" x={MARGE.gauche - 8} y={py(v) + 4} textAnchor="end">
              {format.format(v)}
            </text>
          </g>
        ))}
        {gx.map((v) => (
          <g key={`x${v}`}>
            <line className="grille" x1={px(v)} x2={px(v)} y1={MARGE.haut} y2={MARGE.haut + hauteur} />
            <text className="graduation" x={px(v)} y={MARGE.haut + hauteur + 18} textAnchor="middle">
              {format.format(v)}
            </text>
          </g>
        ))}
        <line className="axe" x1={MARGE.gauche} x2={LARGEUR - MARGE.droite} y1={py(y0)} y2={py(y0)} />
        <line className="axe" x1={MARGE.gauche} x2={MARGE.gauche} y1={MARGE.haut} y2={MARGE.haut + hauteur} />
        <text className="titre-axe" x={MARGE.gauche + largeur / 2} y={HAUTEUR - 8} textAnchor="middle">
          {graphique.axe_x}
        </text>
        <text
          className="titre-axe"
          transform={`translate(14 ${MARGE.haut + hauteur / 2}) rotate(-90)`}
          textAnchor="middle"
        >
          {graphique.axe_y}
        </text>
        {graphique.series.map((s, i) => {
          const tries = [...s.points].sort((a, b) => a[0] - b[0]);
          return (
            <g key={s.nom} className={`serie-${(i % 2) + 1}`}>
              <polyline className="courbe" points={tries.map(([x, y]) => `${px(x)},${py(y)}`).join(" ")} />
              {tries.map(([x, y]) => (
                <circle
                  key={`${x}-${y}`}
                  className={lisse && !(survol?.px === px(x) && survol?.py === py(y)) ? "point point-cache" : "point"}
                  cx={px(x)}
                  cy={py(y)}
                  r={survol?.px === px(x) && survol?.py === py(y) ? 6 : 4}
                  onMouseEnter={() => valeursAuSurvol && setSurvol({ x, y, serie: s.nom, px: px(x), py: py(y) })}
                  onMouseLeave={() => setSurvol(null)}
                />
              ))}
            </g>
          );
        })}
      </svg>
      {survol && (
        <div
          className="infobulle"
          style={{ left: `${(survol.px / LARGEUR) * 100}%`, top: `${(survol.py / HAUTEUR) * 100}%`, marginTop: -44 }}
        >
          {graphique.series.length > 1 && <>{survol.serie} : </>}
          {format.format(survol.x)} ; {format.format(survol.y)}
        </div>
      )}
    </figure>
  );
}
