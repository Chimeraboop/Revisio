// Sélecteur de date de Révisio (à la place du petit calendrier de Windows) : un bouton qui affiche la date
// en toutes lettres et ouvre un mois en grille (lundi → dimanche), dans le style de l'app.
// Clavier : flèches (jour / semaine), Page préc./suiv. (mois), Entrée pour choisir, Échap pour fermer.
import { useEffect, useRef, useState } from "react";
import { CalendarDays, ChevronLeft, ChevronRight } from "lucide-react";
import { dansLeMois, decalerMois, grilleMois, moisDe, nomMois } from "../planning/grilleCalendrier";
import { ajouterJours } from "../planning/planning";

const JOURS = ["L", "M", "M", "J", "V", "S", "D"];
const NOMS_JOURS = ["lundi", "mardi", "mercredi", "jeudi", "vendredi", "samedi", "dimanche"];
const formatLong = new Intl.DateTimeFormat("fr-FR", {
  weekday: "long",
  day: "numeric",
  month: "long",
  year: "numeric",
  timeZone: "UTC",
});

/** « lundi 26 octobre 2026 » pour « 2026-10-26 ». */
export function dateEnLettres(jour: string): string {
  const [a, m, j] = jour.split("-").map(Number);
  return formatLong.format(new Date(Date.UTC(a, m - 1, j)));
}

interface Props {
  id?: string;
  /** AAAA-MM-JJ */
  valeur: string;
  onChange: (jour: string) => void;
  /** Jours avant `min` non choisissables. */
  min?: string;
  /** Aujourd'hui (AAAA-MM-JJ), mis en évidence. */
  aujourdhui: string;
  /** Jours marqués d'un point (ex. évaluations déjà prévues), avec leur description. */
  marques?: Map<string, string>;
}

export default function ChoixDate({ id, valeur, onChange, min, aujourdhui, marques }: Props) {
  const [ouvert, setOuvert] = useState(false);
  const [mois, setMois] = useState(() => moisDe(valeur || aujourdhui));
  const [focus, setFocus] = useState(valeur || aujourdhui);
  const zone = useRef<HTMLDivElement>(null);
  const grille = useRef<HTMLDivElement>(null);

  const permis = (j: string) => !min || j >= min;

  function ouvrir() {
    const depart = valeur || aujourdhui;
    setMois(moisDe(depart));
    setFocus(depart);
    setOuvert(true);
  }

  function choisir(j: string) {
    if (!permis(j)) return;
    onChange(j);
    setOuvert(false);
  }

  // Fermeture au clic à l'extérieur.
  useEffect(() => {
    if (!ouvert) return;
    const clic = (e: MouseEvent) => {
      if (!zone.current?.contains(e.target as Node)) setOuvert(false);
    };
    document.addEventListener("mousedown", clic);
    return () => document.removeEventListener("mousedown", clic);
  }, [ouvert]);

  // Le jour qui a le focus clavier suit les flèches (et change de mois si besoin).
  useEffect(() => {
    if (!ouvert) return;
    if (!dansLeMois(focus, mois)) setMois(moisDe(focus));
    grille.current?.querySelector<HTMLButtonElement>(`[data-jour="${focus}"]`)?.focus();
  }, [focus, ouvert, mois]);

  function touche(e: React.KeyboardEvent) {
    const pas: Record<string, number> = { ArrowLeft: -1, ArrowRight: 1, ArrowUp: -7, ArrowDown: 7 };
    if (e.key in pas) {
      e.preventDefault();
      setFocus((f) => ajouterJours(f, pas[e.key]));
    } else if (e.key === "PageUp" || e.key === "PageDown") {
      e.preventDefault();
      // Même jour du mois précédent / suivant (le 31 devient le dernier jour d'un mois plus court).
      const m = decalerMois(moisDe(focus), e.key === "PageUp" ? -1 : 1);
      const jour = Math.min(Number(focus.slice(8)), new Date(Date.UTC(m.annee, m.mois + 1, 0)).getUTCDate());
      setFocus(`${m.annee}-${String(m.mois + 1).padStart(2, "0")}-${String(jour).padStart(2, "0")}`);
    } else if (e.key === "Escape") {
      e.preventDefault();
      setOuvert(false);
      zone.current?.querySelector<HTMLButtonElement>(".choix-date-bouton")?.focus();
    }
  }

  return (
    <div className="choix-date" ref={zone}>
      <button
        id={id}
        type="button"
        className="champ choix-date-bouton"
        aria-haspopup="dialog"
        aria-expanded={ouvert}
        onClick={() => (ouvert ? setOuvert(false) : ouvrir())}
      >
        <CalendarDays size={16} className="icone" aria-hidden />
        {valeur ? dateEnLettres(valeur) : "Choisis une date"}
      </button>
      {ouvert && (
        <div className="choix-date-panneau" role="dialog" aria-label="Choisir une date" onKeyDown={touche}>
          <div className="choix-date-entete">
            <button type="button" className="bouton bouton-icone" aria-label="Mois précédent" onClick={() => setMois((m) => decalerMois(m, -1))}>
              <ChevronLeft size={17} aria-hidden />
            </button>
            <strong>{nomMois(mois)}</strong>
            <button type="button" className="bouton bouton-icone" aria-label="Mois suivant" onClick={() => setMois((m) => decalerMois(m, 1))}>
              <ChevronRight size={17} aria-hidden />
            </button>
          </div>
          <div className="choix-date-grille" role="grid" ref={grille}>
            {JOURS.map((j, i) => (
              <span key={i} className="choix-date-jour-semaine" aria-label={NOMS_JOURS[i]}>
                {j}
              </span>
            ))}
            {grilleMois(mois).map((j) => {
              const marque = marques?.get(j);
              const classes = [
                "choix-date-jour",
                dansLeMois(j, mois) ? "" : "hors-mois",
                j === aujourdhui ? "aujourdhui" : "",
                j === valeur ? "choisi" : "",
              ];
              return (
                <button
                  key={j}
                  type="button"
                  data-jour={j}
                  className={classes.filter(Boolean).join(" ")}
                  disabled={!permis(j)}
                  tabIndex={j === focus ? 0 : -1}
                  aria-label={dateEnLettres(j) + (marque ? ` · ${marque}` : "")}
                  aria-pressed={j === valeur}
                  title={marque}
                  onClick={() => choisir(j)}
                  onFocus={() => setFocus(j)}
                >
                  {Number(j.slice(8))}
                  {marque && <span className="choix-date-point" aria-hidden />}
                </button>
              );
            })}
          </div>
          <div className="choix-date-pied">
            {[
              ["Demain", 1],
              ["Dans 1 semaine", 7],
              ["Dans 2 semaines", 14],
            ].map(([libelle, n]) => (
              <button key={libelle} type="button" className="lien-discret petit" onClick={() => choisir(ajouterJours(aujourdhui, n as number))}>
                {libelle}
              </button>
            ))}
          </div>
        </div>
      )}
    </div>
  );
}
