// Exercices type bac et mode examen. Gemini rédige le sujet à partir des notes (1 appel) ;
// le sujet est enregistré : on peut le refaire hors ligne, seules les rédactions attendent Internet.
import { useEffect, useMemo, useState } from "react";
import { ClipboardCheck, ClipboardList, FilePen, FolderOpen, KeyRound, Play, RotateCcw, Sparkles, Timer, Trash } from "lucide-react";
import EntetePage from "../composants/EntetePage";
import Message from "../composants/Message";
import {
  api,
  ChapitreChoisi,
  ChapitreIa,
  Copie as CopieRendue,
  EtatIa,
  messageErreur,
  ResumeSujet,
  SujetEnregistre,
  TypeExercice,
} from "../lib/api";
import { useEnLigne, useModeLocal } from "../lib/connexion";
import MessageModeLocal from "../composants/MessageModeLocal";
import Copie, { brouillonExiste, effacerBrouillon } from "../exercices/Copie";
import Resultat from "../exercices/Resultat";
import { lancerTache, oublierTache, useDerniereTache } from "../lib/taches";
import { BoutonArreter } from "../composants/TachesIa";

interface Props {
  onReglages: () => void;
  onOuvrirNote: (chemin: string) => void;
  /** Examen blanc demandé depuis le planning : chapitres déjà cochés (nouvel objet à chaque demande). */
  demandeExamen?: { chapitres: ChapitreChoisi[] } | null;
}

type Etape =
  | { nom: "choix" }
  | { nom: "copie"; sujet: SujetEnregistre }
  | { nom: "resultat"; sujet: SujetEnregistre; copie: CopieRendue; corrigerAuto: boolean };

const cle = (c: { matiere: string; chapitre: string | null }) => `${c.matiere}|${c.chapitre ?? ""}`;
const nomChapitre = (c: { chapitre: string | null }) => c.chapitre ?? "Notes générales de la matière";
const formatDate = new Intl.DateTimeFormat("fr-FR", { day: "numeric", month: "short", hour: "2-digit", minute: "2-digit" });
const formatNote = new Intl.NumberFormat("fr-FR", { maximumFractionDigits: 1 });
const DUREES = [15, 30, 45, 60, 90, 120];

export default function ExercicesVue({ onReglages, onOuvrirNote, demandeExamen }: Props) {
  const enLigne = useEnLigne();
  const modeLocal = useModeLocal();
  const [etape, setEtape] = useState<Etape>({ nom: "choix" });
  const [etat, setEtat] = useState<EtatIa | null>(null);
  const [chapitres, setChapitres] = useState<ChapitreIa[]>([]);
  const [sujets, setSujets] = useState<ResumeSujet[]>([]);
  const [choix, setChoix] = useState("");
  const [types, setTypes] = useState<TypeExercice[]>([]);
  const [typeId, setTypeId] = useState("");
  const [examen, setExamen] = useState<Set<string>>(() => new Set((demandeExamen?.chapitres ?? []).map(cle)));
  const [duree, setDuree] = useState(30);
  // Rédaction du sujet en arrière-plan : quand elle se termine (ici ou au retour sur l'écran), la copie s'ouvre.
  const tache = useDerniereTache("sujet");
  const enCours = tache?.statut === "en-cours" ? (tache.cle as "exercice" | "examen") : null;
  const [aSupprimer, setASupprimer] = useState<number | null>(null);
  const [erreur, setErreur] = useState<string | null>(null);

  const charger = () => {
    api.etatIa().then(setEtat).catch(() => setEtat(null));
    api.sujetsEnregistres().then(setSujets).catch((e) => setErreur(messageErreur(e)));
  };

  useEffect(() => {
    charger();
    api.chapitresIa().then(setChapitres).catch((e) => setErreur(messageErreur(e)));
  }, []);

  useEffect(() => {
    if (demandeExamen) setExamen(new Set(demandeExamen.chapitres.map(cle)));
  }, [demandeExamen]);

  const chapitre = useMemo(() => chapitres.find((c) => cle(c) === choix) ?? null, [chapitres, choix]);
  const matieres = [...new Set(chapitres.map((c) => c.matiere))];

  useEffect(() => {
    setTypes([]);
    setTypeId("");
    if (chapitre)
      api
        .typesExercices(chapitre.matiere)
        .then((t) => {
          const liste = t.filter((x) => x.id !== "examen");
          setTypes(liste);
          setTypeId(liste[0]?.id ?? "");
        })
        .catch(() => {});
  }, [chapitre]);

  const disponible = enLigne && !modeLocal && !!etat?.clePresente;

  useEffect(() => {
    // Pendant une copie ou une correction, on ne l'interrompt pas : le sujet s'ouvrira au retour à la liste.
    if (etape.nom !== "choix") return;
    if (tache?.statut === "finie") {
      const sujet = tache.resultat as SujetEnregistre;
      oublierTache(tache.id);
      effacerBrouillon(sujet.id);
      setEtape({ nom: "copie", sujet });
      charger();
    } else if (tache?.statut === "erreur") {
      setErreur(tache.erreur ?? null);
      oublierTache(tache.id);
    }
  }, [tache?.statut, etape.nom]);

  function creer(type: string, choisis: ChapitreChoisi[], dureeMinutes: number | null, genre: "exercice" | "examen") {
    setErreur(null);
    const sur = choisis.length === 1 ? nomChapitre(choisis[0]) : `${choisis.length} chapitres`;
    lancerTache(
      {
        genre: "sujet",
        cle: genre,
        titre: `${genre === "examen" ? "Sujet d'examen" : "Exercice type bac"} · ${sur}`,
        ecran: "exercices",
        resume: () => "Ton sujet est prêt",
      },
      ({ id }) => api.genererSujet(type, choisis, dureeMinutes, id),
    ).catch(() => {});
  }

  async function ouvrir(id: number, voirCorrection: boolean) {
    setErreur(null);
    try {
      const sujet = await api.sujetEnregistre(id);
      if (voirCorrection && sujet.copies[0]) {
        setEtape({ nom: "resultat", sujet, copie: sujet.copies[0], corrigerAuto: false });
      } else {
        setEtape({ nom: "copie", sujet });
      }
    } catch (e) {
      setErreur(messageErreur(e));
    }
  }

  async function supprimer(id: number) {
    setASupprimer(null);
    try {
      await api.supprimerSujet(id);
      effacerBrouillon(id);
      charger();
    } catch (e) {
      setErreur(messageErreur(e));
    }
  }

  if (etape.nom === "copie") {
    const { sujet } = etape;
    return (
      <Copie
        key={`${sujet.id}-${sujet.copies.length}`}
        sujet={sujet}
        ramassageAutomatique={sujet.typeId === "examen"}
        onRendre={async (reponses, dureeMs) => {
          try {
            const copie = await api.rendreCopie(sujet.id, reponses, dureeMs);
            effacerBrouillon(sujet.id);
            setEtape({ nom: "resultat", sujet, copie, corrigerAuto: true });
          } catch (e) {
            setErreur(messageErreur(e));
            throw e;
          }
        }}
        onAbandonner={() => {
          setEtape({ nom: "choix" });
          charger();
        }}
      />
    );
  }

  if (etape.nom === "resultat") {
    const { sujet, copie } = etape;
    return (
      <Resultat
        key={copie.id}
        sujet={sujet}
        copie={copie}
        corrigerAuto={etape.corrigerAuto}
        onCopie={(c) => setEtape({ ...etape, copie: c, corrigerAuto: false })}
        onRefaire={() => {
          effacerBrouillon(sujet.id);
          setEtape({ nom: "copie", sujet: { ...sujet, copies: [copie, ...sujet.copies] } });
        }}
        onRetour={() => {
          setEtape({ nom: "choix" });
          charger();
        }}
        onOuvrirNote={onOuvrirNote}
      />
    );
  }

  const typeChoisi = types.find((t) => t.id === typeId);
  const choisisExamen: ChapitreChoisi[] = chapitres
    .filter((c) => examen.has(cle(c)))
    .map((c) => ({ matiere: c.matiere, chapitre: c.chapitre }));

  return (
    <div className="page">
      <EntetePage
        icone={ClipboardList}
        surTitre="Créer avec l'IA"
        titre="Exercices type bac"
        description="Des sujets rédigés à partir de tes notes, corrigés par Révisio (calculs, QCM) et par l'IA (rédactions)."
      />
      {modeLocal && <MessageModeLocal quoi="la création d'un sujet" />}
      {!enLigne && (
        <Message genre="hors-ligne">
          <strong>Hors ligne</strong> : impossible de créer un nouveau sujet. Les sujets enregistrés plus bas peuvent
          être refaits ; leurs rédactions seront corrigées au retour d'Internet.
        </Message>
      )}
      {enLigne && etat && !etat.clePresente && (
        <Message genre="attention">
          Aucune clé Gemini.{" "}
          <button className="lien-discret" onClick={onReglages}>
            <KeyRound size={13} aria-hidden /> L'ajouter dans les Réglages
          </button>
        </Message>
      )}
      {erreur && <p className="message-erreur">{erreur}</p>}

      <section className="carte bloc apparait" style={{ "--i": 1 } as React.CSSProperties}>
        <h2 className="titre-bloc">
          <FilePen size={18} className="icone" aria-hidden /> Exercice sur un chapitre
        </h2>
        <p className="texte-doux petit">
          Gemini rédige un sujet à partir de tes notes (et de tes fiches de méthodologie quand il y en a). Les réponses
          courtes, les QCM et les calculs sont corrigés par Révisio, sans IA ; en maths, chaque résultat est recalculé
          par Révisio. Les rédactions sont corrigées par l'IA.
        </p>
        <select aria-label="Chapitre de l'exercice" value={choix} onChange={(e) => setChoix(e.target.value)}>
          <option value="">Choisis un chapitre…</option>
          {matieres.map((m) => (
            <optgroup key={m} label={m}>
              {chapitres
                .filter((c) => c.matiere === m)
                .map((c) => (
                  <option key={cle(c)} value={cle(c)}>
                    {nomChapitre(c)}
                  </option>
                ))}
            </optgroup>
          ))}
        </select>
        {types.length > 0 && (
          <div className="types-exercice" role="radiogroup" aria-label="Type d'exercice">
            {types.map((t) => (
              <button
                key={t.id}
                role="radio"
                aria-checked={t.id === typeId}
                className={"type-exercice" + (t.id === typeId ? " choisi" : "")}
                onClick={() => setTypeId(t.id)}
              >
                <strong>{t.nom}</strong>
                <span className="petit texte-doux">{t.description}</span>
                <span className="petit">{t.dureeMinutes} min</span>
              </button>
            ))}
          </div>
        )}
        <div className="rangee bloc-haut">
          <button
            className="bouton bouton-principal"
            disabled={!disponible || !chapitre || !typeChoisi || enCours !== null}
            onClick={() =>
              chapitre &&
              typeChoisi &&
              creer(typeChoisi.id, [{ matiere: chapitre.matiere, chapitre: chapitre.chapitre }], null, "exercice")
            }
          >
            <Sparkles size={16} className={enCours === "exercice" ? "icone pulse" : "icone"} aria-hidden />
            {enCours === "exercice" ? "Rédaction du sujet… (jusqu'à une minute)" : "Créer le sujet"}
          </button>
          {enCours === "exercice" && <BoutonArreter tache={tache} />}
          <span className="texte-doux petit">1 appel à Gemini, puis 1 pour corriger les rédactions</span>
        </div>
      </section>

      <section className="carte bloc apparait" style={{ "--i": 2 } as React.CSSProperties}>
        <h2 className="titre-bloc">
          <Timer size={18} className="icone" aria-hidden /> Mode examen
        </h2>
        <p className="texte-doux petit">
          Un sujet chronométré sur un ou plusieurs chapitres (QCM, réponses courtes, rédactions, calculs), noté sur 20.
          La copie est ramassée automatiquement à la fin du temps.
        </p>
        <div className="choix-chapitres">
          {matieres.map((m) => {
            const liste = chapitres.filter((c) => c.matiere === m);
            const coches = liste.filter((c) => examen.has(cle(c))).length;
            return (
              <details key={m} open={coches > 0}>
                <summary>
                  {m}
                  {coches > 0 && <span className="texte-doux petit"> · {coches} choisi{coches > 1 ? "s" : ""}</span>}
                </summary>
                {liste.map((c) => (
                  <label key={cle(c)} className="case">
                    <input
                      type="checkbox"
                      checked={examen.has(cle(c))}
                      disabled={!examen.has(cle(c)) && examen.size >= 8}
                      onChange={(e) =>
                        setExamen((s) => {
                          const n = new Set(s);
                          if (e.target.checked) n.add(cle(c));
                          else n.delete(cle(c));
                          return n;
                        })
                      }
                    />
                    {nomChapitre(c)}
                  </label>
                ))}
              </details>
            );
          })}
        </div>
        <div className="rangee bloc-haut">
          <label className="rangee">
            Durée
            <select aria-label="Durée de l'examen" value={duree} onChange={(e) => setDuree(Number(e.target.value))}>
              {DUREES.map((d) => (
                <option key={d} value={d}>
                  {d < 60 ? `${d} min` : `${Math.floor(d / 60)} h${d % 60 ? ` ${d % 60}` : ""}`}
                </option>
              ))}
            </select>
          </label>
          <button
            className="bouton bouton-principal"
            disabled={!disponible || choisisExamen.length === 0 || enCours !== null}
            onClick={() => creer("examen", choisisExamen, duree, "examen")}
          >
            <Sparkles size={16} className={enCours === "examen" ? "icone pulse" : "icone"} aria-hidden />
            {enCours === "examen" ? "Rédaction du sujet… (jusqu'à une minute)" : "Créer le sujet d'examen"}
          </button>
          {enCours === "examen" && <BoutonArreter tache={tache} />}
          <span className="texte-doux petit">
            {choisisExamen.length} chapitre{choisisExamen.length > 1 ? "s" : ""} (8 au plus)
          </span>
        </div>
      </section>

      <section className="carte bloc apparait" style={{ "--i": 3 } as React.CSSProperties}>
        <h2 className="titre-bloc">
          <FolderOpen size={18} className="icone" aria-hidden /> Sujets enregistrés
        </h2>
        {sujets.length === 0 ? (
          <p className="texte-doux">Aucun sujet pour l'instant.</p>
        ) : (
          <ul className="liste-sujets liste-elements">
            {sujets.map((s) => (
              <li key={s.id}>
                <div className="sujet-infos">
                  <strong>{s.titre || s.typeNom}</strong>
                  <span className="texte-doux petit">
                    {s.typeNom} · {s.chapitres.map(nomChapitre).join(", ")} · {formatDate.format(new Date(s.date))}
                  </span>
                </div>
                <span className="sujet-note">
                  {s.derniereNote !== null ? `${formatNote.format(s.derniereNote)} / 20` : s.copies > 0 ? "—" : ""}
                </span>
                <div className="rangee">
                  <button className="bouton" onClick={() => ouvrir(s.id, false)}>
                    {s.copies > 0 && !brouillonExiste(s.id) ? <RotateCcw size={14} aria-hidden /> : <Play size={14} aria-hidden />}
                    {brouillonExiste(s.id) ? "Reprendre" : s.copies > 0 ? "Refaire" : "Commencer"}
                  </button>
                  {s.copies > 0 && (
                    <button className="bouton" onClick={() => ouvrir(s.id, true)}>
                      <ClipboardCheck size={14} aria-hidden /> Correction
                    </button>
                  )}
                  {aSupprimer === s.id ? (
                    <>
                      <button className="bouton bouton-danger" onClick={() => supprimer(s.id)}>
                        <Trash size={14} aria-hidden /> Supprimer le sujet et ses copies
                      </button>
                      <button className="lien-discret petit" onClick={() => setASupprimer(null)}>
                        Annuler
                      </button>
                    </>
                  ) : (
                    <button className="lien-discret petit" onClick={() => setASupprimer(s.id)} aria-label={`Supprimer ${s.titre}`}>
                      <Trash size={13} aria-hidden /> Supprimer
                    </button>
                  )}
                </div>
              </li>
            ))}
          </ul>
        )}
      </section>
    </div>
  );
}
