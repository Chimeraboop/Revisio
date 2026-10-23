// Choix des chapitres d'une séance ciblée : une case par matière (toute la matière) et, en dépliant,
// une case par chapitre, avec le nombre de cartes à revoir.
import { useState } from "react";
import { ChevronDown } from "lucide-react";
import type { ChapitreSeance, ComptesChapitre } from "../lib/api";

/** Nom affiché pour les cartes sans matière (le même que côté Rust). */
export const HORS_MATIERE = "Hors matière";

/** Clé unique d'un chapitre (matière et chapitre peuvent être absents). */
export const cleChapitreSeance = (c: ChapitreSeance) => JSON.stringify([c.matiere, c.chapitre]);

const nomMatiere = (m: string | null) => m ?? HORS_MATIERE;

/** Chapitres regroupés par matière, dans l'ordre reçu (déjà trié par Révisio). */
export function parMatiere(chapitres: ComptesChapitre[]): [string, ComptesChapitre[]][] {
  const groupes = new Map<string, ComptesChapitre[]>();
  for (const c of chapitres) {
    const m = nomMatiere(c.matiere);
    groupes.set(m, [...(groupes.get(m) ?? []), c]);
  }
  return [...groupes];
}

interface Props {
  chapitres: ComptesChapitre[];
  choisis: Set<string>;
  onChange: (choisis: Set<string>) => void;
}

export default function ChoixChapitres({ chapitres, choisis, onChange }: Props) {
  const [ouvertes, setOuvertes] = useState<Set<string>>(new Set());

  const basculer = (cles: string[], coche: boolean) => {
    const n = new Set(choisis);
    cles.forEach((k) => (coche ? n.add(k) : n.delete(k)));
    onChange(n);
  };

  return (
    <div className="choix-seance">
      {parMatiere(chapitres).map(([matiere, liste]) => {
        const cles = liste.map(cleChapitreSeance);
        const nb = cles.filter((k) => choisis.has(k)).length;
        const dues = liste.reduce((t, c) => t + c.dues, 0);
        const total = liste.reduce((t, c) => t + c.total, 0);
        const ouverte = ouvertes.has(matiere);
        return (
          <div key={matiere} className="choix-matiere">
            <div className="choix-matiere-ligne">
              <label className="case">
                <input
                  type="checkbox"
                  checked={nb === cles.length}
                  ref={(el) => {
                    if (el) el.indeterminate = nb > 0 && nb < cles.length;
                  }}
                  onChange={(e) => basculer(cles, e.target.checked)}
                />
                <strong>{matiere}</strong>
              </label>
              <span className="texte-doux petit">
                {dues} à revoir · {total} carte{total > 1 ? "s" : ""}
                {nb > 0 && nb < cles.length && ` · ${nb} chapitre${nb > 1 ? "s" : ""} sur ${cles.length}`}
              </span>
              <button
                className="lien-discret petit choix-deplier"
                aria-expanded={ouverte}
                onClick={() =>
                  setOuvertes((o) => {
                    const n = new Set(o);
                    if (ouverte) n.delete(matiere);
                    else n.add(matiere);
                    return n;
                  })
                }
              >
                {liste.length} chapitre{liste.length > 1 ? "s" : ""}
                <ChevronDown size={14} className={"icone chevron" + (ouverte ? " ouvert" : "")} aria-hidden />
              </button>
            </div>
            {ouverte && (
              <div className="choix-chapitres-liste">
                {liste.map((c) => {
                  const k = cleChapitreSeance(c);
                  return (
                    <label key={k} className="case">
                      <input type="checkbox" checked={choisis.has(k)} onChange={(e) => basculer([k], e.target.checked)} />
                      <span>{c.chapitre ?? "Notes générales"}</span>
                      <span className="texte-doux petit">
                        {c.dues} à revoir · {c.total} carte{c.total > 1 ? "s" : ""}
                      </span>
                    </label>
                  );
                })}
              </div>
            )}
          </div>
        );
      })}
    </div>
  );
}
