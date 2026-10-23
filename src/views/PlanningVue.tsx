// Planning : dates d'évaluation et répartition des chapitres à réviser sur les jours restants.
// Tout fonctionne hors ligne ; seul le bouton « examen blanc » demande Gemini.
import { useEffect, useMemo, useState } from "react";
import { ArrowRight, CalendarCheck, CalendarDays, CalendarPlus, ClipboardList, GraduationCap, Pencil, Save, Trash } from "lucide-react";
import EntetePage from "../composants/EntetePage";
import {
  api,
  ChapitreChoisi,
  ChapitreIa,
  Evaluation,
  GenreEvaluation,
  messageErreur,
  NOMS_EVALUATIONS,
} from "../lib/api";
import { cleChapitre, compteARebours, ecartJours, ElementPlan, libelleJour } from "../planning/planning";
import ChoixDate from "../composants/ChoixDate";
import { usePlanning } from "../planning/usePlanning";
import Calendrier from "../planning/Calendrier";
import { couleurMatiere } from "../planning/grilleCalendrier";
import { formaterDuree } from "../revision/stats";

interface Props {
  onReviserChapitre: (c: ChapitreChoisi) => void;
  onExamen: (chapitres: ChapitreChoisi[]) => void;
}

const nomChapitre = (c: string | null) => c ?? "Notes générales";
const GENRES = Object.keys(NOMS_EVALUATIONS) as GenreEvaluation[];

const vide = (aujourdhui: string): Evaluation => ({
  id: 0,
  jour: aujourdhui,
  matiere: "",
  titre: "",
  genre: "controle",
  chapitres: [],
});

/** Les éléments d'un jour, regroupés par évaluation. */
function parEvaluation(elements: ElementPlan[]): [Evaluation, ElementPlan[]][] {
  const groupes = new Map<number, [Evaluation, ElementPlan[]]>();
  for (const e of elements) {
    const g = groupes.get(e.evaluation.id) ?? [e.evaluation, []];
    g[1].push(e);
    groupes.set(e.evaluation.id, g);
  }
  return [...groupes.values()];
}

function libelleNature(e: ElementPlan): string {
  if (e.nature === "jour_j") return "dernier coup d'œil";
  if (e.nature === "revision_generale") return "révision générale";
  return e.passage > 1 ? `${e.passage}e passage` : "1er passage";
}

export default function PlanningVue({ onReviserChapitre, onExamen }: Props) {
  const { charge, evaluations, revues, jours, infos, plan, aujourdhui, charger } = usePlanning();
  const [choisi, setChoisi] = useState(aujourdhui);
  const [chapitres, setChapitres] = useState<ChapitreIa[]>([]);
  const [formulaire, setFormulaire] = useState<Evaluation | null>(null);
  const [aSupprimer, setASupprimer] = useState<number | null>(null);
  const [erreur, setErreur] = useState<string | null>(null);

  useEffect(() => {
    api.chapitresIa().then(setChapitres).catch((e) => setErreur(messageErreur(e)));
  }, []);

  const matieres = useMemo(() => [...new Set(chapitres.map((c) => c.matiere))], [chapitres]);
  const revuesDe = (matiere: string, chapitre: string | null) =>
    revues.find((r) => r.matiere === matiere && r.chapitre === chapitre)?.cartes ?? 0;
  const planChoisi = plan.find((j) => j.jour === choisi);
  const evaluationsChoisies = evaluations.filter((ev) => ev.jour === choisi);
  const activiteChoisie = jours.find((j) => j.jour === choisi);

  function choisirMatiere(matiere: string) {
    if (!formulaire) return;
    // Par défaut, tous les chapitres de la matière sont à réviser.
    setFormulaire({ ...formulaire, matiere, chapitres: chapitres.filter((c) => c.matiere === matiere).map((c) => c.chapitre) });
  }

  async function enregistrer() {
    if (!formulaire) return;
    const titre = formulaire.titre.trim() || `${NOMS_EVALUATIONS[formulaire.genre]} de ${formulaire.matiere}`;
    try {
      await api.enregistrerEvaluation({ ...formulaire, titre });
      setFormulaire(null);
      setErreur(null);
      charger();
    } catch (e) {
      setErreur(messageErreur(e));
    }
  }

  async function supprimer(id: number) {
    setASupprimer(null);
    try {
      await api.supprimerEvaluation(id);
      charger();
    } catch (e) {
      setErreur(messageErreur(e));
    }
  }

  const basculerChapitre = (c: string | null, coche: boolean) =>
    formulaire &&
    setFormulaire({
      ...formulaire,
      chapitres: coche ? [...formulaire.chapitres, c] : formulaire.chapitres.filter((x) => x !== c),
    });

  return (
    <div className="page page-large">
      <EntetePage
        icone={CalendarDays}
        surTitre="Réviser"
        titre="Planning"
        description="Ton calendrier de révision : les évaluations, et les chapitres à revoir chaque jour jusque-là."
      />
      {erreur && <p className="message-erreur">{erreur}</p>}

      <section className="carte bloc apparait" style={{ "--i": 1 } as React.CSSProperties}>
        {charge ? (
          <Calendrier
            plan={plan}
            evaluations={evaluations}
            activite={jours}
            aujourdhui={aujourdhui}
            choisi={choisi}
            onChoisir={setChoisi}
          />
        ) : (
          <p className="texte-doux">Chargement…</p>
        )}
      </section>

      <section className="carte bloc apparait detail-jour" key={choisi} style={{ "--i": 2 } as React.CSSProperties}>
        <div className="rangee entre">
          <h2 className="titre-bloc">
            <CalendarCheck size={18} className="icone" aria-hidden /> {libelleJour(choisi, aujourdhui)}
          </h2>
          {choisi >= aujourdhui && !formulaire && (
            <button className="bouton" onClick={() => setFormulaire({ ...vide(aujourdhui), jour: choisi })}>
              <CalendarPlus size={15} aria-hidden /> Ajouter une évaluation ce jour-là
            </button>
          )}
        </div>

        {activiteChoisie && activiteChoisie.revisions > 0 && (
          <p className="texte-doux petit">
            {activiteChoisie.revisions} réponse{activiteChoisie.revisions > 1 ? "s" : ""} ·{" "}
            {Math.round((activiteChoisie.justes / activiteChoisie.revisions) * 100)} % justes · {formaterDuree(activiteChoisie.dureeMs)} de
            révision
          </p>
        )}

        {evaluationsChoisies.map((ev) => (
          <div key={ev.id} className="eval-detail" style={{ "--couleur": couleurMatiere(ev.matiere) } as React.CSSProperties}>
            <GraduationCap size={17} aria-hidden />
            <div>
              <strong>{ev.titre}</strong>
              <div className="texte-doux petit">
                {NOMS_EVALUATIONS[ev.genre]} · {ev.matiere} · {ev.chapitres.map(nomChapitre).join(", ")}
              </div>
            </div>
          </div>
        ))}

        {!planChoisi ? (
          evaluationsChoisies.length === 0 && (
            <p className="texte-doux">
              {choisi < aujourdhui
                ? "Rien n'était prévu ce jour-là."
                : evaluations.length
                  ? "Rien de prévu ce jour-là pour tes évaluations : fais simplement tes séances du jour."
                  : "Ajoute tes dates d'évaluation : Révisio répartira les chapitres à réviser sur les jours restants."}
            </p>
          )
        ) : (
          <>
            {choisi === aujourdhui && (
              <p className="texte-doux petit">
                Ces chapitres passent aussi en premier dans tes séances du jour, même si leurs cartes ne sont pas encore
                à revoir.
              </p>
            )}
            {parEvaluation(planChoisi.elements).map(([ev, elements]) => (
              <div key={ev.id} className="plan-evaluation">
                <div className="rangee entre">
                  <strong className="rangee">
                    <span className="point-matiere" style={{ background: couleurMatiere(ev.matiere) }} aria-hidden />
                    {ev.titre} <span className="texte-doux">· {compteARebours(ev.jour, aujourdhui)}</span>
                  </strong>
                  {elements[0].nature !== "chapitre" && choisi === aujourdhui && (
                    <button
                      className="bouton"
                      onClick={() => onExamen(elements.map((e) => ({ matiere: ev.matiere, chapitre: e.chapitre })))}
                    >
                      <ClipboardList size={15} aria-hidden /> Examen blanc sur ces chapitres
                    </button>
                  )}
                </div>
                <ul className="liste-plan">
                  {elements.map((e) => {
                    const info = infos.get(cleChapitre(ev.matiere, e.chapitre));
                    const faites = choisi === aujourdhui ? revuesDe(ev.matiere, e.chapitre) : 0;
                    return (
                      <li key={`${e.chapitre}`}>
                        <div>
                          <div>
                            {nomChapitre(e.chapitre)} <span className="texte-doux petit">· {libelleNature(e)}</span>
                          </div>
                          <div className="texte-doux petit">
                            {info
                              ? `${info.total} cartes · maîtrise ${Math.round(info.maitrise * 100)} %` +
                                (choisi === aujourdhui
                                  ? ` · ${Math.min(faites, info.total)} revue${faites > 1 ? "s" : ""} aujourd'hui`
                                  : "")
                              : "Pas encore de cartes : relis tes notes ou crée des questions dans l'Atelier IA."}
                          </div>
                          {info && choisi === aujourdhui && (
                            <div className="progression petite-progression">
                              <div
                                className="progression-barre"
                                style={{ width: `${Math.min(100, (faites / Math.max(info.total, 1)) * 100)}%` }}
                              />
                            </div>
                          )}
                        </div>
                        {choisi >= aujourdhui && (
                          <button
                            className={"bouton" + (choisi === aujourdhui ? " bouton-principal" : "")}
                            disabled={!info}
                            onClick={() => onReviserChapitre({ matiere: ev.matiere, chapitre: e.chapitre })}
                          >
                            {choisi === aujourdhui ? "Réviser ce chapitre" : "Réviser en avance"}{" "}
                            <ArrowRight size={15} className="icone fleche" aria-hidden />
                          </button>
                        )}
                      </li>
                    );
                  })}
                </ul>
              </div>
            ))}
          </>
        )}
      </section>

      <section className="carte bloc apparait" style={{ "--i": 3 } as React.CSSProperties}>
        <div className="rangee entre">
          <h2 className="titre-bloc">
            <CalendarDays size={18} className="icone" aria-hidden /> Mes évaluations
          </h2>
          {!formulaire && (
            <button className="bouton bouton-principal" onClick={() => setFormulaire(vide(aujourdhui))}>
              <CalendarPlus size={16} aria-hidden /> Ajouter une évaluation
            </button>
          )}
        </div>

        {formulaire && (
          <div className="formulaire-evaluation apparait">
            <div className="grille-reglages">
              <label htmlFor="eval-jour">Date</label>
              <ChoixDate
                id="eval-jour"
                valeur={formulaire.jour}
                min={aujourdhui}
                aujourdhui={aujourdhui}
                marques={new Map(evaluations.map((ev) => [ev.jour, ev.titre]))}
                onChange={(jour) => setFormulaire({ ...formulaire, jour })}
              />
              <label htmlFor="eval-matiere">Matière</label>
              <select id="eval-matiere" value={formulaire.matiere} onChange={(e) => choisirMatiere(e.target.value)}>
                <option value="">Choisis…</option>
                {matieres.map((m) => (
                  <option key={m} value={m}>
                    {m}
                  </option>
                ))}
              </select>
              <label htmlFor="eval-genre">Type</label>
              <select
                id="eval-genre"
                value={formulaire.genre}
                onChange={(e) => setFormulaire({ ...formulaire, genre: e.target.value as GenreEvaluation })}
              >
                {GENRES.map((g) => (
                  <option key={g} value={g}>
                    {NOMS_EVALUATIONS[g]}
                  </option>
                ))}
              </select>
              <label htmlFor="eval-titre">Titre (facultatif)</label>
              <input
                id="eval-titre"
                className="champ"
                value={formulaire.titre}
                placeholder={formulaire.matiere ? `${NOMS_EVALUATIONS[formulaire.genre]} de ${formulaire.matiere}` : ""}
                onChange={(e) => setFormulaire({ ...formulaire, titre: e.target.value })}
              />
            </div>
            {formulaire.matiere && (
              <fieldset className="chapitres-evaluation">
                <legend>Chapitres à réviser</legend>
                {chapitres
                  .filter((c) => c.matiere === formulaire.matiere)
                  .map((c) => (
                    <label key={cleChapitre(c.matiere, c.chapitre)} className="case">
                      <input
                        type="checkbox"
                        checked={formulaire.chapitres.includes(c.chapitre)}
                        onChange={(e) => basculerChapitre(c.chapitre, e.target.checked)}
                      />
                      {nomChapitre(c.chapitre)}
                    </label>
                  ))}
              </fieldset>
            )}
            <div className="rangee bloc-haut">
              <button
                className="bouton bouton-principal"
                disabled={!formulaire.matiere || !formulaire.jour || formulaire.chapitres.length === 0}
                onClick={enregistrer}
              >
                <Save size={15} aria-hidden /> Enregistrer
              </button>
              <button className="bouton" onClick={() => setFormulaire(null)}>
                Annuler
              </button>
            </div>
          </div>
        )}

        {charge && evaluations.length === 0 && !formulaire && <p className="texte-doux">Aucune évaluation à venir.</p>}
        <ul className="liste-sujets">
          {evaluations.map((ev) => (
            <li key={ev.id}>
              <div className="sujet-infos">
                <strong>{ev.titre}</strong>
                <span className="texte-doux petit">
                  {libelleJour(ev.jour, aujourdhui)}
                  {ecartJours(aujourdhui, ev.jour) > 1 ? ` (${compteARebours(ev.jour, aujourdhui)})` : ""} ·{" "}
                  {NOMS_EVALUATIONS[ev.genre]} ·{" "}
                  {ev.chapitres.map(nomChapitre).join(", ")}
                </span>
              </div>
              <div className="rangee">
                <button className="bouton" onClick={() => setFormulaire(ev)}>
                  <Pencil size={14} aria-hidden /> Modifier
                </button>
                {aSupprimer === ev.id ? (
                  <>
                    <button className="bouton bouton-danger" onClick={() => supprimer(ev.id)}>
                      <Trash size={14} aria-hidden /> Supprimer l'évaluation
                    </button>
                    <button className="lien-discret petit" onClick={() => setASupprimer(null)}>
                      Annuler
                    </button>
                  </>
                ) : (
                  <button className="lien-discret petit" onClick={() => setASupprimer(ev.id)}>
                    <Trash size={13} aria-hidden /> Supprimer
                  </button>
                )}
              </div>
            </li>
          ))}
        </ul>
      </section>
    </div>
  );
}
