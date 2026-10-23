import { useEffect, useState } from "react";
import { listen } from "@tauri-apps/api/event";
import { api, EtatIndexation } from "./api";

/** État de l'indexation du vault, mis à jour en direct par l'événement « indexation » du cœur Rust. */
export function useIndexation(): EtatIndexation | null {
  const [etat, setEtat] = useState<EtatIndexation | null>(null);

  useEffect(() => {
    let actif = true;
    api.etatIndexation().then((e) => actif && setEtat(e));
    const arret = listen<EtatIndexation>("indexation", (ev) => setEtat(ev.payload));
    return () => {
      actif = false;
      arret.then((f) => f());
    };
  }, []);

  return etat;
}
