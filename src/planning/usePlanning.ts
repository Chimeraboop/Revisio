// Charge les évaluations, la maîtrise des chapitres et l'avancement du jour, puis calcule le planning.
import { useCallback, useEffect, useMemo, useState } from "react";
import { api, CarteStat, Evaluation, JourActivite, RevuesChapitre } from "../lib/api";
import { jourLocal } from "../revision/stats";
import { chapitresDuJour, infoParChapitre, planifier } from "./planning";

interface Donnees {
  evaluations: Evaluation[];
  cartes: CarteStat[];
  revues: RevuesChapitre[];
  /** Activité passée (réponses par jour), pour le calendrier. */
  jours: JourActivite[];
}

/** `actualiser` : toute valeur qui change quand il faut recharger (fin d'indexation…). */
export function usePlanning(actualiser?: unknown) {
  const [donnees, setDonnees] = useState<Donnees | null>(null);
  const aujourdhui = jourLocal(new Date());

  const charger = useCallback(
    () =>
      Promise.all([api.evaluations(jourLocal(new Date())), api.tableauDeBord(), api.revuesDuJour()])
        .then(([evaluations, tableau, revues]) =>
          setDonnees({ evaluations, cartes: tableau.cartes, revues, jours: tableau.jours }),
        )
        .catch(() => setDonnees({ evaluations: [], cartes: [], revues: [], jours: [] })),
    [],
  );

  useEffect(() => {
    charger();
  }, [charger, actualiser]);

  const infos = useMemo(() => infoParChapitre(donnees?.cartes ?? []), [donnees]);
  const plan = useMemo(
    () => (donnees ? planifier(donnees.evaluations, infos, aujourdhui) : []),
    [donnees, infos, aujourdhui],
  );
  const duJour = useMemo(() => chapitresDuJour(plan, aujourdhui), [plan, aujourdhui]);

  /** Chapitres d'aujourd'hui recalculés à l'instant (au moment de lancer une séance). */
  const chapitresDuJourFrais = useCallback(async () => {
    try {
      const jour = jourLocal(new Date());
      const [evaluations, tableau] = await Promise.all([api.evaluations(jour), api.tableauDeBord()]);
      return chapitresDuJour(planifier(evaluations, infoParChapitre(tableau.cartes), jour), jour);
    } catch {
      return [];
    }
  }, []);

  return {
    charge: donnees !== null,
    evaluations: donnees?.evaluations ?? [],
    revues: donnees?.revues ?? [],
    jours: donnees?.jours ?? [],
    infos,
    plan,
    aujourdhui,
    /** Chapitres à réviser aujourd'hui (séance du jour). */
    duJour,
    chapitresDuJour: chapitresDuJourFrais,
    charger,
  };
}
