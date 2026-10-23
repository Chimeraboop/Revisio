// Petites animations du tableau de bord (désactivées si Windows demande de réduire les animations).
import { useEffect, useRef, useState } from "react";

export function animationsReduites(): boolean {
  return !!window.matchMedia?.("(prefers-reduced-motion: reduce)").matches;
}

/** Valeur qui monte doucement jusqu'à `cible` (courbe qui ralentit à la fin). */
export function useCompteur(cible: number, dureeMs = 900): number {
  const [valeur, setValeur] = useState(0);
  const depart = useRef(0);
  useEffect(() => {
    if (animationsReduites()) {
      depart.current = cible;
      setValeur(cible);
      return;
    }
    const de = depart.current;
    const t0 = performance.now();
    let id = 0;
    const pas = (t: number) => {
      const p = Math.min(1, (t - t0) / dureeMs);
      const v = de + (cible - de) * (1 - Math.pow(1 - p, 3));
      depart.current = v;
      setValeur(v);
      if (p < 1) id = requestAnimationFrame(pas);
    };
    id = requestAnimationFrame(pas);
    return () => cancelAnimationFrame(id);
  }, [cible, dureeMs]);
  return valeur;
}

/** Évolution entre deux périodes, en % (null si la période précédente est vide). */
export function evolution(avant: number, apres: number): number | null {
  if (avant <= 0) return null;
  return Math.round(((apres - avant) / avant) * 100);
}
