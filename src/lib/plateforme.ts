// Système d'exploitation : Révisio tourne sous Windows et (en version expérimentale) sous macOS.
// Ce module donne les mots et les touches propres à chacun.

export const estMac = typeof navigator !== "undefined" && /Mac/i.test(navigator.platform || navigator.userAgent);

/** « Windows » ou « macOS », pour les textes de l'interface. */
export const NOM_SYSTEME = estMac ? "macOS" : "Windows";

/** Coffre à secrets du système, où est rangée la clé Gemini. */
export const NOM_COFFRE = estMac ? "Trousseau de macOS" : "Gestionnaire d'identification de Windows";

/** Touche des raccourcis : Ctrl sous Windows, ⌘ (Commande) sous macOS. */
export const TOUCHE_COMMANDE = estMac ? "⌘" : "Ctrl";

/** Touche Alt sous Windows, ⌥ (Option) sous macOS. */
export const TOUCHE_ALT = estMac ? "⌥" : "Alt";

/** Vrai si la touche des raccourcis est enfoncée (Ctrl sous Windows, ⌘ sous macOS). */
export function toucheCommande(e: { ctrlKey: boolean; metaKey: boolean }): boolean {
  return estMac ? e.metaKey : e.ctrlKey;
}
