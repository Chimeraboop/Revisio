// Générations par l'IA en arrière-plan.
// Une génération appartient à ce gestionnaire, pas à l'écran qui l'a lancée : elle continue quand l'élève
// change de page, son résultat reste disponible au retour, et une notification prévient quand elle se termine.
// Chaque tâche a un identifiant transmis au cœur Rust, qui permet de l'interrompre (`annuler_tache`).
import { useSyncExternalStore } from "react";
import { invoke } from "@tauri-apps/api/core";
import { messageErreur } from "./api";
import { signalerErreurReseau } from "./connexion";

/** Écrans qui lancent des générations (mêmes noms que les vues d'App). */
export type EcranTache = "atelier" | "exercices" | "cartes" | "podcasts" | "videos" | "reviser" | "photos";

export type StatutTache = "en-cours" | "finie" | "erreur" | "interrompue";

export interface Tache {
  id: string;
  /** Sorte de génération (« questions », « feynman », « podcast »…). */
  genre: string;
  /** Précise à quoi la tâche se rapporte (carte corrigée, copie…), pour la retrouver sur l'écran. */
  cle?: string;
  /** « Questions · La méiose » : affiché dans la notification et la liste des tâches. */
  titre: string;
  ecran: EcranTache;
  debut: number;
  fin?: number;
  statut: StatutTache;
  /** Étape en cours, pour les générations en plusieurs temps (« Voix : morceau 2 sur 4 »). */
  etape?: string;
  resultat?: unknown;
  erreur?: string;
  /** Phrase de la notification de fin (« 8 questions ajoutées à tes révisions »). */
  resume?: string;
}

export interface ControleTache {
  id: string;
  /** Vrai si l'élève a demandé l'arrêt : une génération en plusieurs étapes doit alors s'arrêter. */
  arretee: () => boolean;
  etape: (texte: string) => void;
}

export interface LancementTache<T> {
  genre: string;
  cle?: string;
  titre: string;
  ecran: EcranTache;
  /** Phrase de fin à partir du résultat. */
  resume?: (resultat: T) => string;
}

let taches: Tache[] = [];
const arrets = new Set<string>();
const abonnes = new Set<() => void>();
/** Écran affiché (posé par App) : pas de notification pour une tâche dont on voit déjà le résultat. */
let ecranActif: string | null = null;
/** Tâches terminées dont l'élève n'a pas encore vu la notification. */
let notifications: string[] = [];

function publier() {
  abonnes.forEach((f) => f());
}

function modifier(id: string, changement: Partial<Tache>) {
  taches = taches.map((t) => (t.id === id ? { ...t, ...changement } : t));
  publier();
}

function nouvelId(): string {
  return crypto.randomUUID();
}

/** Lance une génération ; la promesse renvoyée donne le résultat (ou l'erreur), même si l'écran a été quitté. */
export function lancerTache<T>(infos: LancementTache<T>, travail: (controle: ControleTache) => Promise<T>): Promise<T> {
  const id = nouvelId();
  const tache: Tache = {
    id,
    genre: infos.genre,
    cle: infos.cle,
    titre: infos.titre,
    ecran: infos.ecran,
    debut: Date.now(),
    statut: "en-cours",
  };
  // On garde les tâches récentes seulement (une dizaine suffit pour la liste et le retour sur un écran).
  taches = [...taches.filter((t) => t.statut === "en-cours" || Date.now() - (t.fin ?? 0) < 3_600_000).slice(-15), tache];
  publier();
  const controle: ControleTache = {
    id,
    arretee: () => arrets.has(id),
    etape: (texte) => modifier(id, { etape: texte }),
  };
  const promesse = travail(controle);
  promesse.then(
    (resultat) => {
      if (arrets.has(id)) return terminer(id, { statut: "interrompue" });
      terminer(id, { statut: "finie", resultat, resume: infos.resume?.(resultat) });
    },
    (e) => {
      const message = messageErreur(e);
      signalerErreurReseau(message);
      terminer(id, arrets.has(id) ? { statut: "interrompue" } : { statut: "erreur", erreur: message });
    },
  );
  return promesse;
}

function terminer(id: string, changement: Partial<Tache>) {
  arrets.delete(id);
  const tache = taches.find((t) => t.id === id);
  modifier(id, { ...changement, etape: undefined, fin: Date.now() });
  // Interrompue par l'élève : il le sait déjà. Sinon, on prévient s'il regarde un autre écran.
  if (tache && changement.statut !== "interrompue" && (ecranActif !== tache.ecran || (typeof document !== "undefined" && document.hidden))) {
    notifications = [...notifications, id];
    publier();
  }
}

/** Travail fait par le cœur Rust sans que l'interface l'ait lancé (photos du dossier surveillé) :
 * une tâche déjà terminée est ajoutée pour la liste et la notification. */
export function signalerTacheTerminee(infos: { genre: string; titre: string; ecran: EcranTache; resume: string; erreur?: boolean }) {
  const id = nouvelId();
  const maintenant = Date.now();
  taches = [
    ...taches.slice(-15),
    {
      id,
      genre: infos.genre,
      titre: infos.titre,
      ecran: infos.ecran,
      debut: maintenant,
      fin: maintenant,
      statut: infos.erreur ? "erreur" : "finie",
      resume: infos.erreur ? undefined : infos.resume,
      erreur: infos.erreur ? infos.resume : undefined,
    },
  ];
  if (ecranActif !== infos.ecran || (typeof document !== "undefined" && document.hidden)) notifications = [...notifications, id];
  publier();
}

export const MESSAGE_INTERROMPUE = "Génération interrompue.";

/** Pour les générations en plusieurs étapes : à appeler entre deux étapes (lève une erreur si l'arrêt est demandé). */
export function verifierArret(controle: ControleTache) {
  if (controle.arretee()) throw new Error(MESSAGE_INTERROMPUE);
}

/** Attente (quota de la minute…) qui s'arrête aussitôt si l'élève interrompt la génération. */
export async function patienter(controle: ControleTache, secondes: number, texte: (reste: number) => string) {
  for (let s = secondes; s > 0; s--) {
    verifierArret(controle);
    controle.etape(texte(s));
    await new Promise((ok) => setTimeout(ok, 1000));
  }
  verifierArret(controle);
}

/** Arrête une génération : la requête en cours est abandonnée côté Rust, les étapes suivantes ne sont pas lancées. */
export function annulerTache(id: string) {
  const tache = taches.find((t) => t.id === id);
  if (!tache || tache.statut !== "en-cours") return;
  arrets.add(id);
  modifier(id, { etape: "Arrêt en cours…" });
  invoke("annuler_tache", { id }).catch(() => {});
}

export function definirEcranActif(ecran: string) {
  ecranActif = ecran;
  // En ouvrant l'écran d'une tâche terminée, sa notification n'a plus lieu d'être.
  const vues = notifications.filter((id) => taches.find((t) => t.id === id)?.ecran === ecran);
  if (vues.length) {
    notifications = notifications.filter((id) => !vues.includes(id));
    publier();
  }
}

export function fermerNotification(id: string) {
  notifications = notifications.filter((n) => n !== id);
  publier();
}

/** Retire une tâche terminée (son résultat ne s'affiche plus sur l'écran). */
export function oublierTache(id: string) {
  taches = taches.filter((t) => t.id !== id || t.statut === "en-cours");
  notifications = notifications.filter((n) => n !== id);
  publier();
}

function sAbonner(f: () => void) {
  abonnes.add(f);
  return () => {
    abonnes.delete(f);
  };
}

/** Lecture directe (hors React) : toutes les tâches récentes. */
export function lireTaches(): Tache[] {
  return taches;
}

/** Lecture directe (hors React) : tâches terminées à signaler, les plus récentes en dernier. */
export function lireNotifications(): Tache[] {
  return notifications.map((id) => taches.find((t) => t.id === id)).filter((t): t is Tache => !!t);
}

export function useTaches(): Tache[] {
  return useSyncExternalStore(sAbonner, lireTaches);
}

export function useNotifications(): Tache[] {
  const liste = useSyncExternalStore(sAbonner, () => notifications);
  const toutes = useTaches();
  return liste.map((id) => toutes.find((t) => t.id === id)).filter((t): t is Tache => !!t);
}

/** Dernière tâche de ce genre (et de cette clé si elle est donnée) : en cours, ou terminée avec son résultat. */
export function useDerniereTache(genre: string, cle?: string): Tache | undefined {
  const toutes = useTaches();
  return [...toutes].reverse().find((t) => t.genre === genre && (cle === undefined || t.cle === cle));
}

/** Pour les tests. */
export function _reinitialiser() {
  taches = [];
  notifications = [];
  arrets.clear();
  ecranActif = null;
}
