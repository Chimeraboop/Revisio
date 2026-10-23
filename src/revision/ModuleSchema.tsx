// Schéma muet : le dessin s'affiche avec ses légendes remplacées par des numéros.
import { useEffect, useMemo, useRef, useState } from "react";
import { Check, Equal, X } from "lucide-react";
import type { CarteSeance } from "../lib/api";
import { chargerScene } from "../excalidraw/chargement";
import Dessin from "../excalidraw/Dessin";
import { masquerLegendes, type Scene, type Trou } from "../excalidraw/scene";
import type { ModuleCarte, ProprietesQuestion } from "./modules";
import { TexteMaths } from "./symboles";
import { verifier } from "./verification";

export interface ReponseSchema {
  valeurs: Record<number, string>;
  trous: Trou[];
  /** La banque de mots a été affichée : la note est plafonnée à « Difficile ». */
  aide: boolean;
}

function legendes(carte: CarteSeance): string[] {
  return Array.isArray(carte.donnees.legendes) ? (carte.donnees.legendes as string[]) : [];
}

function melanger<T>(l: T[]): T[] {
  return [...l].sort(() => Math.random() - 0.5);
}

function Question({ carte, reponse, setReponse, corrige, onValider }: ProprietesQuestion<ReponseSchema>) {
  const [scene, setScene] = useState<Scene | null | undefined>(undefined);
  const champs = useRef<(HTMLInputElement | null)[]>([]);
  const banque = useMemo(() => melanger(reponse.trous.map((t) => t.attendu)), [reponse.trous]);

  useEffect(() => {
    let actif = true;
    chargerScene(carte.noteChemin).then((s) => {
      if (!actif) return;
      if (!s) {
        setScene(null);
        return;
      }
      const muet = masquerLegendes(s, legendes(carte));
      setScene(muet.scene);
      setReponse({ ...reponse, trous: muet.trous });
    });
    return () => {
      actif = false;
    };
    // Chargé une seule fois par carte.
    // eslint-disable-next-line react-hooks/exhaustive-deps
  }, [carte.id]);

  useEffect(() => {
    if (reponse.trous.length && !corrige) champs.current[0]?.focus();
  }, [reponse.trous.length, corrige]);

  if (scene === undefined) return <p className="texte-doux">Chargement du schéma…</p>;
  if (scene === null) return <p className="message-erreur">Ce dessin est illisible : signale la carte pour la retirer.</p>;

  return (
    <div className="question-schema">
      <div className="terme-petit">{carte.question}</div>
      <Dessin scene={scene} titre={carte.question} />
      <div className="champs-legendes">
        {reponse.trous.map((t, i) => (
          <label key={t.numero} className="champ-legende">
            <span className="numero">{t.numero}</span>
            <input
              ref={(el) => {
                champs.current[i] = el;
              }}
              className="champ"
              disabled={corrige}
              value={reponse.valeurs[t.numero] ?? ""}
              onChange={(e) => setReponse({ ...reponse, valeurs: { ...reponse.valeurs, [t.numero]: e.target.value } })}
              onKeyDown={(e) => {
                if (e.key !== "Enter") return;
                e.preventDefault();
                if (i + 1 < reponse.trous.length) champs.current[i + 1]?.focus();
                else onValider();
              }}
            />
          </label>
        ))}
      </div>
      {!corrige &&
        (reponse.aide ? (
          <div className="integration-legendes">
            {banque.map((l, i) => (
              <span key={i} className="etiquette">
                <TexteMaths texte={l} />
              </span>
            ))}
          </div>
        ) : (
          <button className="lien-discret" onClick={() => setReponse({ ...reponse, aide: true })}>
            Afficher la banque de mots (la note sera au mieux « Difficile »)
          </button>
        ))}
    </div>
  );
}

function Correction({ carte, reponse }: { carte: CarteSeance; reponse: ReponseSchema }) {
  const [scene, setScene] = useState<Scene | null>(null);
  useEffect(() => {
    chargerScene(carte.noteChemin).then(setScene);
  }, [carte.noteChemin]);
  return (
    <div className="correction-schema">
      <table className="tableau">
        <tbody>
          {reponse.trous.map((t) => {
            const r = verifier(reponse.valeurs[t.numero] ?? "", t.attendu);
            return (
              <tr key={t.numero}>
                <td className="numero">{t.numero}</td>
                <td className={`verdict-${r.verdict}`}>
                  {r.verdict === "juste" ? (
                    <Check size={16} strokeWidth={2.5} aria-label="juste" />
                  ) : r.verdict === "presque" ? (
                    <Equal size={16} strokeWidth={2.5} aria-label="presque" />
                  ) : (
                    <X size={16} strokeWidth={2.5} aria-label="faux" />
                  )}
                </td>
                <td>
                  {reponse.valeurs[t.numero] ? <TexteMaths texte={reponse.valeurs[t.numero]} /> : <span className="texte-doux">—</span>}
                </td>
                <td className="verdict-juste">
                  <TexteMaths texte={t.attendu} />
                </td>
              </tr>
            );
          })}
        </tbody>
      </table>
      {scene && (
        <details>
          <summary className="lien-discret">Revoir le schéma complet</summary>
          <Dessin scene={scene} titre={carte.question} />
        </details>
      )}
    </div>
  );
}

export const schema: ModuleCarte<ReponseSchema> = {
  mode: () => "repondre",
  consigne: () => "Schéma muet — retrouve les légendes",
  reponseInitiale: () => ({ valeurs: {}, trous: [], aide: false }),
  Question,
  Correction,
  evaluer: (_carte, r) => {
    if (!r.trous.length) return { verdict: "faux", message: "Schéma illisible." };
    const verdicts = r.trous.map((t) => verifier(r.valeurs[t.numero] ?? "", t.attendu).verdict);
    const justes = verdicts.filter((v) => v === "juste").length;
    const presque = verdicts.filter((v) => v === "presque").length;
    const part = (justes + presque) / verdicts.length;
    const message = `${justes} / ${verdicts.length} légendes justes${presque ? `, ${presque} presque` : ""}${
      r.aide ? " (avec la banque de mots)" : ""
    }.`;
    if (justes === verdicts.length && !r.aide) return { verdict: "juste", message };
    if (part >= 0.7) return { verdict: "presque", message };
    return { verdict: "faux", message };
  },
  enTexte: (r) => r.trous.map((t) => `${t.numero}: ${r.valeurs[t.numero] ?? ""}`).join(" | "),
};
