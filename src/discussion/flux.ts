// Texte de la réponse de l'IA pendant qu'elle s'écrit (évènement « discussion-flux » envoyé par Révisio).
// Gardé hors de l'écran : en revenant sur une conversation dont la réponse est en cours, on la voit avancer.
import { useSyncExternalStore } from "react";
import { listen } from "@tauri-apps/api/event";

const textes = new Map<number, string>();
const abonnes = new Set<() => void>();
let ecoute = false;

function publier() {
  abonnes.forEach((f) => f());
}

/** Commence à écouter Révisio (une seule fois pour toute l'app). */
export function ecouterFlux() {
  if (ecoute) return;
  ecoute = true;
  listen<{ conversationId: number; texte: string }>("discussion-flux", (e) => {
    textes.set(e.payload.conversationId, e.payload.texte);
    publier();
  }).catch(() => {
    ecoute = false;
  });
}

/** La réponse est terminée (ou interrompue) : on oublie le texte provisoire. */
export function effacerFlux(conversationId: number) {
  if (textes.delete(conversationId)) publier();
}

export function useFlux(conversationId: number | null): string | null {
  return useSyncExternalStore(
    (f) => {
      abonnes.add(f);
      return () => abonnes.delete(f);
    },
    () => (conversationId === null ? null : (textes.get(conversationId) ?? null)),
  );
}
