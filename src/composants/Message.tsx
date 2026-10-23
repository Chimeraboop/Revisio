// Message encadré avec icône : information, réussite, attention ou erreur.
import { CircleAlert, CircleCheck, Info, TriangleAlert, WifiOff, type LucideIcon } from "lucide-react";
import type { ReactNode } from "react";

type Genre = "info" | "succes" | "attention" | "erreur" | "hors-ligne";

const ICONES: Record<Genre, LucideIcon> = {
  info: Info,
  succes: CircleCheck,
  attention: TriangleAlert,
  erreur: CircleAlert,
  "hors-ligne": WifiOff,
};

export default function Message({ genre = "info", children }: { genre?: Genre; children: ReactNode }) {
  const Icone = ICONES[genre];
  return (
    <div className={`message message-${genre}`} role={genre === "erreur" ? "alert" : "status"}>
      <Icone size={18} strokeWidth={2} className="message-icone" aria-hidden />
      <div className="message-texte">{children}</div>
    </div>
  );
}
