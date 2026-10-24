// Tableau de bord : accueil, chiffres clés, séances du jour, activité, maîtrise par matière, copies, points faibles.
import { useEffect, useMemo, useState } from "react";
import {
  ArrowRight,
  BookOpen,
  CalendarClock,
  Clock,
  Download,
  Flame,
  Languages,
  Layers,
  type LucideIcon,
  RefreshCw,
  Target,
  TrendingDown,
  TrendingUp,
} from "lucide-react";
import {
  api,
  EtatIndexation,
  NoteCopie,
  GroupeRevision,
  NOMS_EVALUATIONS,
  NOMS_GROUPES,
  ResumeRevision,
  TableauDeBord,
} from "../lib/api";
import { NOMS_GENRES } from "../revision/modules";
import { TexteMaths } from "../revision/symboles";
import {
  derniersJours,
  formaterDuree,
  LIBELLES_STATUT,
  maitriseParMatiere,
  serieDeJours,
  statut,
  totaux,
  type Maitrise,
} from "../revision/stats";
import { compteARebours } from "../planning/planning";
import { usePlanning } from "../planning/usePlanning";
import CourbeActivite from "../accueil/CourbeActivite";
import { evolution, useCompteur } from "../accueil/animations";

interface Props {
  indexation: EtatIndexation | null;
  onSeanceDuJour: (groupe: GroupeRevision) => void;
  onPlanning: () => void;
  onRapport: () => void;
  onExercices: () => void;
  onOuvrirNote: (chemin: string, ancre?: string | null) => void;
}

const formatNote = new Intl.NumberFormat("fr-FR", { maximumFractionDigits: 1 });
const formatJourCourt = new Intl.DateTimeFormat("fr-FR", { day: "numeric", month: "short" });
const formatDuJour = new Intl.DateTimeFormat("fr-FR", { weekday: "long", day: "numeric", month: "long" });

function salutation(heure = new Date().getHours()) {
  if (heure < 5 || heure >= 18) return "Bonsoir";
  if (heure < 12) return "Bonjour";
  return "Bon après-midi";
}

/** Point de couleur d'un statut (solide, fragile, à revoir…). */
function Point({ classe }: { classe: string }) {
  return <span className={`point-statut ${classe}`} aria-hidden />;
}

function Jauge({ m }: { m: Maitrise }) {
  const s = statut(m);
  const pourcent = Math.round(m.moyenne * 100);
  return (
    <div className="jauge">
      <div className="jauge-piste">
        <div className={`jauge-remplissage statut-${s}`} style={{ width: `${pourcent}%` }} />
      </div>
      <span className="jauge-statut">
        <Point classe={`statut-${s}`} /> {LIBELLES_STATUT[s]}
        {m.vues > 0 && <span className="texte-doux"> · {pourcent} %</span>}
      </span>
      <span className="texte-doux petit">
        {m.vues} / {m.total} cartes vues
      </span>
    </div>
  );
}

interface Chiffre {
  titre: string;
  icone: LucideIcon;
  valeur: number | null;
  format: (n: number) => string;
  detail: string;
  /** Évolution par rapport à la semaine précédente (en % ou en points). */
  tendance?: { valeur: number; unite: string } | null;
}

function CarteChiffre({ c, rang }: { c: Chiffre; rang: number }) {
  const anime = useCompteur(c.valeur ?? 0);
  const Icone = c.icone;
  const t = c.tendance;
  return (
    <div className="carte-sombre carte-chiffre apparait survol-leve" style={{ "--i": rang } as React.CSSProperties}>
      <div className="carte-chiffre-titre">
        <span>{c.titre}</span>
        <Icone size={17} strokeWidth={1.8} className="icone" aria-hidden />
      </div>
      <div className="carte-chiffre-valeur">{c.valeur === null ? "—" : c.format(anime)}</div>
      <div className="carte-chiffre-detail">
        {t && t.valeur !== 0 && (
          <span className={"tendance " + (t.valeur > 0 ? "hausse" : "baisse")}>
            {t.valeur > 0 ? <TrendingUp size={13} aria-hidden /> : <TrendingDown size={13} aria-hidden />}
            {t.valeur > 0 ? "+" : ""}
            {t.valeur}
            {t.unite}
          </span>
        )}
        <span>{c.detail}</span>
      </div>
    </div>
  );
}

export default function AccueilVue({ indexation, onSeanceDuJour, onPlanning, onRapport, onExercices, onOuvrirNote }: Props) {
  const [tableau, setTableau] = useState<TableauDeBord | null>(null);
  const [resume, setResume] = useState<ResumeRevision | null>(null);
  const [notes, setNotes] = useState<NoteCopie[]>([]);

  // Recharge à chaque fin d'indexation (les cartes ont pu changer).
  useEffect(() => {
    if (indexation?.enCours) return;
    api.tableauDeBord().then(setTableau).catch(() => setTableau(null));
    api.resumeRevision().then(setResume).catch(() => setResume(null));
    api.dernieresNotes().then(setNotes).catch(() => setNotes([]));
  }, [indexation?.enCours, indexation?.dernier]);

  const maitrise = useMemo(() => (tableau ? maitriseParMatiere(tableau.cartes) : []), [tableau]);
  const quatorze = useMemo(() => (tableau ? derniersJours(tableau.jours, 14) : []), [tableau]);
  const semaine = totaux(quatorze.slice(-7));
  const avant = totaux(quatorze.slice(0, 7));
  const total = totaux(tableau?.jours ?? []);
  const serie = tableau ? serieDeJours(tableau.jours) : 0;
  const rapport = indexation?.dernier;
  const planning = usePlanning(indexation?.dernier);
  const dues = resume?.groupes.reduce((t, g) => t + g.dues, 0);
  const vues = tableau ? tableau.cartes.filter((c) => c.etatFsrs).length : null;
  const reussite = (t: typeof semaine) => (t.revisions ? (t.justes / t.revisions) * 100 : null);
  const reussiteSemaine = reussite(semaine);
  const reussiteAvant = reussite(avant);
  const copies = notes.slice(0, 8);

  const chiffres: Chiffre[] = [
    { titre: "Série", icone: Flame, valeur: tableau ? serie : null, format: (n) => `${Math.round(n)}`, detail: `jour${serie > 1 ? "s" : ""} d'affilée` },
    {
      titre: "Réussite · 7 jours",
      icone: Target,
      valeur: reussiteSemaine,
      format: (n) => `${Math.round(n)} %`,
      detail: `${semaine.revisions} réponse${semaine.revisions > 1 ? "s" : ""}`,
      tendance:
        reussiteSemaine !== null && reussiteAvant !== null
          ? { valeur: Math.round(reussiteSemaine - reussiteAvant), unite: " pts" }
          : null,
    },
    {
      titre: "Temps · 7 jours",
      icone: Clock,
      valeur: tableau ? semaine.dureeMs : null,
      format: (n) => formaterDuree(n),
      detail: `${formaterDuree(total.dureeMs)} au total`,
      tendance: (() => {
        const e = evolution(avant.dureeMs, semaine.dureeMs);
        return e === null ? null : { valeur: e, unite: " %" };
      })(),
    },
    {
      titre: "Cartes vues",
      icone: Layers,
      valeur: vues,
      format: (n) => `${Math.round(n)}`,
      detail: `sur ${tableau?.cartes.length ?? "—"} cartes`,
    },
  ];

  return (
    <div className="page page-large accueil">
      <header className="entete-accueil apparait">
        <div>
          <p className="sur-titre">Tableau de bord</p>
          <h1>{salutation()}, content de te revoir</h1>
          <p className="texte-doux">
            {formatDuJour.format(new Date())}
            {dues !== undefined && ` · ${dues} carte${dues > 1 ? "s" : ""} t'attend${dues > 1 ? "ent" : ""} aujourd'hui`}
          </p>
        </div>
        <button className="bouton" onClick={onRapport}>
          <Download size={16} className="icone" aria-hidden /> Exporter ma progression
        </button>
      </header>

      <section className="grille-chiffres bloc">
        {chiffres.map((c, i) => (
          <CarteChiffre key={c.titre} c={c} rang={i + 1} />
        ))}
      </section>

      <section className="deux-seances bloc">
        {(["langues", "autres"] as GroupeRevision[]).map((g, i) => {
          const rg = resume?.groupes.find((x) => x.groupe === g);
          const Icone = g === "langues" ? Languages : BookOpen;
          const nouvelles = rg ? Math.min(rg.nouvellesRestantes, rg.nouvellesDisponibles) : 0;
          return (
            <div key={g} className="carte seance-du-jour apparait survol-leve" style={{ "--i": i + 5 } as React.CSSProperties}>
              <div className="seance-icone">
                <Icone size={20} strokeWidth={1.8} aria-hidden />
              </div>
              <div className="seance-texte">
                <h2>{NOMS_GROUPES[g]}</h2>
                <p className="texte-doux petit">
                  {rg ? (
                    <>
                      <strong className="seance-nombre">{rg.dues}</strong> à revoir · {nouvelles} nouvelle{nouvelles > 1 ? "s" : ""} possible
                      {nouvelles > 1 ? "s" : ""}
                    </>
                  ) : (
                    "…"
                  )}
                </p>
              </div>
              <button className="bouton bouton-principal" onClick={() => onSeanceDuJour(g)}>
                Séance du jour <ArrowRight size={16} className="icone fleche" aria-hidden />
              </button>
            </div>
          );
        })}
      </section>

      <section className="carte-sombre bloc apparait" style={{ "--i": 7 } as React.CSSProperties}>
        <div className="rangee entre">
          <h2>Activité des 14 derniers jours</h2>
          <span className="texte-doux petit">
            {totaux(quatorze).revisions} réponses · {formaterDuree(totaux(quatorze).dureeMs)}
          </span>
        </div>
        <CourbeActivite jours={quatorze} />
      </section>

      {planning.evaluations.length > 0 && (
        <section className="carte bloc evaluations-a-venir apparait" style={{ "--i": 8 } as React.CSSProperties}>
          <div className="rangee entre">
            <h2>Prochaines évaluations</h2>
            <button className="lien-discret" onClick={onPlanning}>
              Voir le planning
            </button>
          </div>
          <ul className="liste-plan">
            {planning.evaluations.slice(0, 4).map((ev) => (
              <li key={ev.id}>
                <span className="rangee">
                  <CalendarClock size={16} className="icone texte-doux" aria-hidden />
                  <strong>{ev.titre}</strong>
                  <span className="texte-doux petit">
                    {NOMS_EVALUATIONS[ev.genre]} · {ev.chapitres.length} chapitre{ev.chapitres.length > 1 ? "s" : ""}
                  </span>
                </span>
                <span className="compte-a-rebours">{compteARebours(ev.jour, planning.aujourdhui)}</span>
              </li>
            ))}
          </ul>
          {planning.duJour.length > 0 && (
            <p className="petit">
              Au programme aujourd'hui :{" "}
              {planning.duJour.map((c) => `${c.matiere} › ${c.chapitre ?? "notes générales"}`).join(" ; ")} (en
              premier dans tes séances du jour).
            </p>
          )}
        </section>
      )}

      <section className="carte bloc apparait" style={{ "--i": 9 } as React.CSSProperties}>
        <h2>Maîtrise par matière</h2>
        <p className="texte-doux petit legende-statuts">
          Probabilité moyenne de te souvenir des cartes aujourd'hui (une carte jamais vue compte 0) :
          <span>
            <Point classe="statut-solide" /> solide dès 80 %
          </span>
          <span>
            <Point classe="statut-fragile" /> fragile dès 50 %
          </span>
          <span>
            <Point classe="statut-a-revoir" /> à revoir en dessous
          </span>
        </p>
        <div className="liste-maitrise">
          {maitrise.map((m) => (
            <details key={m.matiere} className="maitrise">
              <summary>
                <span className="maitrise-nom">{m.matiere}</span>
                <Jauge m={m} />
              </summary>
              {m.chapitres.map((c) => (
                <div key={c.chapitre} className="maitrise maitrise-chapitre">
                  <span className="maitrise-nom">{c.chapitre}</span>
                  <Jauge m={c} />
                </div>
              ))}
            </details>
          ))}
        </div>
      </section>

      {copies.length > 0 && (
        <section className="carte-sombre bloc apparait" style={{ "--i": 10 } as React.CSSProperties}>
          <div className="rangee entre">
            <h2>Exercices et examens</h2>
            <span className="texte-doux petit">
              Moyenne des {copies.length} dernières copies :{" "}
              <strong className="texte-clair">{formatNote.format(copies.reduce((t, n) => t + n.note, 0) / copies.length)} / 20</strong>
            </span>
          </div>
          <table className="tableau-sombre">
            <thead>
              <tr>
                <th>Sujet</th>
                <th>Type</th>
                <th>Matière</th>
                <th>Date</th>
                <th>Note</th>
              </tr>
            </thead>
            <tbody>
              {copies.map((n, i) => (
                <tr key={i}>
                  <td>{n.titre || n.typeNom}</td>
                  <td>{n.typeNom}</td>
                  <td>{n.matiere}</td>
                  <td>{formatJourCourt.format(new Date(n.date))}</td>
                  <td>
                    <Point classe={n.note >= 14 ? "statut-solide" : n.note >= 10 ? "statut-fragile" : "statut-a-revoir"} />{" "}
                    {formatNote.format(n.note)} / 20
                  </td>
                </tr>
              ))}
            </tbody>
          </table>
          <button className="lien-discret petit lien-clair" onClick={onExercices}>
            Voir les sujets et leurs corrections
          </button>
        </section>
      )}

      <section className="carte bloc apparait" style={{ "--i": 11 } as React.CSSProperties}>
        <h2>Notions les plus faibles</h2>
        {tableau && tableau.faibles.length === 0 ? (
          <p className="texte-doux">Rien à signaler pour l'instant : les cartes ratées ou oubliées apparaîtront ici.</p>
        ) : (
          <table className="tableau-donnees">
            <thead>
              <tr>
                <th>Notion</th>
                <th>Type</th>
                <th>Chapitre</th>
                <th>Erreurs</th>
                <th aria-label="Action" />
              </tr>
            </thead>
            <tbody>
              {tableau?.faibles.map((f) => (
                <tr key={f.carteId}>
                  <td className="cellule-notion">
                    <TexteMaths texte={f.genre === "trou" ? f.reponse : f.genre === "ordre" ? f.section : f.question} />
                  </td>
                  <td className="texte-doux">{NOMS_GENRES[f.genre]}</td>
                  <td className="texte-doux">{[f.matiere, f.chapitre].filter(Boolean).join(" › ")}</td>
                  <td>
                    <Point classe={f.erreurs + f.oublis >= 3 ? "statut-a-revoir" : "statut-fragile"} /> {f.erreurs} · {f.oublis}{" "}
                    <span className="texte-doux petit">oubli{f.oublis > 1 ? "s" : ""}</span>
                  </td>
                  <td>
                    <button className="bouton bouton-icone" title="Relire la note" onClick={() => onOuvrirNote(f.noteChemin, f.section || null)}>
                      <BookOpen size={15} aria-hidden />
                      <span className="sr-only">Relire la note</span>
                    </button>
                  </td>
                </tr>
              ))}
            </tbody>
          </table>
        )}
      </section>

      <section className="carte bloc apparait" style={{ "--i": 12 } as React.CSSProperties}>
        <div className="rangee entre">
          <h2>Mes notes</h2>
          <button className="bouton" disabled={indexation?.enCours} onClick={() => api.reindexer()}>
            <RefreshCw size={15} className={"icone" + (indexation?.enCours ? " tourne" : "")} aria-hidden /> Réindexer
          </button>
        </div>
        {indexation?.enCours && <p className="texte-doux">Lecture des notes en cours…</p>}
        {indexation?.erreur && <p className="message-erreur">{indexation.erreur}</p>}
        {rapport && !indexation?.enCours && (
          <p className="texte-doux petit">
            {rapport.total} notes et {rapport.fichiersAnnexes} images indexées en {rapport.dureeMs} ms
            {indexation?.cartes && ` · ${indexation.cartes.actives} cartes de révision`}.
          </p>
        )}
        {rapport && rapport.erreurs.length > 0 && (
          <details className="petit">
            <summary className="message-erreur">{rapport.erreurs.length} fichier(s) illisible(s)</summary>
            <ul>
              {rapport.erreurs.map((e) => (
                <li key={e}>{e}</li>
              ))}
            </ul>
          </details>
        )}
      </section>
    </div>
  );
}
