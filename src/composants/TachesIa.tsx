// Générations par l'IA en arrière-plan : indicateur de la barre du haut (liste + bouton Arrêter),
// notifications de fin, et bouton « Arrêter » à placer à côté des boutons de génération.
import { useEffect, useRef, useState } from "react";
import { CircleAlert, CircleCheck, CircleSlash, LoaderCircle, Sparkles, Square, X } from "lucide-react";
import {
  annulerTache,
  EcranTache,
  fermerNotification,
  Tache,
  useNotifications,
  useTaches,
} from "../lib/taches";

function duree(t: Tache, maintenant: number): string {
  const s = Math.max(0, Math.round(((t.fin ?? maintenant) - t.debut) / 1000));
  return s < 60 ? `${s} s` : `${Math.floor(s / 60)} min ${String(s % 60).padStart(2, "0")}`;
}

/** Fait avancer les durées affichées chaque seconde tant qu'une tâche tourne. */
function useMaintenant(actif: boolean): number {
  const [maintenant, setMaintenant] = useState(Date.now());
  useEffect(() => {
    if (!actif) return;
    const minuteur = setInterval(() => setMaintenant(Date.now()), 1000);
    return () => clearInterval(minuteur);
  }, [actif]);
  return maintenant;
}

const ICONES_STATUT = {
  finie: CircleCheck,
  erreur: CircleAlert,
  interrompue: CircleSlash,
} as const;

function texteStatut(t: Tache): string {
  if (t.statut === "finie") return t.resume ?? "Terminé";
  if (t.statut === "interrompue") return "Interrompue";
  return t.erreur ?? "Erreur";
}

/** Bouton « Arrêter » d'une génération en cours (rien si la tâche n'est pas en cours). */
export function BoutonArreter({ tache }: { tache: Tache | undefined }) {
  if (tache?.statut !== "en-cours") return null;
  const arret = tache.etape === "Arrêt en cours…";
  return (
    <button className="bouton bouton-arreter" onClick={() => annulerTache(tache.id)} disabled={arret}>
      <Square size={13} className="icone" fill="currentColor" aria-hidden />
      {arret ? "Arrêt…" : "Arrêter"}
    </button>
  );
}

/** Indicateur de la barre du haut : générations en cours et récentes, avec un bouton Arrêter pour chacune. */
export function IndicateurTaches({ onOuvrir }: { onOuvrir: (ecran: EcranTache) => void }) {
  const taches = useTaches();
  const [ouvert, setOuvert] = useState(false);
  const zone = useRef<HTMLDivElement>(null);
  const enCours = taches.filter((t) => t.statut === "en-cours");
  const maintenant = useMaintenant(ouvert && enCours.length > 0);

  // Fermeture au clic à l'extérieur ou avec Échap.
  useEffect(() => {
    if (!ouvert) return;
    const clic = (e: MouseEvent) => {
      if (!zone.current?.contains(e.target as Node)) setOuvert(false);
    };
    const touche = (e: KeyboardEvent) => e.key === "Escape" && setOuvert(false);
    document.addEventListener("mousedown", clic);
    document.addEventListener("keydown", touche);
    return () => {
      document.removeEventListener("mousedown", clic);
      document.removeEventListener("keydown", touche);
    };
  }, [ouvert]);

  if (taches.length === 0) return null;
  const recentes = [...taches].reverse().slice(0, 8);

  return (
    <div className="taches-haut" ref={zone}>
      <button
        className={"indicateur indicateur-taches" + (enCours.length ? " actif" : "")}
        aria-expanded={ouvert}
        onClick={() => setOuvert((o) => !o)}
        title="Générations par l'IA"
      >
        {enCours.length ? (
          <>
            <LoaderCircle className="icone tourne" size={14} aria-hidden />
            {enCours.length} génération{enCours.length > 1 ? "s" : ""}
          </>
        ) : (
          <>
            <Sparkles className="icone" size={14} aria-hidden /> IA
          </>
        )}
      </button>
      {ouvert && (
        <div className="panneau-taches" role="dialog" aria-label="Générations par l'IA">
          <div className="panneau-taches-titre">Générations par l'IA</div>
          <ul>
            {recentes.map((t) => {
              const Icone = t.statut === "en-cours" ? LoaderCircle : ICONES_STATUT[t.statut];
              return (
                <li key={t.id} className={`tache-ligne statut-${t.statut}`}>
                  <Icone size={16} className={"icone" + (t.statut === "en-cours" ? " tourne" : "")} aria-hidden />
                  <div className="tache-texte">
                    <button
                      className="lien-discret tache-titre"
                      onClick={() => {
                        onOuvrir(t.ecran);
                        setOuvert(false);
                      }}
                    >
                      {t.titre}
                    </button>
                    <span className="texte-doux petit">
                      {t.statut === "en-cours" ? (t.etape ?? "En cours") : texteStatut(t)} · {duree(t, maintenant)}
                    </span>
                  </div>
                  <BoutonArreter tache={t} />
                </li>
              );
            })}
          </ul>
          <p className="texte-doux petit panneau-taches-pied">
            Tu peux changer d'écran : les générations continuent, et une notification te prévient à la fin.
          </p>
        </div>
      )}
    </div>
  );
}

/** Notifications (en bas à droite) quand une génération se termine pendant que l'élève fait autre chose. */
export function NotificationsTaches({ onOuvrir }: { onOuvrir: (ecran: EcranTache) => void }) {
  const notifications = useNotifications();
  return (
    <div className="notifications" aria-live="polite">
      {notifications.slice(-3).map((t) => (
        <Notification key={t.id} tache={t} onOuvrir={onOuvrir} />
      ))}
    </div>
  );
}

function Notification({ tache, onOuvrir }: { tache: Tache; onOuvrir: (ecran: EcranTache) => void }) {
  // Une réussite disparaît seule au bout de 12 s ; une erreur reste jusqu'à ce qu'on la ferme.
  useEffect(() => {
    if (tache.statut !== "finie") return;
    const minuteur = setTimeout(() => fermerNotification(tache.id), 12_000);
    return () => clearTimeout(minuteur);
  }, [tache.id, tache.statut]);
  const Icone = tache.statut === "finie" ? CircleCheck : CircleAlert;
  return (
    <div className={`notification statut-${tache.statut}`} role="status">
      <Icone size={18} className="icone" aria-hidden />
      <div className="notification-texte">
        <strong>{tache.titre}</strong>
        <span className="petit">{tache.statut === "finie" ? texteStatut(tache) : `Échec : ${texteStatut(tache)}`}</span>
      </div>
      <button className="bouton notification-voir" onClick={() => onOuvrir(tache.ecran)}>
        Voir
      </button>
      <button className="bouton-discret" aria-label="Fermer" onClick={() => fermerNotification(tache.id)}>
        <X size={15} aria-hidden />
      </button>
    </div>
  );
}
