import { useEffect, useState } from "react";
import { ArrowRight, BookOpen, CalendarOff, Crosshair, Keyboard, Languages, Layers, PartyPopper } from "lucide-react";
import EntetePage from "../composants/EntetePage";
import Message from "../composants/Message";
import {
  api,
  CarteSeance,
  ChapitreChoisi,
  ChapitreSeance,
  debutDuJour,
  EtatIndexation,
  GenreCarte,
  GroupeRevision,
  messageErreur,
  NOMS_GROUPES,
  Reglages,
  ResumeRevision,
  Seance,
} from "../lib/api";
import { cleChapitre, concerneGroupe } from "../planning/planning";
import { usePlanning } from "../planning/usePlanning";
import { NOMS_GENRES } from "../revision/modules";
import { TexteMaths } from "../revision/symboles";
import SeanceVue, { Bilan } from "./SeanceVue";
import ChoixChapitres, { cleChapitreSeance } from "../revision/ChoixChapitres";

/** Séance demandée depuis un autre écran : séance du jour d'un groupe, ou un chapitre (planning, Atelier IA :
 *  `genres` limite aux questions de l'IA par exemple). */
export type DemandeSeance = { groupe: GroupeRevision } | { chapitre: ChapitreChoisi; genres?: GenreCarte[] };

interface Props {
  reglages: Reglages;
  indexation: EtatIndexation | null;
  /** Demande venue du tableau de bord ou du planning (nouvel objet à chaque demande). */
  demandeSeanceDuJour?: DemandeSeance | null;
  /** Réglages modifiés depuis cet écran (matières exclues des séances du jour). */
  onReglages: (r: Reglages) => void;
}

export function dureeDuGroupe(reglages: Reglages, groupe: GroupeRevision): number {
  return groupe === "langues" ? reglages.dureeSeanceLangues : reglages.dureeSeance;
}

const trieur = new Intl.Collator("fr", { numeric: true });

function melanger<T>(liste: T[]): T[] {
  const copie = [...liste];
  for (let i = copie.length - 1; i > 0; i--) {
    const j = Math.floor(Math.random() * (i + 1));
    [copie[i], copie[j]] = [copie[j], copie[i]];
  }
  return copie;
}

/**
 * Ordre de la séance : chapitres du planning (avant une évaluation), cartes dues, points faibles,
 * puis nouveautés ; chaque groupe est mélangé si l'entrelacement est activé, sinon rangé par
 * matière et chapitre. Une carte n'apparaît qu'une fois.
 * `limite` : nombre maximal de cartes (séance du jour en temps limité).
 */
export function ordonner(
  s: Pick<Seance, "planifiees" | "dues" | "faibles" | "nouvelles">,
  entrelacement: boolean,
  limite = Infinity,
): CarteSeance[] {
  const parChapitre = (a: CarteSeance, b: CarteSeance) =>
    trieur.compare(a.matiere ?? "", b.matiere ?? "") ||
    trieur.compare(a.chapitre ?? "", b.chapitre ?? "") ||
    trieur.compare(a.noteChemin, b.noteChemin);
  const ranger = (l: CarteSeance[]) => (entrelacement ? melanger(l) : [...l].sort(parChapitre));
  const groupes = [
    entrelacement ? melanger(s.planifiees) : s.planifiees,
    ranger(s.dues),
    s.faibles,
    entrelacement ? melanger(s.nouvelles) : s.nouvelles,
  ];
  const vues = new Set<number>();
  return groupes
    .flat()
    .filter((c) => !vues.has(c.id) && vues.add(c.id))
    .slice(0, limite);
}

/** Nombre de cartes qui tiennent dans la séance du jour. */
export function cartesPourDuree(minutes: number, dureeMoyenneMs: number): number {
  return Math.max(5, Math.floor((minutes * 60_000) / Math.max(dureeMoyenneMs, 1)));
}

export default function ReviserVue({ reglages, indexation, demandeSeanceDuJour, onReglages }: Props) {
  const [resume, setResume] = useState<ResumeRevision | null>(null);
  /** Séance ciblée : chapitres cochés (clés de `cleChapitreSeance`) ; aucun = toutes les matières. */
  const [choisis, setChoisis] = useState<Set<string>>(new Set());
  /** Séance ciblée sur des chapitres : toutes leurs cartes (même pas encore à revoir), ou seulement les dues. */
  const [toutRevoir, setToutRevoir] = useState(true);
  const [genres, setGenres] = useState<GenreCarte[]>([]);
  const [cartes, setCartes] = useState<CarteSeance[] | null>(null);
  const [bilan, setBilan] = useState<Bilan | null>(null);
  const [message, setMessage] = useState<string | null>(null);

  const planning = usePlanning(bilan);

  const recharger = () => api.resumeRevision().then(setResume).catch((e) => setMessage(messageErreur(e)));

  useEffect(() => {
    if (!indexation?.enCours) recharger();
  }, [indexation?.enCours, indexation?.cartes]);

  useEffect(() => {
    if (!demandeSeanceDuJour) return;
    if ("groupe" in demandeSeanceDuJour) commencer(demandeSeanceDuJour.groupe);
    else reviserChapitre(demandeSeanceDuJour.chapitre, demandeSeanceDuJour.genres);
  }, [demandeSeanceDuJour]);

  async function lancer(preparation: Promise<Seance>, minutes: number | null) {
    setMessage(null);
    setBilan(null);
    try {
      const [s, r] = await Promise.all([preparation, api.resumeRevision()]);
      const limite = minutes ? cartesPourDuree(minutes, r.dureeMoyenneMs) : Infinity;
      const liste = ordonner(s, reglages.entrelacement, limite);
      if (!liste.length) setMessage("Rien à réviser pour l'instant avec ces filtres. Reviens plus tard !");
      else setCartes(liste);
    } catch (e) {
      setMessage(messageErreur(e));
    }
  }

  /** `groupe` : séance du jour « Langues » ou « Autres matières » ; `null` : séance ciblée. */
  async function commencer(groupe: GroupeRevision | null) {
    // Les chapitres du planning d'aujourd'hui passent en premier dans la séance du jour
    // (la base ne garde que les cartes du groupe et des matières incluses).
    const planifies: ChapitreSeance[] = groupe ? await planning.chapitresDuJour() : [];
    const chapitres = groupe ? [] : chapitresChoisis;
    await lancer(
      api.preparerSeance({
        genres: groupe ? [] : genres,
        debutJour: debutDuJour(),
        groupe,
        chapitres,
        // Séance ciblée « tout revoir » : toutes les cartes des chapitres cochés passent, même pas encore dues.
        planifies: groupe ? planifies : toutRevoir ? chapitres : [],
      }),
      groupe ? dureeDuGroupe(reglages, groupe) : null,
    );
  }

  /** Inclut ou exclut une matière des deux séances du jour (réglage enregistré). */
  async function basculerMatiere(m: string) {
    const exclues = reglages.matieresExclues;
    const nouvelles = exclues.includes(m) ? exclues.filter((x) => x !== m) : [...exclues, m];
    try {
      onReglages(await api.definirMatieresExclues(nouvelles));
      recharger();
    } catch (e) {
      setMessage(messageErreur(e));
    }
  }

  /** Tout un chapitre (même les cartes pas encore à revoir), éventuellement d'un seul type de cartes. */
  async function reviserChapitre(c: ChapitreChoisi, genresVoulus: GenreCarte[] = []) {
    await lancer(
      api.preparerSeance({
        // `chapitres` (et non matière + chapitre) : « notes générales » (chapitre nul) ne prend pas toute la matière.
        chapitres: [c],
        genres: genresVoulus,
        debutJour: debutDuJour(),
        groupe: null,
        planifies: [c],
      }),
      reglages.dureeSeance,
    );
  }

  if (cartes) {
    return (
      <SeanceVue
        cartes={cartes}
        reglages={reglages}
        onFin={(b) => {
          setCartes(null);
          setBilan(b);
          recharger();
        }}
      />
    );
  }

  const genresDisponibles = (resume?.parGenre ?? []).map(([g]) => g);
  const chapitresChoisis: ChapitreSeance[] = (resume?.chapitres ?? [])
    .filter((c) => choisis.has(cleChapitreSeance(c)))
    .map(({ matiere, chapitre }) => ({ matiere, chapitre }));
  const cartesChoisies = (resume?.chapitres ?? []).filter((c) => choisis.has(cleChapitreSeance(c)));
  const exclues = reglages.matieresExclues;

  return (
    <div className="page">
      <EntetePage
        icone={Layers}
        surTitre="Réviser"
        titre="Séances de révision"
        description="Les cartes reviennent au bon moment grâce à la répétition espacée (FSRS)."
      />

      {bilan && <BilanSeance bilan={bilan} />}

      {(["langues", "autres"] as GroupeRevision[]).map((g) => {
        const rg = resume?.groupes.find((x) => x.groupe === g);
        const duree = dureeDuGroupe(reglages, g);
        const planDuGroupe = planning.duJour.filter(
          (c) => !exclues.includes(c.matiere) && concerneGroupe(planning.infos.get(cleChapitre(c.matiere, c.chapitre)), g),
        );
        const Icone = g === "langues" ? Languages : BookOpen;
        return (
          <section key={g} className="carte bloc seance-du-jour apparait survol-leve" style={{ "--i": g === "langues" ? 1 : 2 } as React.CSSProperties}>
            <div className="seance-icone">
              <Icone size={20} strokeWidth={1.8} aria-hidden />
            </div>
            <div className="seance-texte">
              <h2>Séance du jour — {NOMS_GROUPES[g]}</h2>
              {rg && resume ? (
                <p className="texte-doux">
                  {g === "langues" ? "Tout le vocabulaire · " : "Textes à trous, schémas, remises dans l'ordre · "}
                  {planDuGroupe.length > 0 && "d'abord les chapitres du planning, puis "}
                  {rg.dues} à revoir, puis les points faibles, puis {Math.min(rg.nouvellesRestantes, rg.nouvellesDisponibles)}{" "}
                  nouvelle{rg.nouvellesRestantes > 1 ? "s" : ""} · {duree} min environ (
                  {cartesPourDuree(duree, resume.dureeMoyenneMs)} cartes au plus)
                  {exclues.length > 0 && ` · sans ${exclues.join(", ")}`}
                </p>
              ) : (
                <p className="texte-doux">Chargement…</p>
              )}
              {planDuGroupe.length > 0 && (
                <p className="petit">
                  Planning du jour :{" "}
                  {planDuGroupe.map((c) => `${c.matiere} › ${c.chapitre ?? "notes générales"}`).join(" ; ")}
                </p>
              )}
            </div>
            <button className="bouton bouton-principal bouton-large" onClick={() => commencer(g)}>
              Commencer <ArrowRight size={17} className="icone fleche" aria-hidden />
            </button>
          </section>
        );
      })}

      {resume && resume.matieres.length > 0 && (
        <section className="carte bloc apparait matieres-du-jour" style={{ "--i": 3 } as React.CSSProperties}>
          <h2 className="titre-bloc">
            <CalendarOff size={18} className="icone" aria-hidden /> Matières des séances du jour
          </h2>
          <p className="texte-doux petit">
            Clique sur une matière pour la laisser de côté dans les deux séances du jour (et le planning du jour). Ses
            cartes restent disponibles en séance ciblée ; reclique pour la remettre.
          </p>
          <div className="puces-matieres" role="group" aria-label="Matières incluses dans les séances du jour">
            {resume.matieres.map((m) => {
              const incluse = !exclues.includes(m.matiere);
              return (
                <button
                  key={m.matiere}
                  className={"puce-matiere" + (incluse ? " incluse" : "")}
                  aria-pressed={incluse}
                  onClick={() => basculerMatiere(m.matiere)}
                  title={`${m.dues} à revoir · ${incluse ? "incluse : clique pour l'exclure des séances du jour" : "exclue : clique pour la remettre"}`}
                >
                  {m.matiere}
                  <span className="texte-doux" aria-label={`${m.dues} à revoir`}>
                    {m.dues}
                  </span>
                </button>
              );
            })}
          </div>
        </section>
      )}

      <section className="carte bloc apparait" style={{ "--i": 4 } as React.CSSProperties}>
        <h2 className="titre-bloc">
          <Crosshair size={18} className="icone" aria-hidden /> Séance ciblée
        </h2>
        <p className="texte-doux petit">
          Coche des matières entières, ou déplie-les pour choisir des chapitres. Sans rien cocher : toutes les matières.
        </p>
        {resume && <ChoixChapitres chapitres={resume.chapitres} choisis={choisis} onChange={setChoisis} />}
        {choisis.size > 0 && (
          <div className="rangee bloc-haut">
            <div className="segmente" role="radiogroup" aria-label="Cartes des chapitres choisis">
              <button role="radio" aria-checked={toutRevoir} className={"segment" + (toutRevoir ? " actif" : "")} onClick={() => setToutRevoir(true)}>
                Tout revoir
              </button>
              <button role="radio" aria-checked={!toutRevoir} className={"segment" + (!toutRevoir ? " actif" : "")} onClick={() => setToutRevoir(false)}>
                Seulement ce qui est à revoir
              </button>
            </div>
            <span className="texte-doux petit">
              {chapitresChoisis.length} chapitre{chapitresChoisis.length > 1 ? "s" : ""} ·{" "}
              {toutRevoir
                ? `${cartesChoisies.reduce((t, c) => t + c.total, 0)} cartes, même pas encore à revoir`
                : `${cartesChoisies.reduce((t, c) => t + c.dues, 0)} à revoir, puis points faibles et nouvelles`}
            </span>
            <button className="lien-discret petit" onClick={() => setChoisis(new Set())}>
              Tout décocher
            </button>
          </div>
        )}
        <div className="rangee bloc-haut">
          {genresDisponibles.map((g) => (
            <label key={g} className="case">
              <input
                type="checkbox"
                checked={genres.includes(g)}
                onChange={(e) => setGenres((l) => (e.target.checked ? [...l, g] : l.filter((x) => x !== g)))}
              />
              {NOMS_GENRES[g] ?? g}
            </label>
          ))}
          <span className="espace" />
          <button className="bouton" onClick={() => commencer(null)}>
            Commencer la séance ciblée <ArrowRight size={16} className="icone fleche" aria-hidden />
          </button>
        </div>
        <p className="texte-doux petit">Sans type coché, tous les types d'exercices sont inclus.</p>
      </section>

      {message && <Message genre="info">{message}</Message>}

      {resume && (
        <section className="carte bloc apparait" style={{ "--i": 5 } as React.CSSProperties}>
          <h2 className="titre-bloc">
            <Layers size={18} className="icone" aria-hidden /> Mes cartes
          </h2>
          <table className="tableau-donnees">
            <thead>
              <tr>
                <th>Matière</th>
                <th>À revoir</th>
                <th>Jamais vues</th>
                <th>Total</th>
              </tr>
            </thead>
            <tbody>
              {resume.matieres.map((m) => (
                <tr key={m.matiere}>
                  <td>{m.matiere}</td>
                  <td>{m.dues}</td>
                  <td>{m.nouvelles}</td>
                  <td>{m.total}</td>
                </tr>
              ))}
            </tbody>
          </table>
          <p className="texte-doux petit">
            {resume.parGenre.map(([g, n]) => `${NOMS_GENRES[g] ?? g} : ${n}`).join(" · ")}
          </p>
        </section>
      )}

      <section className="aide texte-doux petit rangee">
        <Keyboard size={16} aria-hidden />
        <span>
          <kbd>S</kbd> <kbd>H</kbd> <kbd>A</kbd> confiance (sûr, hésitant, au hasard) · <kbd>Espace</kbd> révéler ·{" "}
          <kbd>1</kbd> à <kbd>4</kbd> note (à revoir, difficile, bien, facile) · <kbd>Entrée</kbd> valider
        </span>
      </section>
    </div>
  );
}

/** Intitulé court d'une carte pour une liste (sans la syntaxe Markdown du trou). */
function resumeQuestion(carte: CarteSeance): string {
  if (carte.genre === "trou") return carte.question.replace("==……==", "……").replace(/\*\*/g, "");
  if (carte.genre === "schema") return `Schéma : ${carte.question}`;
  if (carte.genre === "ordre") return carte.section || carte.question;
  return carte.question;
}

function BilanSeance({ bilan }: { bilan: Bilan }) {
  const minutes = Math.max(1, Math.round(bilan.dureeMs / 60_000));
  const taux = bilan.vues ? Math.round((bilan.justes / bilan.vues) * 100) : 0;
  return (
    <section className="carte-sombre bloc bilan apparait">
      <h2 className="titre-bloc">
        <PartyPopper size={19} className="icone" aria-hidden /> Séance terminée
      </h2>
      <div className="bilan-chiffres">
        <div>
          <span className="bilan-valeur">{bilan.vues}</span>
          <span className="texte-doux">réponse{bilan.vues > 1 ? "s" : ""}</span>
        </div>
        <div>
          <span className="bilan-valeur">{taux} %</span>
          <span className="texte-doux">de réussite</span>
        </div>
        <div>
          <span className="bilan-valeur">{minutes} min</span>
          <span className="texte-doux">de révision</span>
        </div>
      </div>
      {bilan.erreurs.length > 0 && (
        <>
          <p className="texte-doux petit">À retravailler (aussi dans le carnet d'erreurs) :</p>
          <ul className="liste-erreurs">
            {bilan.erreurs.map(({ carte, reponseDonnee }, i) => (
              <li key={i}>
                <strong>
                  <TexteMaths texte={resumeQuestion(carte)} />
                </strong>{" "}
                <ArrowRight size={13} className="icone" aria-hidden /> <TexteMaths texte={carte.reponse} />
                <span className="texte-doux">
                  {" "}
                  (ta réponse : {reponseDonnee ? <TexteMaths texte={reponseDonnee} /> : "—"})
                </span>
              </li>
            ))}
          </ul>
        </>
      )}
    </section>
  );
}
