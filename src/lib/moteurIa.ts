// Quelle IA répondra aux tâches simples (questions, corrections, Feynman, cartes mentales) :
// Gemini (en ligne) ou l'IA locale Ollama (sur le PC), selon le réglage et ce qui est disponible.
// Le choix réel est fait par Révisio côté Rust (même logique) ; ici on sait quoi afficher et quels
// boutons activer, même hors ligne.
import { useCallback, useEffect, useState } from "react";
import { api, EtatIa, EtatIaLocale, ModeIaLocale, Reglages } from "./api";
import { useEnLigne, useModeLocal } from "./connexion";

export interface ChoixMoteur {
  /** Une tâche simple peut être lancée. */
  simple: boolean;
  /** C'est l'IA locale qui répondra (plus lente : prévenir). */
  local: boolean;
  /** À afficher près des boutons (null : Gemini, comme d'habitude). */
  badge: string | null;
}

/** Le modèle choisi est-il installé dans Ollama (« qwen3.5:4b », ou « nom » pour « nom:latest ») ? */
export function modeleInstalle(choisi: string, installes: string[]): boolean {
  if (!choisi) return false;
  const complet = choisi.includes(":") ? choisi : `${choisi}:latest`;
  return installes.includes(choisi) || installes.includes(complet);
}

export function choisirMoteur(mode: ModeIaLocale, gemini: boolean, modeleLocal: string | null): ChoixMoteur {
  if (mode === "toujours") {
    return modeleLocal
      ? { simple: true, local: true, badge: `IA locale · ${modeleLocal}` }
      : { simple: false, local: false, badge: "IA locale indisponible (Ollama ne répond pas ou modèle absent)" };
  }
  if (mode === "secours" && !gemini && modeleLocal) {
    return { simple: true, local: true, badge: `IA locale · ${modeleLocal} (Gemini indisponible)` };
  }
  return { simple: gemini, local: false, badge: null };
}

export interface MoteurIa extends ChoixMoteur {
  etat: EtatIa | null;
  locale: EtatIaLocale | null;
  reglages: Reglages | null;
  enLigne: boolean;
  /** Gemini utilisable (Internet + clé). */
  gemini: boolean;
  /** Modèle local prêt à répondre (Ollama répond, modèle installé, IA locale activée). */
  modeleLocal: string | null;
  recharger: () => void;
}

export function useMoteurIa(): MoteurIa {
  const enLigne = useEnLigne();
  const modeLocal = useModeLocal();
  const [etat, setEtat] = useState<EtatIa | null>(null);
  const [locale, setLocale] = useState<EtatIaLocale | null>(null);
  const [reglages, setReglages] = useState<Reglages | null>(null);

  const recharger = useCallback(() => {
    api.etatIa().then(setEtat).catch(() => setEtat(null));
    api.lireReglages().then(setReglages).catch(() => setReglages(null));
    api.etatIaLocale().then(setLocale).catch(() => setLocale(null));
  }, []);

  useEffect(recharger, [recharger, enLigne]);
  // Ollama a pu être lancé entre-temps : on revérifie quand on revient sur la fenêtre.
  useEffect(() => {
    window.addEventListener("focus", recharger);
    return () => window.removeEventListener("focus", recharger);
  }, [recharger]);

  // Mode local (interrupteur de la barre du haut) : Gemini coupé, l'IA locale répond toujours.
  const gemini = enLigne && !!etat?.clePresente && !modeLocal;
  const mode = modeLocal ? "toujours" : (reglages?.iaLocale ?? "jamais");
  const pret =
    mode !== "jamais" &&
    !!reglages?.modeleLocal &&
    !!locale?.version &&
    modeleInstalle(reglages.modeleLocal, locale.modeles.map((m) => m.nom));
  const modeleLocal = pret ? reglages!.modeleLocal : null;
  return { etat, locale, reglages, enLigne, gemini, modeleLocal, recharger, ...choisirMoteur(mode, gemini, modeleLocal) };
}

/** Nom lisible du modèle qui a répondu (« local:qwen3.5:4b » → « IA locale (qwen3.5:4b) »). */
export function nomModele(modele: string | undefined | null): string {
  if (!modele) return "";
  return modele.startsWith("local:") ? `IA locale (${modele.slice(6)})` : `Gemini (${modele})`;
}
