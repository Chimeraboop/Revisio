// Remise dans l'ordre : les éléments sont mélangés, on les remonte / descend.
import { ArrowDown, ArrowUp } from "lucide-react";
import { MarkdownObsidian } from "../markdown/rendu";
import type { CarteSeance } from "../lib/api";
import type { ModuleCarte, ProprietesQuestion } from "./modules";
import type { Resultat } from "./verification";

export interface ReponseOrdre {
  /** Indices (dans l'ordre correct) des éléments, dans l'ordre proposé par l'élève. */
  ordre: number[];
}

function elements(carte: CarteSeance): string[] {
  return Array.isArray(carte.donnees.elements) ? (carte.donnees.elements as string[]) : [];
}

function dates(carte: CarteSeance): string[] {
  return Array.isArray(carte.donnees.dates) ? (carte.donnees.dates as string[]) : [];
}

/** Mélange qui garantit que la liste n'est pas déjà dans le bon ordre. */
export function melangerOrdre(n: number, aleatoire = Math.random): number[] {
  const indices = Array.from({ length: n }, (_, i) => i);
  if (n < 2) return indices;
  do {
    for (let i = n - 1; i > 0; i--) {
      const j = Math.floor(aleatoire() * (i + 1));
      [indices[i], indices[j]] = [indices[j], indices[i]];
    }
  } while (indices.every((v, i) => v === i));
  return indices;
}

/** Évaluation : nombre d'éléments à la bonne place, et paires voisines dans le bon ordre. */
export function evaluerOrdre(ordre: number[]): Resultat {
  const n = ordre.length;
  if (!n) return { verdict: "faux" };
  const bienPlaces = ordre.filter((v, i) => v === i).length;
  const pairesJustes = ordre.slice(1).filter((v, i) => v > ordre[i]).length;
  const message = `${bienPlaces} / ${n} à la bonne place.`;
  if (bienPlaces === n) return { verdict: "juste", message };
  if (pairesJustes / (n - 1) >= 0.75) return { verdict: "presque", message };
  return { verdict: "faux", message };
}

const sansNavigation = (carte: CarteSeance) => ({ source: carte.noteChemin, ouvrirNote: () => {} });

function Question({ carte, reponse, setReponse, corrige }: ProprietesQuestion<ReponseOrdre>) {
  const liste = elements(carte);
  const deplacer = (position: number, decalage: number) => {
    const cible = position + decalage;
    if (cible < 0 || cible >= reponse.ordre.length) return;
    const ordre = [...reponse.ordre];
    [ordre[position], ordre[cible]] = [ordre[cible], ordre[position]];
    setReponse({ ordre });
  };
  return (
    <div className="question-ordre">
      <div className="terme-petit">{carte.section}</div>
      <ol className="liste-ordre">
        {reponse.ordre.map((indice, position) => (
          <li key={indice} className="element-ordre">
            <span className="numero">{position + 1}</span>
            <div className="element-texte">
              <MarkdownObsidian markdown={liste[indice]} navigation={sansNavigation(carte)} />
            </div>
            {!corrige && (
              <span className="fleches">
                <button className="bouton" onClick={() => deplacer(position, -1)} disabled={position === 0} title="Monter">
                  <ArrowUp size={15} aria-label="Monter" />
                </button>
                <button
                  className="bouton"
                  onClick={() => deplacer(position, 1)}
                  disabled={position === reponse.ordre.length - 1}
                  title="Descendre"
                >
                  <ArrowDown size={15} aria-label="Descendre" />
                </button>
              </span>
            )}
          </li>
        ))}
      </ol>
    </div>
  );
}

function Correction({ carte, reponse }: { carte: CarteSeance; reponse: ReponseOrdre }) {
  const liste = elements(carte);
  const lesDates = dates(carte);
  return (
    <ol className="liste-ordre correction-ordre">
      {liste.map((texte, i) => {
        const placeParEleve = reponse.ordre.indexOf(i);
        return (
          <li key={i} className="element-ordre">
            <span className={`numero ${placeParEleve === i ? "numero-juste" : "numero-faux"}`}>{i + 1}</span>
            <div className="element-texte">
              <MarkdownObsidian
                markdown={(lesDates[i] ? `**${lesDates[i]}** : ` : "") + texte}
                navigation={sansNavigation(carte)}
              />
            </div>
            {placeParEleve !== i && <span className="texte-doux petit">tu l'avais mis en {placeParEleve + 1}</span>}
          </li>
        );
      })}
    </ol>
  );
}

export const ordre: ModuleCarte<ReponseOrdre> = {
  mode: () => "repondre",
  consigne: (carte) => carte.question,
  reponseInitiale: (carte) => ({ ordre: melangerOrdre(elements(carte).length) }),
  Question,
  Correction,
  evaluer: (_carte, r) => evaluerOrdre(r.ordre),
  enTexte: (r) => r.ordre.map((i) => i + 1).join(", "),
};
