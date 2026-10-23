// Répétition espacée FSRS (bibliothèque ts-fsrs).
// L'état d'une carte est stocké en JSON dans la base ; ici on le relit, on le fait avancer
// selon la note donnée (1 à 4) et on calcule la prochaine date de révision.
import { createEmptyCard, fsrs, generatorParameters, State, type Card, type Grade } from "ts-fsrs";

export type Note = 1 | 2 | 3 | 4;

export const LIBELLES_NOTES: Record<Note, string> = {
  1: "À revoir",
  2: "Difficile",
  3: "Bien",
  4: "Facile",
};

// Rétention visée : 90 % (valeur recommandée par FSRS). Le « fuzz » évite que toutes
// les cartes apprises le même jour reviennent toutes le même jour.
const planificateur = fsrs(generatorParameters({ request_retention: 0.9, enable_fuzz: true }));

export function lireEtat(json: string | null | undefined, maintenant: Date): Card {
  if (!json) return createEmptyCard(maintenant);
  const brut = JSON.parse(json);
  return {
    ...brut,
    due: new Date(brut.due),
    last_review: brut.last_review ? new Date(brut.last_review) : undefined,
  };
}

/** Nouvel état après une réponse notée `note`. */
export function planifier(json: string | null, note: Note, maintenant = new Date()) {
  const { card } = planificateur.next(lireEtat(json, maintenant), maintenant, note as Grade);
  return { etat: JSON.stringify(card), echeance: card.due.getTime(), carte: card };
}

/** Délai avant la prochaine révision pour chacune des 4 notes (affiché sur les boutons). */
export function apercu(json: string | null, maintenant = new Date()): Record<Note, string> {
  const resultat = planificateur.repeat(lireEtat(json, maintenant), maintenant);
  const delai = (n: Note) => formaterDelai(resultat[n as Grade].card.due.getTime() - maintenant.getTime());
  return { 1: delai(1), 2: delai(2), 3: delai(3), 4: delai(4) };
}

/** Probabilité (0 à 1) de se souvenir de la carte maintenant ; 0 pour une carte jamais vue. */
export function retention(json: string | null, maintenant = new Date()): number {
  if (!json) return 0;
  const carte = lireEtat(json, maintenant);
  if (carte.state === State.New) return 0;
  return planificateur.get_retrievability(carte, maintenant, false);
}

export function formaterDelai(ms: number): string {
  const minutes = Math.max(1, Math.round(ms / 60_000));
  if (minutes < 60) return `${minutes} min`;
  const heures = Math.round(minutes / 60);
  if (heures < 24) return `${heures} h`;
  const jours = Math.round(heures / 24);
  if (jours < 31) return `${jours} j`;
  const mois = Math.round(jours / 30);
  if (mois < 12) return `${mois} mois`;
  return `${Math.round(jours / 365)} an${jours >= 730 ? "s" : ""}`;
}
