import { useEffect, useSyncExternalStore } from "react";
import { invoke } from "@tauri-apps/api/core";

// Connexion Internet, partagée par tous les écrans.
// `navigator.onLine` ne suffit pas sous Windows : il dit « en ligne » dès qu'une carte réseau est active
// (cartes virtuelles, Wi-Fi sans accès…). Révisio vérifie donc aussi que le serveur de Gemini est joignable
// (commande `verifier_internet`) : au lancement, à chaque changement de réseau, au retour sur la fenêtre,
// toutes les minutes, et dès qu'une génération échoue faute de connexion.
let enLigne = navigator.onLine;
const abonnesReseau = new Set<() => void>();
let verification: Promise<void> | null = null;
let surveillanceDemarree = false;

function poserEnLigne(valeur: boolean) {
  if (valeur === enLigne) return;
  enLigne = valeur;
  abonnesReseau.forEach((f) => f());
}

/** Revérifie tout de suite l'accès à Internet (une vérification à la fois). */
export function verifierInternet(): Promise<void> {
  if (!navigator.onLine) {
    poserEnLigne(false);
    return Promise.resolve();
  }
  verification ??= invoke<boolean>("verifier_internet")
    .then(poserEnLigne)
    .catch(() => poserEnLigne(navigator.onLine))
    .finally(() => (verification = null));
  return verification;
}

/** Une génération a échoué faute de connexion : on le sait sans attendre la prochaine vérification. */
export function signalerErreurReseau(message: string) {
  if (message.startsWith("Pas de connexion Internet")) poserEnLigne(false);
}

function demarrerSurveillance() {
  if (surveillanceDemarree) return;
  surveillanceDemarree = true;
  const verifier = () => void verifierInternet();
  window.addEventListener("online", verifier);
  window.addEventListener("offline", verifier);
  window.addEventListener("focus", verifier);
  setInterval(verifier, 60_000);
  verifier();
}

/** Vrai si Internet est vraiment joignable (l'IA en ligne et l'espace web en ont besoin). */
export function useEnLigne(): boolean {
  useEffect(demarrerSurveillance, []);
  return useSyncExternalStore(
    (f) => {
      abonnesReseau.add(f);
      return () => abonnesReseau.delete(f);
    },
    () => enLigne,
  );
}

// Mode local (interrupteur « En ligne » de la barre du haut) : Gemini est coupé, l'IA locale répond.
// Valeur partagée par tous les écrans ; App la met à jour depuis les réglages.
let modeLocal = false;
const abonnes = new Set<() => void>();

export function definirModeLocalCourant(valeur: boolean) {
  if (valeur === modeLocal) return;
  modeLocal = valeur;
  abonnes.forEach((f) => f());
}

export function useModeLocal(): boolean {
  return useSyncExternalStore(
    (f) => {
      abonnes.add(f);
      return () => abonnes.delete(f);
    },
    () => modeLocal,
  );
}
