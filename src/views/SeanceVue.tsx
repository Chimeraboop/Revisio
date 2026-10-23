import { useCallback, useEffect, useMemo, useState } from "react";
import {
  CircleAlert,
  CircleCheck,
  CircleQuestionMark,
  CircleX,
  CornerDownLeft,
  Dices,
  ExternalLink,
  Eye,
  FileText,
  Flag,
  RotateCw,
  type LucideIcon,
  ShieldCheck,
  X,
} from "lucide-react";
import { api, CarteSeance, Confiance, messageErreur, Reglages } from "../lib/api";
import { MarkdownObsidian } from "../markdown/rendu";
import { apercu, LIBELLES_NOTES, planifier, type Note } from "../revision/fsrs";
import { MODULES, type ModuleCarte } from "../revision/modules";
import { TexteMaths } from "../revision/symboles";
import { noteAutoEvaluee, noteSuggeree, type Resultat } from "../revision/verification";

export interface Bilan {
  vues: number;
  justes: number;
  dureeMs: number;
  erreurs: { carte: CarteSeance; reponseDonnee: string }[];
}

interface Props {
  cartes: CarteSeance[];
  reglages: Reglages;
  onFin: (bilan: Bilan) => void;
}

type Phase = "question" | "confiance" | "corrige";

const CONFIANCES: { valeur: Confiance; libelle: string; touche: string; icone: LucideIcon }[] = [
  { valeur: "sur", libelle: "Sûr", touche: "S", icone: ShieldCheck },
  { valeur: "hesitant", libelle: "Hésitant", touche: "H", icone: CircleQuestionMark },
  { valeur: "hasard", libelle: "Au hasard", touche: "A", icone: Dices },
];

const VERDICTS = {
  juste: { libelle: "Juste", icone: CircleCheck },
  presque: { libelle: "Presque", icone: CircleAlert },
  faux: { libelle: "Faux", icone: CircleX },
} as const;

/** Une carte ratée (prochaine révision dans moins de 15 min) revient en fin de séance, 3 fois au plus. */
const DELAI_RETOUR_MS = 15 * 60_000;
const RETOURS_MAX = 3;

interface Notation {
  carte: CarteSeance;
  correcte: boolean;
  reponseDonnee: string | null;
  etat: string;
  echeance: number;
}

/** La séance : file de cartes, bilan, cartes ratées remises en fin de file. */
export default function SeanceVue({ cartes, reglages, onFin }: Props) {
  const [file, setFile] = useState(cartes);
  const [retours, setRetours] = useState<Record<number, number>>({});
  const [debutSeance] = useState(Date.now());
  const [bilan, setBilan] = useState<Bilan>({ vues: 0, justes: 0, dureeMs: 0, erreurs: [] });

  const carte = file[0];

  const terminer = useCallback(
    (b: Bilan) => onFin({ ...b, dureeMs: Date.now() - debutSeance }),
    [onFin, debutSeance],
  );

  useEffect(() => {
    if (!carte) terminer(bilan);
  }, [carte, bilan, terminer]);

  function apresNotation({ carte, correcte, reponseDonnee, etat, echeance }: Notation) {
    setBilan((b) => ({
      ...b,
      vues: b.vues + 1,
      justes: b.justes + (correcte ? 1 : 0),
      erreurs: correcte ? b.erreurs : [...b.erreurs, { carte, reponseDonnee: reponseDonnee ?? "(auto-évaluée)" }],
    }));
    const deja = retours[carte.id] ?? 0;
    const revient = echeance - Date.now() < DELAI_RETOUR_MS && deja < RETOURS_MAX;
    setRetours((r) => ({ ...r, [carte.id]: deja + 1 }));
    setFile((f) => (revient ? [...f.slice(1), { ...carte, etatFsrs: etat, echeance }] : f.slice(1)));
  }

  if (!carte) return null;
  const total = bilan.vues + file.length;

  return (
    <div className="page seance">
      <div className="seance-entete">
        <div className="rangee entre">
          <span className="seance-compteur">
            <strong>{bilan.vues + 1}</strong> <span className="texte-doux">/ {total}</span>
          </span>
          <span className="texte-doux petit seance-chemin">{[carte.matiere, carte.chapitre].filter(Boolean).join(" › ")}</span>
          <span className="rangee">
            {carte.etatFsrs ? null : <span className="badge-plein">Nouvelle</span>}
            <button className="bouton bouton-discret" onClick={() => terminer(bilan)}>
              <X size={15} aria-hidden /> Terminer
            </button>
          </span>
        </div>
        <div className="progression progression-seance">
          <div className="progression-barre" style={{ width: `${(bilan.vues / Math.max(total, 1)) * 100}%` }} />
        </div>
      </div>

      {MODULES[carte.genre] ? (
        // La clé change à chaque carte : tout l'état de la carte (réponse, phase…) repart de zéro.
        <CarteEnCours
          key={`${carte.id}-${bilan.vues}`}
          carte={carte}
          module={MODULES[carte.genre]}
          reglages={reglages}
          onNotee={apresNotation}
          onRetiree={() => setFile((f) => f.slice(1))}
        />
      ) : (
        <div className="carte">
          <p className="message-erreur">Type de carte non pris en charge : {carte.genre}</p>
          <button className="bouton" onClick={() => setFile((f) => f.slice(1))}>
            Passer
          </button>
        </div>
      )}
    </div>
  );
}

interface PropsCarte {
  carte: CarteSeance;
  module: ModuleCarte<unknown>;
  reglages: Reglages;
  onNotee: (n: Notation) => void;
  onRetiree: () => void;
}

/** Une carte : question → (confiance) → correction → note 1-4. */
function CarteEnCours({ carte, module, reglages, onNotee, onRetiree }: PropsCarte) {
  const mode = module.mode(carte, reglages);
  const [phase, setPhase] = useState<Phase>("question");
  const [confiance, setConfiance] = useState<Confiance | null>(null);
  const [reponse, setReponse] = useState<unknown>(() => module.reponseInitiale(carte));
  const [resultat, setResultat] = useState<Resultat | null>(null);
  const [debut] = useState(Date.now());
  const [rappel, setRappel] = useState<string | null>(null);
  const [erreur, setErreur] = useState<string | null>(null);
  const [extraitVisible, setExtraitVisible] = useState(false);
  const [enCours, setEnCours] = useState(false);

  const intervalles = useMemo(() => apercu(carte.etatFsrs), [carte.etatFsrs]);

  function corriger(c: Confiance) {
    if (mode === "repondre") setResultat(module.evaluer(carte, reponse));
    setConfiance(c);
    setPhase("corrige");
  }

  function choisirConfiance(c: Confiance) {
    if (phase === "corrige") return;
    setConfiance(c);
    setRappel(null);
    if (phase === "confiance") corriger(c);
  }

  /** Réponse validée (Entrée) : on demande la confiance si elle n'a pas encore été donnée. */
  function valider() {
    if (phase !== "question" || mode !== "repondre") return;
    if (confiance) corriger(confiance);
    else setPhase("confiance");
  }

  // Vocabulaire à retourner : une vraie carte recto / verso, qui se retourne d'un clic (comme Quizlet).
  const retournable = mode === "retourner" && carte.genre === "vocabulaire";
  const [verso, setVerso] = useState(false);
  const [secousse, setSecousse] = useState(0);

  function reveler() {
    if (phase !== "question" || mode !== "retourner") return;
    if (!confiance) {
      setRappel("Indique d'abord ta confiance : S (sûr), H (hésitant) ou A (au hasard).");
      setSecousse((n) => n + 1);
      return;
    }
    corriger(confiance);
    setVerso(true);
  }

  /** Clic sur la carte : la première fois elle se retourne (réponse révélée), ensuite elle passe d'une face à l'autre. */
  function cliquerCarte() {
    if (phase === "question") reveler();
    else setVerso((v) => !v);
  }

  const suggestion: Note | null =
    phase === "corrige" && mode === "repondre" && resultat && confiance ? noteSuggeree(resultat.verdict, confiance) : null;

  async function noter(choisie: Note) {
    if (!confiance || phase !== "corrige" || enCours) return;
    const correcte = mode === "repondre" ? resultat?.verdict !== "faux" : choisie >= 2;
    // Une bonne réponse « au hasard » est toujours à revoir.
    const note = correcte && confiance === "hasard" ? 1 : mode === "retourner" ? noteAutoEvaluee(choisie, confiance) : choisie;
    const { etat, echeance } = planifier(carte.etatFsrs, note);
    const reponseDonnee = mode === "repondre" ? module.enTexte(reponse) : null;
    setEnCours(true);
    try {
      await api.enregistrerRevision({
        carteId: carte.id,
        note,
        confiance,
        correcte,
        reponseDonnee,
        dureeMs: Date.now() - debut,
        etatFsrs: etat,
        echeance,
      });
      onNotee({ carte, correcte, reponseDonnee, etat, echeance });
    } catch (e) {
      setErreur(messageErreur(e));
      setEnCours(false);
    }
  }

  async function signaler() {
    if (!window.confirm("Retirer définitivement cette carte (fausse, mal posée ou inutile) ?")) return;
    try {
      await api.suspendreCarte(carte.id);
      onRetiree();
    } catch (e) {
      setErreur(messageErreur(e));
    }
  }

  // Raccourcis clavier : S/H/A = confiance, Espace = révéler, 1-4 = note, Entrée = valider / note suggérée.
  useEffect(() => {
    const touche = (e: KeyboardEvent) => {
      const dansChamp =
        (e.target instanceof HTMLInputElement || e.target instanceof HTMLTextAreaElement) && !e.target.disabled;
      if (dansChamp || e.ctrlKey || e.altKey || e.metaKey) return;
      // Sur un clavier AZERTY, la rangée du haut donne « & é " ' » sans Maj : on se fie à la
      // touche physique (Digit1…4 ou pavé numérique) plutôt qu'au caractère produit.
      const chiffre = /^(Digit|Numpad)([1-4])$/.exec(e.code)?.[2];
      const k = chiffre ?? e.key.toLowerCase();
      if (phase === "question" && mode === "repondre" && k === "enter") {
        e.preventDefault();
        valider();
        return;
      }
      if (phase !== "corrige") {
        const c = CONFIANCES.find((x) => x.touche.toLowerCase() === k);
        if (c) {
          e.preventDefault();
          choisirConfiance(c.valeur);
          return;
        }
        if (k === " " && mode === "retourner") {
          e.preventDefault();
          reveler();
        }
        return;
      }
      if (["1", "2", "3", "4"].includes(k)) {
        e.preventDefault();
        noter(Number(k) as Note);
      } else if ((k === " " || k === "enter") && suggestion) {
        e.preventDefault();
        noter(suggestion);
      }
    };
    window.addEventListener("keydown", touche);
    return () => window.removeEventListener("keydown", touche);
  });

  const Question = module.Question;
  const Correction = module.Correction;

  const avertissementHasard = confiance === "hasard" && (resultat ? resultat.verdict !== "faux" : true) && (
    <p className="texte-doux petit">Réponse annoncée « au hasard » : la carte reviendra bientôt, même si tu as trouvé.</p>
  );

  return (
    <>
      {retournable ? (
        <div className="entree-carte">
          <div
            key={secousse}
            className={"carte-retournable" + (verso ? " verso" : "") + (secousse ? " secoue" : "")}
            role="button"
            tabIndex={0}
            aria-pressed={verso}
            aria-label={phase === "question" ? "Retourner la carte" : verso ? "Revoir le recto" : "Revoir la réponse"}
            onClick={cliquerCarte}
          >
            <div className="carte-retournable-interieur">
              <div className="carte carte-revision face face-recto" aria-hidden={verso}>
                <div className="entete-carte-revision">
                  <div className="consigne">{module.consigne(carte)}</div>
                  <button
                    className="lien-discret"
                    onClick={(e) => {
                      e.stopPropagation();
                      signaler();
                    }}
                    title="Signaler une carte fausse, mal posée ou inutile (elle sera retirée)"
                  >
                    <Flag size={13} aria-hidden /> Signaler
                  </button>
                </div>
                <div className="face-contenu">
                  <Question carte={carte} mode={mode} reponse={reponse} setReponse={setReponse} corrige={phase !== "question"} onValider={valider} />
                </div>
                <div className="face-aide texte-doux petit">
                  <RotateCw size={13} aria-hidden /> {phase === "question" ? "Clique sur la carte pour la retourner" : "Clique pour revoir la réponse"}
                </div>
              </div>
              <div className="carte carte-revision face face-verso" aria-hidden={!verso}>
                <div className="entete-carte-revision">
                  <div className="consigne">Réponse</div>
                </div>
                <div className="face-contenu">
                  {phase === "corrige" && <Correction carte={carte} reponse={reponse} />}
                  {avertissementHasard}
                </div>
                <div className="face-aide texte-doux petit">
                  <RotateCw size={13} aria-hidden /> Clique pour revoir le recto
                </div>
              </div>
            </div>
          </div>
        </div>
      ) : (
      <div className="carte carte-revision entree-carte">
        <div className="entete-carte-revision">
          <div className="consigne">{module.consigne(carte)}</div>
          <button className="lien-discret" onClick={signaler} title="Signaler une carte fausse, mal posée ou inutile (elle sera retirée)">
            <Flag size={13} aria-hidden /> Signaler
          </button>
        </div>
        <Question
          carte={carte}
          mode={mode}
          reponse={reponse}
          setReponse={setReponse}
          corrige={phase !== "question"}
          onValider={valider}
        />

        {phase === "corrige" && (
          <div className="correction">
            {resultat && (
              <div className={`verdict verdict-${resultat.verdict}`}>
                {(() => {
                  const V = VERDICTS[resultat.verdict].icone;
                  return <V size={18} strokeWidth={2.2} aria-hidden />;
                })()}
                {VERDICTS[resultat.verdict].libelle}
                {resultat.message && (
                  <span className="verdict-message">
                    {" "}
                    — <TexteMaths texte={resultat.message} />
                  </span>
                )}
              </div>
            )}
            <Correction carte={carte} reponse={reponse} />
            {avertissementHasard}
          </div>
        )}
      </div>
      )}

      {phase !== "corrige" && (
        <div className="zone-actions">
          <div className="rangee centre">
            <span className="texte-doux petit">Ta confiance</span>
            <div className="segmente" role="radiogroup" aria-label="Ta confiance">
              {CONFIANCES.map((c) => {
                const Icone = c.icone;
                return (
                  <button
                    key={c.valeur}
                    role="radio"
                    aria-checked={confiance === c.valeur}
                    className={"segment" + (confiance === c.valeur ? " actif" : "")}
                    onClick={() => choisirConfiance(c.valeur)}
                  >
                    <Icone size={15} aria-hidden /> {c.libelle} <kbd>{c.touche}</kbd>
                  </button>
                );
              })}
            </div>
          </div>
          {mode === "retourner" ? (
            <button className="bouton bouton-principal bouton-large" onClick={reveler}>
              {retournable ? <RotateCw size={17} aria-hidden /> : <Eye size={17} aria-hidden />}
              {retournable ? "Retourner la carte" : "Révéler la réponse"} <kbd>Espace</kbd>
            </button>
          ) : phase === "question" ? (
            <button className="bouton bouton-principal bouton-large" onClick={valider}>
              <CornerDownLeft size={17} aria-hidden /> Valider ma réponse <kbd>Entrée</kbd>
            </button>
          ) : (
            <p className="texte-doux centre">Choisis ta confiance pour voir la correction.</p>
          )}
          {rappel && <p className="message-erreur centre">{rappel}</p>}
        </div>
      )}

      {phase === "corrige" && (
        <div className="zone-actions apparait">
          <div className="notes-fsrs">
            {([1, 2, 3, 4] as Note[]).map((n) => (
              <button
                key={n}
                className={`bouton note-fsrs note-${n}` + (suggestion === n ? " suggere" : "")}
                onClick={() => noter(n)}
                disabled={enCours}
              >
                <span className="note-fsrs-libelle">
                  {LIBELLES_NOTES[n]} <kbd>{n}</kbd>
                </span>
                <span className="petit note-fsrs-delai">{intervalles[n]}</span>
              </button>
            ))}
          </div>
          <p className="texte-doux petit centre">
            {suggestion
              ? `Note suggérée : ${LIBELLES_NOTES[suggestion]} (Entrée). Tu peux en choisir une autre avec 1-4.`
              : "Note-toi honnêtement : 1 si tu ne savais pas, 2 à 4 si tu savais."}
          </p>
          <div className="rangee centre">
            <button className="lien-discret" onClick={() => setExtraitVisible((v) => !v)}>
              <FileText size={14} aria-hidden /> {extraitVisible ? "Masquer" : "Voir"} l'extrait de la note
            </button>
            <button className="lien-discret" onClick={() => api.ouvrirDansObsidian(carte.noteChemin)}>
              <ExternalLink size={14} aria-hidden /> Ouvrir dans Obsidian
            </button>
          </div>
          {extraitVisible && (
            <div className="carte extrait apparait">
              <div className="texte-doux petit">
                {carte.titreNote}
                {carte.section ? ` › ${carte.section}` : ""}
              </div>
              <MarkdownObsidian markdown={carte.extrait} navigation={{ source: carte.noteChemin, ouvrirNote: () => {} }} />
            </div>
          )}
        </div>
      )}

      {erreur && <p className="message-erreur">{erreur}</p>}
    </>
  );
}
