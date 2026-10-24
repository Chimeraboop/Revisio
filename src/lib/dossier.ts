// Le dossier de notes est-il un vault Obsidian ? (valeur partagée, posée par App à chaque changement
// de dossier) : le bouton « Ouvrir dans Obsidian » n'apparaît que dans ce cas.
import { useSyncExternalStore } from "react";

let estObsidian = false;
const abonnes = new Set<() => void>();

export function definirEstObsidian(valeur: boolean) {
  if (valeur === estObsidian) return;
  estObsidian = valeur;
  abonnes.forEach((f) => f());
}

export function useEstObsidian(): boolean {
  return useSyncExternalStore(
    (f) => {
      abonnes.add(f);
      return () => abonnes.delete(f);
    },
    () => estObsidian,
  );
}
