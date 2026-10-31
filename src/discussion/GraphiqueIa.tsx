// Graphique proposé par l'IA (bloc ```graphique, JSON) : courbes données, fonctions (points calculés
// par Révisio, pas par l'IA) ou barres. Les courbes réutilisent le traceur des exercices type bac.
import { useEffect, useState } from "react";
import { api, messageErreur, type GraphiqueSujet } from "../lib/api";
import Graphique, { graduations } from "../exercices/Graphique";

export interface SpecGraphique {
  titre?: string;
  type?: "fonction" | "courbes" | "barres";
  axeX?: string;
  axeY?: string;
  xmin?: number;
  xmax?: number;
  fonctions?: { nom?: string; expression: string }[];
  series?: { nom?: string; points?: [number, number][]; valeurs?: number[] }[];
  categories?: string[];
}

/** Lit le JSON du bloc ; `null` s'il est illisible ou d'un type inconnu. */
export function lireSpec(contenu: string): SpecGraphique | null {
  try {
    const s = JSON.parse(contenu) as SpecGraphique;
    if (!s || typeof s !== "object") return null;
    if (s.type === "fonction" && Array.isArray(s.fonctions) && s.fonctions.length) return s;
    if (s.type === "courbes" && Array.isArray(s.series) && s.series.some((x) => x.points?.length)) return s;
    if (s.type === "barres" && Array.isArray(s.categories) && Array.isArray(s.series)) return s;
    return null;
  } catch {
    return null;
  }
}

export default function GraphiqueIa({ contenu }: { contenu: string }) {
  const spec = lireSpec(contenu);
  const [courbes, setCourbes] = useState<GraphiqueSujet | null>(null);
  const [erreur, setErreur] = useState<string | null>(null);

  useEffect(() => {
    setCourbes(null);
    setErreur(null);
    if (!spec) return;
    // Le titre est déjà affiché au-dessus du visuel (proposition ou création).
    const base = { titre: "", axe_x: spec.axeX ?? "x", axe_y: spec.axeY ?? "y" };
    if (spec.type === "courbes") {
      setCourbes({
        ...base,
        series: (spec.series ?? []).map((s, i) => ({ nom: s.nom ?? `Série ${i + 1}`, points: s.points ?? [] })),
      });
    } else if (spec.type === "fonction") {
      const [xmin, xmax] = [spec.xmin ?? -5, spec.xmax ?? 5];
      Promise.all(spec.fonctions!.map((f) => api.pointsFonction(f.expression, xmin, xmax)))
        .then((points) =>
          setCourbes({ ...base, series: spec.fonctions!.map((f, i) => ({ nom: f.nom ?? f.expression, points: points[i] })) }),
        )
        .catch((e) => setErreur(messageErreur(e)));
    }
  }, [contenu]);

  if (!spec) return <p className="message-erreur">Ce graphique est illisible (données mal écrites par l'IA).</p>;
  if (erreur) return <p className="message-erreur">Graphique impossible à tracer : {erreur}</p>;
  if (spec.type === "barres") return <Barres spec={spec} />;
  if (!courbes) return <p className="progression-texte">Calcul des points…</p>;
  return (
    <div className="graphique-ia">
      <Graphique graphique={courbes} valeursAuSurvol lisse={spec.type === "fonction"} />
    </div>
  );
}

const L = 640;
const H = 340;
const M = { haut: 16, droite: 16, bas: 56, gauche: 56 };
const format = new Intl.NumberFormat("fr-FR", { maximumFractionDigits: 3 });

/** Diagramme en barres groupées (une couleur par série). */
function Barres({ spec }: { spec: SpecGraphique }) {
  const categories = spec.categories ?? [];
  const series = (spec.series ?? []).filter((s) => Array.isArray(s.valeurs));
  const valeurs = series.flatMap((s) => s.valeurs!.filter((v) => Number.isFinite(v)));
  if (!categories.length || !valeurs.length) return <p className="message-erreur">Graphique vide.</p>;
  const grad = graduations(Math.min(0, ...valeurs), Math.max(0, ...valeurs), 6);
  const [ymin, ymax] = [grad[0], grad[grad.length - 1]];
  const y = (v: number) => M.haut + ((ymax - v) / (ymax - ymin)) * (H - M.haut - M.bas);
  const largeurGroupe = (L - M.gauche - M.droite) / categories.length;
  const largeurBarre = Math.min(48, (largeurGroupe * 0.8) / Math.max(1, series.length));
  return (
    <figure className="graphique graphique-sujet graphique-ia">
      <svg viewBox={`0 0 ${L} ${H}`} role="img" aria-label={spec.titre ?? "Diagramme en barres"}>
        {grad.map((g) => (
          <g key={g}>
            <line x1={M.gauche} x2={L - M.droite} y1={y(g)} y2={y(g)} className="grille" />
            <text x={M.gauche - 8} y={y(g)} className="graduation" textAnchor="end" dominantBaseline="middle">
              {format.format(g)}
            </text>
          </g>
        ))}
        {categories.map((c, i) => {
          const x0 = M.gauche + i * largeurGroupe + (largeurGroupe - largeurBarre * series.length) / 2;
          return (
            <g key={c}>
              {series.map((s, k) => {
                const v = s.valeurs![i];
                if (!Number.isFinite(v)) return null;
                const [haut, bas] = [y(Math.max(0, v)), y(Math.min(0, v))];
                return (
                  <rect
                    key={k}
                    x={x0 + k * largeurBarre}
                    y={haut}
                    width={largeurBarre - 3}
                    height={Math.max(1, bas - haut)}
                    rx={3}
                    className={`barre barre-${k % 4}`}
                  >
                    <title>{`${s.nom ?? `Série ${k + 1}`} · ${c} : ${format.format(v)}`}</title>
                  </rect>
                );
              })}
              <text x={M.gauche + (i + 0.5) * largeurGroupe} y={H - M.bas + 18} className="graduation" textAnchor="middle">
                {c.length > 14 ? c.slice(0, 13) + "…" : c}
              </text>
            </g>
          );
        })}
        <line x1={M.gauche} x2={L - M.droite} y1={y(0)} y2={y(0)} className="axe" />
        {spec.axeY && (
          <text x={14} y={H / 2} className="graduation" textAnchor="middle" transform={`rotate(-90 14 ${H / 2})`}>
            {spec.axeY}
          </text>
        )}
      </svg>
      {series.length > 1 && (
        <div className="legende-barres petit">
          {series.map((s, k) => (
            <span key={k}>
              <span className={`pastille barre-${k % 4}`} /> {s.nom ?? `Série ${k + 1}`}
            </span>
          ))}
        </div>
      )}
    </figure>
  );
}
