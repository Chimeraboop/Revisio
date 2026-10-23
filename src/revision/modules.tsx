// Un « module » par genre de carte : comment l'afficher, comment y répondre, comment la corriger.
// La séance (SeanceVue) ne connaît que cette interface commune.
import type { ReactNode } from "react";
import type { CarteSeance, Reglages } from "../lib/api";
import { MarkdownObsidian } from "../markdown/rendu";
import { schema } from "./ModuleSchema";
import { ordre } from "./ModuleOrdre";
import { ouverte, qcm } from "./ModulesIa";
import { TexteMaths } from "./symboles";
import { verifier, type Resultat } from "./verification";

export interface ProprietesQuestion<R> {
  carte: CarteSeance;
  mode: "retourner" | "repondre";
  reponse: R;
  setReponse: (r: R) => void;
  /** La correction est affichée : plus de saisie possible. */
  corrige: boolean;
  /** Entrée dans le champ de réponse. */
  onValider: () => void;
}

export interface ModuleCarte<R> {
  /** « retourner » : la carte se retourne et on s'auto-note ; « repondre » : réponse vérifiée par l'app. */
  mode: (carte: CarteSeance, reglages: Reglages) => "retourner" | "repondre";
  consigne: (carte: CarteSeance) => string;
  reponseInitiale: (carte: CarteSeance) => R;
  Question: (p: ProprietesQuestion<R>) => ReactNode;
  /** Bonne réponse affichée après la tentative. */
  Correction: (p: { carte: CarteSeance; reponse: R }) => ReactNode;
  evaluer: (carte: CarteSeance, reponse: R) => Resultat;
  /** Ce qui est enregistré dans l'historique / le carnet d'erreurs. */
  enTexte: (reponse: R) => string;
}

export function ChampTexte({ reponse, setReponse, corrige, onValider }: ProprietesQuestion<string>) {
  return (
    <input
      className="champ champ-reponse"
      autoFocus
      disabled={corrige}
      value={reponse}
      placeholder="Ta réponse, puis Entrée"
      onChange={(e) => setReponse(e.target.value)}
      onKeyDown={(e) => {
        if (e.key === "Enter") {
          e.preventDefault();
          onValider();
        }
      }}
    />
  );
}

const vocabulaire: ModuleCarte<string> = {
  mode: (carte, reglages) =>
    (carte.donnees.sens === "vers_fr" ? reglages.vocabVersFrancais : reglages.vocabDepuisFrancais) === "taper"
      ? "repondre"
      : "retourner",
  consigne: (carte) => {
    const langue = String(carte.donnees.nomLangue ?? "Langue étrangère");
    return carte.donnees.sens === "vers_fr" ? `${langue} → français` : `Français → ${langue.toLowerCase()}`;
  },
  reponseInitiale: () => "",
  Question: (p) => (
    <div className="question-vocab">
      <div className="terme">
        <TexteMaths texte={p.carte.question} />
      </div>
      {p.mode === "repondre" && <ChampTexte {...p} />}
    </div>
  ),
  Correction: ({ carte }) => (
    <div className="terme terme-reponse">
      <TexteMaths texte={carte.reponse} />
    </div>
  ),
  evaluer: (carte, reponse) => verifier(reponse, carte.reponse),
  enTexte: (r) => r,
};

/** Le trou est écrit `==……==` dans la question (affiché surligné). */
const TROU = "==……==";

/**
 * Question d'un texte à trous avec la réponse surlignée. Une balise <mark> plutôt que `==…==` :
 * la réponse peut contenir « = », du gras ou une formule $…$ sans casser le surlignage.
 */
export function remplirTrou(question: string, reponse: string): string {
  const sure = reponse.replace(/&/g, "&amp;").replace(/</g, "&lt;").replace(/>/g, "&gt;");
  return question.replace(TROU, `<mark>${sure}</mark>`);
}
const sansNavigation = (carte: CarteSeance) => ({ source: carte.noteChemin, ouvrirNote: () => {} });

const trou: ModuleCarte<string> = {
  mode: () => "repondre",
  consigne: (carte) => `Complète le texte${carte.section ? ` — ${carte.section}` : ""}`,
  reponseInitiale: () => "",
  Question: (p) => (
    <div className="question-trou">
      <MarkdownObsidian markdown={p.carte.question} navigation={sansNavigation(p.carte)} />
      <ChampTexte {...p} />
    </div>
  ),
  Correction: ({ carte }) => (
    <div className="correction-trou">
      <MarkdownObsidian markdown={remplirTrou(carte.question, carte.reponse)} navigation={sansNavigation(carte)} />
    </div>
  ),
  evaluer: (carte, reponse) => verifier(reponse, carte.reponse),
  enTexte: (r) => r,
};

export const MODULES: Record<string, ModuleCarte<unknown>> = {
  vocabulaire: vocabulaire as ModuleCarte<unknown>,
  trou: trou as ModuleCarte<unknown>,
  schema: schema as ModuleCarte<unknown>,
  ordre: ordre as ModuleCarte<unknown>,
  qcm: qcm as ModuleCarte<unknown>,
  ouverte: ouverte as ModuleCarte<unknown>,
};

export const NOMS_GENRES: Record<string, string> = {
  vocabulaire: "Vocabulaire",
  trou: "Textes à trous",
  schema: "Schémas muets",
  ordre: "Remise dans l'ordre",
  qcm: "QCM (IA)",
  ouverte: "Questions ouvertes (IA)",
  examen: "Exercice ou examen",
};
