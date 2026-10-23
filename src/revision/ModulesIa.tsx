// Cartes générées par l'IA : QCM et questions ouvertes. Elles s'utilisent hors ligne
// comme les autres cartes ; la correction montre la phrase de ta note qui justifie la réponse.
import { useEffect, useState } from "react";
import { Check, Sparkles, X } from "lucide-react";
import { chargerScene } from "../excalidraw/chargement";
import Dessin from "../excalidraw/Dessin";
import type { Scene } from "../excalidraw/scene";
import { api, type CarteSeance, type CorrectionIa } from "../lib/api";
import { nomModele, useMoteurIa } from "../lib/moteurIa";
import { MarkdownObsidian } from "../markdown/rendu";
import BadgeIa from "../views/BadgeIa";
import { lancerTache, useDerniereTache } from "../lib/taches";
import { BoutonArreter } from "../composants/TachesIa";
import type { ModuleCarte, ProprietesQuestion } from "./modules";

const sansNavigation = (carte: CarteSeance) => ({ source: carte.noteChemin, ouvrirNote: () => {} });

function choix(carte: CarteSeance): string[] {
  return Array.isArray(carte.donnees.choix) ? (carte.donnees.choix as string[]) : [];
}

function bonne(carte: CarteSeance): number {
  return typeof carte.donnees.bonneReponse === "number" ? carte.donnees.bonneReponse : -1;
}

/** Questions sur un schéma (vision) : le dessin est affiché au-dessus de la question. */
function SchemaDeLaQuestion({ carte }: { carte: CarteSeance }) {
  const chemin = typeof carte.donnees.schema === "string" ? carte.donnees.schema : null;
  const [scene, setScene] = useState<Scene | null | undefined>(undefined);
  useEffect(() => {
    if (chemin) chargerScene(chemin).then(setScene);
  }, [chemin]);
  if (!chemin || scene === undefined) return null;
  return <Dessin scene={scene} titre={carte.section} />;
}

/** La phrase de la note qui justifie la réponse (vérifiée lors de la génération). */
function Source({ carte }: { carte: CarteSeance }) {
  const citation = String(carte.donnees.citation ?? "");
  if (!citation) return null;
  return (
    <blockquote className="citation-source">
      « {citation} »
      <span className="texte-doux petit">
        {" "}
        — {carte.titreNote}
        {carte.section ? ` › ${carte.section}` : ""}
      </span>
    </blockquote>
  );
}

export interface ReponseQcm {
  choisi: number | null;
}

function QuestionQcm({ carte, reponse, setReponse, corrige }: ProprietesQuestion<ReponseQcm>) {
  const liste = choix(carte);

  // Touches 1 à 4 (rangée du haut ou pavé numérique) pour choisir, avant la correction.
  useEffect(() => {
    if (corrige) return;
    const touche = (e: KeyboardEvent) => {
      if (e.target instanceof HTMLInputElement || e.target instanceof HTMLTextAreaElement) return;
      const n = /^(Digit|Numpad)([1-4])$/.exec(e.code)?.[2];
      if (n && Number(n) <= liste.length) {
        e.preventDefault();
        setReponse({ choisi: Number(n) - 1 });
      }
    };
    window.addEventListener("keydown", touche);
    return () => window.removeEventListener("keydown", touche);
  }, [corrige, liste.length, setReponse]);

  return (
    <div className="question-qcm">
      <SchemaDeLaQuestion carte={carte} />
      <div className="enonce">
        <MarkdownObsidian markdown={carte.question} navigation={sansNavigation(carte)} />
      </div>
      <div className="choix-qcm">
        {liste.map((c, i) => {
          const juste = corrige && i === bonne(carte);
          const faux = corrige && i === reponse.choisi && i !== bonne(carte);
          return (
            <button
              key={i}
              className={
                "choix" +
                (reponse.choisi === i ? " choisi" : "") +
                (juste ? " choix-juste" : "") +
                (faux ? " choix-faux" : "")
              }
              disabled={corrige}
              onClick={() => setReponse({ choisi: i })}
            >
              <kbd>{i + 1}</kbd>
              <span className="choix-texte">
                <MarkdownObsidian markdown={c} navigation={sansNavigation(carte)} />
              </span>
              {juste && <Check size={18} strokeWidth={2.5} className="choix-marque verdict-juste" aria-label="bonne réponse" />}
              {faux && <X size={18} strokeWidth={2.5} className="choix-marque verdict-faux" aria-label="ta réponse, fausse" />}
            </button>
          );
        })}
      </div>
    </div>
  );
}

export const qcm: ModuleCarte<ReponseQcm> = {
  mode: () => "repondre",
  consigne: (carte) => `QCM${carte.donnees.notion ? ` — ${carte.donnees.notion}` : ""} (touches 1 à 4, puis Entrée)`,
  reponseInitiale: () => ({ choisi: null }),
  Question: QuestionQcm,
  Correction: ({ carte }) => (
    <div className="correction-ia">
      {carte.donnees.justification ? (
        <MarkdownObsidian markdown={String(carte.donnees.justification)} navigation={sansNavigation(carte)} />
      ) : null}
      <Source carte={carte} />
    </div>
  ),
  evaluer: (carte, r) => {
    if (r.choisi === null) return { verdict: "faux", message: "Aucune proposition choisie." };
    return r.choisi === bonne(carte) ? { verdict: "juste" } : { verdict: "faux" };
  },
  enTexte: (r) => (r.choisi === null ? "" : `proposition ${r.choisi + 1}`),
};

/** Question ouverte : tu rédiges, puis tu compares avec la réponse attendue et tu t'auto-notes. */
function QuestionOuverte({ carte, reponse, setReponse, corrige }: ProprietesQuestion<string>) {
  return (
    <div className="question-ouverte">
      <SchemaDeLaQuestion carte={carte} />
      <div className="enonce">
        <MarkdownObsidian markdown={carte.question} navigation={sansNavigation(carte)} />
      </div>
      <textarea
        className="champ zone-reponse"
        rows={4}
        placeholder="Rédige ta réponse (brouillon), puis choisis ta confiance et révèle la correction."
        value={reponse}
        disabled={corrige}
        onChange={(e) => setReponse(e.target.value)}
      />
    </div>
  );
}

/** Note conseillée (1 à 4) d'après la note sur 20 de l'IA. */
export function noteConseillee(sur20: number): { note: 1 | 2 | 3; libelle: string } {
  if (sur20 >= 16) return { note: 3, libelle: "Bien (3)" };
  if (sur20 >= 10) return { note: 2, libelle: "Difficile (2)" };
  return { note: 1, libelle: "À revoir (1)" };
}

function CorrectionOuverte({ carte, reponse }: { carte: CarteSeance; reponse: string }) {
  const moteur = useMoteurIa();
  // La correction est une tâche (interruptible) ; on retrouve celle de cette carte.
  // La même carte peut revenir plus tard dans la journée : la clé inclut la réponse donnée cette fois-ci.
  const cleTache = `${carte.id}|${reponse}`;
  const tache = useDerniereTache("correction", cleTache);
  const enCours = tache?.statut === "en-cours";
  const correction = tache?.statut === "finie" ? (tache.resultat as CorrectionIa) : null;
  const erreur = tache?.statut === "erreur" ? tache.erreur : null;

  function corriger() {
    lancerTache(
      {
        genre: "correction",
        cle: cleTache,
        titre: "Correction d'une réponse",
        ecran: "reviser",
        resume: (c: CorrectionIa) => `${c.note} / 20`,
      },
      ({ id }) => api.corrigerReponse(carte.id, reponse, id),
    ).catch(() => {});
  }

  return (
    <div className="correction-ia">
      <div className="texte-doux petit">Réponse attendue</div>
      <MarkdownObsidian markdown={carte.reponse} navigation={sansNavigation(carte)} />
      <Source carte={carte} />
      {!correction && (
        <div className="rangee bloc-haut">
          <button className="bouton" disabled={!moteur.simple || enCours || reponse.trim().length < 2} onClick={corriger}>
            <Sparkles size={15} aria-hidden />
            {enCours ? (moteur.local ? "Correction par l'IA locale…" : "Correction en cours…") : "Faire corriger ma réponse par l'IA"}
          </button>
          <BoutonArreter tache={tache} />
          <BadgeIa moteur={moteur} />
          {!moteur.simple && !moteur.enLigne && (
            <span className="texte-doux petit">Hors ligne : compare toi-même avec la réponse attendue.</span>
          )}
          {moteur.simple && reponse.trim().length < 2 && (
            <span className="texte-doux petit">Écris ta réponse dans le cadre pour pouvoir la faire corriger.</span>
          )}
        </div>
      )}
      {erreur && <p className="message-erreur">{erreur}</p>}
      {correction && (
        <div className="carte retour-ia">
          <div className="rangee entre">
            <strong className={`verdict-${correction.verdict === "partiel" ? "presque" : correction.verdict}`}>
              {correction.note} / 20 —{" "}
              {correction.verdict === "juste" ? "Juste" : correction.verdict === "partiel" ? "Partiellement juste" : "Faux"}
            </strong>
            <span className="texte-doux petit">Note conseillée : {noteConseillee(correction.note).libelle}</span>
          </div>
          <ListeRetour titre="Ce qui est juste" elements={correction.points_justes} classe="verdict-juste" />
          <ListeRetour titre="Ce qui manque" elements={correction.manques} classe="verdict-presque" />
          <ListeRetour titre="Erreurs" elements={correction.erreurs} classe="verdict-faux" />
          {correction.conseil && (
            <p className="petit">
              <strong>Conseil :</strong> <TexteIa texte={correction.conseil} />
            </p>
          )}
          {correction.modele && <p className="texte-doux petit">Corrigé par {nomModele(correction.modele)}.</p>}
        </div>
      )}
    </div>
  );
}

/** Texte court renvoyé par l'IA, rendu en Markdown pour afficher le gras et les formules ($n$…). */
export function TexteIa({ texte }: { texte: string }) {
  return (
    <span className="texte-ia">
      <MarkdownObsidian markdown={texte} navigation={{ source: "", ouvrirNote: () => {} }} />
    </span>
  );
}

export function ListeRetour({ titre, elements, classe }: { titre: string; elements: string[]; classe: string }) {
  if (!elements.length) return null;
  return (
    <div>
      <div className={`petit ${classe}`}>{titre}</div>
      <ul className="liste-retour">
        {elements.map((e, i) => (
          <li key={i}>
            <TexteIa texte={e} />
          </li>
        ))}
      </ul>
    </div>
  );
}

export const ouverte: ModuleCarte<string> = {
  mode: () => "retourner",
  consigne: (carte) => `Question ouverte${carte.donnees.notion ? ` — ${carte.donnees.notion}` : ""}`,
  reponseInitiale: () => "",
  Question: QuestionOuverte,
  Correction: CorrectionOuverte,
  evaluer: () => ({ verdict: "juste" }),
  enTexte: (r) => r,
};
