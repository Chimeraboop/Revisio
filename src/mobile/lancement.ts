// Quand le téléphone se synchronise tout seul (décisions pures, testées).
//
// La connexion à Google ne dure qu'une heure. Au lancement de l'app, rien n'est en cours : si elle a expiré,
// on la renouvelle aussitôt (aller-retour chez Google sans rien demander, la page se recharge), sinon le
// téléphone resterait avec le contenu d'il y a plusieurs jours. Au retour dans l'app (elle était en arrière-plan),
// une séance peut être en cours : on ne quitte jamais la page, on signale seulement qu'il faut synchroniser.

/** Pas deux renouvellements automatiques de suite (Google qui ne répond pas comme prévu ne doit pas boucler). */
export const ECART_RENOUVELLEMENT = 10 * 60_000;
/** Au retour dans l'app, pas de nouvelle synchronisation si la dernière est toute récente. */
export const ECART_RETOUR = 2 * 60_000;

export interface Situation {
  connecte: boolean;
  enLigne: boolean;
  jetonValable: boolean;
  maintenant: number;
}

export function auLancement(
  s: Situation & { refusGoogle: boolean; dernierRenouvellement: number | null },
): "synchroniser" | "renouveler" | "attendre" {
  if (!s.connecte || !s.enLigne) return "attendre";
  if (s.jetonValable) return "synchroniser";
  if (s.refusGoogle) return "attendre";
  if (s.dernierRenouvellement !== null && s.maintenant - s.dernierRenouvellement < ECART_RENOUVELLEMENT) return "attendre";
  return "renouveler";
}

export function auRetour(s: Situation & { derniere: number | null; enCours: boolean }): "synchroniser" | "signaler" | "rien" {
  if (!s.connecte || !s.enLigne || s.enCours) return "rien";
  if (s.derniere !== null && s.maintenant - s.derniere < ECART_RETOUR) return "rien";
  return s.jetonValable ? "synchroniser" : "signaler";
}
