// Petite mention de l'IA qui répondra à une tâche simple quand ce n'est pas Gemini (IA locale, plus lente).
import { Cpu } from "lucide-react";
import type { MoteurIa } from "../lib/moteurIa";

export default function BadgeIa({ moteur }: { moteur: MoteurIa }) {
  if (!moteur.badge) return null;
  return (
    <span className={"pastille badge-ia " + (moteur.simple ? "statut-fragile" : "statut-a-revoir")}>
      <Cpu size={13} className="icone" aria-hidden /> {moteur.badge}
      {moteur.local && " — plus lente que Gemini : compte quelques minutes"}
    </span>
  );
}
