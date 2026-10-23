// Atelier IA : génération par Gemini (en ligne) ou, pour les tâches simples, par l'IA locale (Ollama).
// Le contenu généré est enregistré et reste utilisable hors ligne (les questions deviennent des cartes).
import { useEffect, useMemo, useState } from "react";
import { FolderOpen, KeyRound, Layers, ListChecks, Network, Sparkles } from "lucide-react";
import EntetePage from "../composants/EntetePage";
import Message from "../composants/Message";
import { api, ChapitreChoisi, ChapitreIa, EstimationIa, messageErreur, ResultatGeneration } from "../lib/api";
import { useModeLocal } from "../lib/connexion";
import { nomModele, useMoteurIa } from "../lib/moteurIa";
import { lancerTache, useDerniereTache } from "../lib/taches";
import { BoutonArreter } from "../composants/TachesIa";
import BadgeIa from "./BadgeIa";
import FeynmanVue from "./FeynmanVue";
import VisionSchemas from "./VisionSchemas";

interface Props {
  onReglages: () => void;
  /** Ouvre une séance avec les questions de l'IA de ce chapitre (même pas encore à revoir). */
  onReviserQuestions: (c: ChapitreChoisi) => void;
  /** Ouvre l'écran Cartes mentales sur ce chapitre. */
  onOuvrirCartes: (c: ChapitreChoisi) => void;
}

const cle = (c: ChapitreIa) => `${c.matiere}|${c.chapitre ?? ""}`;
const nomChapitre = (c: ChapitreIa) => c.chapitre ?? "Notes générales de la matière";

export default function AtelierIaVue({ onReglages, onReviserQuestions, onOuvrirCartes }: Props) {
  const moteur = useMoteurIa();
  const modeLocal = useModeLocal();
  const { enLigne, etat } = moteur;
  const [chapitres, setChapitres] = useState<ChapitreIa[]>([]);
  // La génération continue en arrière-plan si l'élève quitte l'écran : son état est dans le gestionnaire de tâches.
  // Au retour, on retrouve le chapitre de la dernière génération.
  const tache = useDerniereTache("questions");
  const [choix, setChoix] = useState(() => tache?.cle ?? "");
  const [nombre, setNombre] = useState(10);
  const [avecSourcesWeb, setAvecSourcesWeb] = useState(false);
  const [estimation, setEstimation] = useState<EstimationIa | null>(null);
  const [erreur, setErreur] = useState<string | null>(null);
  const enCours = tache?.statut === "en-cours";
  const resultat = tache?.statut === "finie" ? (tache.resultat as ResultatGeneration) : null;

  const charger = () => {
    api.chapitresIa().then(setChapitres).catch((e) => setErreur(messageErreur(e)));
  };

  useEffect(charger, []);

  const chapitre = useMemo(() => chapitres.find((c) => cle(c) === choix) ?? null, [chapitres, choix]);

  // L'estimation est locale (aucun appel) : on sait combien d'appels coûtera la génération.
  useEffect(() => {
    setEstimation(null);
    if (chapitre) api.estimerQuestions(chapitre.matiere, chapitre.chapitre).then(setEstimation).catch(() => {});
  }, [chapitre]);

  // Fin d'une génération (même lancée avant d'arriver sur l'écran) : les compteurs changent.
  useEffect(() => {
    if (tache && tache.statut !== "en-cours") {
      charger();
      moteur.recharger();
    }
  }, [tache?.statut]);

  function generer() {
    if (!chapitre) return;
    setErreur(null);
    lancerTache(
      {
        genre: "questions",
        cle: cle(chapitre),
        titre: `Questions · ${nomChapitre(chapitre)}`,
        ecran: "atelier",
        resume: (r: ResultatGeneration) => `${r.creees} question${r.creees > 1 ? "s" : ""} ajoutée${r.creees > 1 ? "s" : ""} à tes révisions`,
      },
      ({ id }) => api.genererQuestions(chapitre.matiere, chapitre.chapitre, nombre, avecSourcesWeb, id),
    ).catch(() => {});
  }

  const matieres = [...new Set(chapitres.map((c) => c.matiere))];
  /** Chapitres où l'IA a déjà créé quelque chose. */
  const creations = chapitres.filter((c) => c.questionsIa > 0 || c.cartesMentales > 0);
  /** Chapitre de la dernière génération (pour « Réviser ces questions »). */
  const chapitreTache = chapitres.find((c) => cle(c) === tache?.cle) ?? null;

  return (
    <div className="page">
      <EntetePage
        icone={Sparkles}
        surTitre="Créer avec l'IA"
        titre="Atelier IA"
        description="Des questions et des explications créées uniquement à partir de tes notes, qui rejoignent tes révisions."
      />
      {modeLocal && (
        <Message genre="info">
          <strong>Mode local</strong> :{" "}
          {moteur.local
            ? `l'IA locale (${moteur.modeleLocal}) répond aux questions et à la méthode Feynman ; les questions sur un schéma demandent Gemini.`
            : "choisis un modèle d'IA locale dans les Réglages pour générer sans Gemini."}
        </Message>
      )}
      {!enLigne && !modeLocal && (
        <Message genre="hors-ligne">
          <strong>Hors ligne</strong> :{" "}
          {moteur.local
            ? `l'IA locale (${moteur.modeleLocal}) prend le relais pour les questions et la méthode Feynman ; les questions sur un schéma demandent Internet.`
            : "la génération est indisponible (sauf avec l'IA locale, à régler dans les Réglages)."}{" "}
          Les questions déjà générées restent dans tes séances de révision.
        </Message>
      )}
      {enLigne && !modeLocal && etat && !etat.clePresente && (
        <Message genre="attention">
          Aucune clé Gemini{moteur.local ? ` : l'IA locale (${moteur.modeleLocal}) prend le relais pour les tâches simples` : ""}.{" "}
          <button className="lien-discret" onClick={onReglages}>
            <KeyRound size={13} aria-hidden /> Ajouter une clé dans les Réglages
          </button>
        </Message>
      )}

      {creations.length > 0 && (
        <section className="carte bloc apparait" style={{ "--i": 1 } as React.CSSProperties}>
          <h2 className="titre-bloc">
            <FolderOpen size={18} className="icone" aria-hidden /> Tes créations
          </h2>
          <p className="texte-doux petit">
            Les <strong>questions</strong> créées ici deviennent des cartes de révision : elles arrivent peu à peu dans ta
            séance du jour « Autres matières » (écran Réviser), comme les autres cartes. Pour les travailler tout de suite,
            clique sur « Réviser ». Les <strong>cartes mentales</strong> se trouvent dans l'écran Cartes mentales.
          </p>
          <table className="tableau-donnees tableau-creations">
            <thead>
              <tr>
                <th>Chapitre</th>
                <th>Questions de l'IA</th>
                <th>Cartes mentales</th>
              </tr>
            </thead>
            <tbody>
              {creations.map((c) => (
                <tr key={cle(c)}>
                  <td>
                    <span className="texte-doux petit">{c.matiere}</span>
                    <br />
                    {nomChapitre(c)}
                  </td>
                  <td>
                    {c.questionsIa > 0 ? (
                      <div className="rangee">
                        <span>
                          {c.questionsIa}
                          {c.questionsDues > 0 && <span className="texte-doux petit"> · {c.questionsDues} à revoir</span>}
                        </span>
                        <button className="bouton bouton-petit" onClick={() => onReviserQuestions(c)}>
                          <Layers size={14} aria-hidden /> Réviser
                        </button>
                      </div>
                    ) : (
                      <span className="texte-doux">—</span>
                    )}
                  </td>
                  <td>
                    {c.cartesMentales > 0 ? (
                      <div className="rangee">
                        <span>{c.cartesMentales}</span>
                        <button className="bouton bouton-petit" onClick={() => onOuvrirCartes(c)}>
                          <Network size={14} aria-hidden /> Ouvrir
                        </button>
                      </div>
                    ) : (
                      <span className="texte-doux">—</span>
                    )}
                  </td>
                </tr>
              ))}
            </tbody>
          </table>
        </section>
      )}

      <section className="carte bloc apparait" style={{ "--i": 2 } as React.CSSProperties}>
        <h2 className="titre-bloc">
          <ListChecks size={18} className="icone" aria-hidden /> Questions sur un chapitre
        </h2>
        <p className="texte-doux petit">
          L'IA crée des QCM et des questions ouvertes <strong>uniquement</strong> à partir de tes notes (fiche de
          synthèse en priorité). Chaque question doit citer une phrase de ta note : si la citation est introuvable, la
          question est rejetée. Les questions deviennent des cartes de ta séance « Autres matières » (écran Réviser).
        </p>
        <div className="rangee">
          <select aria-label="Chapitre pour les questions" value={choix} onChange={(e) => setChoix(e.target.value)}>
            <option value="">Choisis un chapitre…</option>
            {matieres.map((m) => (
              <optgroup key={m} label={m}>
                {chapitres
                  .filter((c) => c.matiere === m)
                  .map((c) => (
                    <option key={cle(c)} value={cle(c)}>
                      {nomChapitre(c)} ({c.notes} note{c.notes > 1 ? "s" : ""}
                      {c.questionsIa ? `, ${c.questionsIa} questions déjà générées` : ""})
                    </option>
                  ))}
              </optgroup>
            ))}
          </select>
          <label className="rangee">
            Nombre
            <select value={nombre} onChange={(e) => setNombre(Number(e.target.value))}>
              {[5, 10, 15, 20].map((n) => (
                <option key={n} value={n}>
                  {n}
                </option>
              ))}
            </select>
          </label>
        </div>
        <label className="case petit">
          <input type="checkbox" checked={avecSourcesWeb} onChange={(e) => setAvecSourcesWeb(e.target.checked)} />
          Inclure mes sources web de ce chapitre (passages enregistrés dans l'espace web)
        </label>
        {estimation && (
          <p className="texte-doux petit">
            {estimation.extraits} extraits de tes notes · ≈ {estimation.appels} appel{estimation.appels > 1 ? "s" : ""} à{" "}
            {moteur.local ? "l'IA locale" : "Gemini"} (jusqu'à 3 par appel si la réponse doit être redemandée)
            {etat && !moteur.local ? ` · ${etat.appelsAujourdhui} appels utilisés aujourd'hui` : ""}
          </p>
        )}
        <div className="rangee">
          <button className="bouton bouton-principal" disabled={!chapitre || !moteur.simple || enCours} onClick={generer}>
            <Sparkles size={16} className={enCours ? "icone pulse" : "icone"} aria-hidden />
            {enCours
              ? moteur.local
                ? "Génération en cours avec l'IA locale… (plusieurs minutes)"
                : "Génération en cours… (cela peut prendre une minute)"
              : "Générer les questions"}
          </button>
          <BoutonArreter tache={tache} />
          <BadgeIa moteur={moteur} />
        </div>
        {enCours && tache?.titre && <p className="texte-doux petit">{tache.titre} : tu peux changer d'écran, la génération continue.</p>}
        {erreur && <p className="message-erreur">{erreur}</p>}
        {tache?.statut === "erreur" && <p className="message-erreur">{tache.erreur}</p>}
        {tache?.statut === "interrompue" && <Message genre="info">Génération interrompue. Les questions déjà créées sont gardées.</Message>}
        {resultat && (
          <div className="bloc-haut">
            <Message genre="succes">
              <strong>{resultat.creees}</strong> question{resultat.creees > 1 ? "s" : ""} ajoutée
              {resultat.creees > 1 ? "s" : ""} à tes révisions
              {resultat.rejetees > 0 && `, ${resultat.rejetees} rejetée${resultat.rejetees > 1 ? "s" : ""}`} (
              {resultat.appels} appel{resultat.appels > 1 ? "s" : ""}, {nomModele(resultat.modele)}).
            </Message>
            {resultat.creees > 0 && chapitreTache && (
              <div className="rangee">
                <button className="bouton" onClick={() => onReviserQuestions(chapitreTache)}>
                  <Layers size={15} aria-hidden /> Réviser ces questions maintenant
                </button>
                <span className="texte-doux petit">Sinon, elles arriveront peu à peu dans ta séance du jour.</span>
              </div>
            )}
            {resultat.raisons.length > 0 && (
              <details className="petit">
                <summary className="lien-discret">Pourquoi des questions ont été rejetées</summary>
                <ul>
                  {resultat.raisons.map((r, i) => (
                    <li key={i}>{r}</li>
                  ))}
                </ul>
              </details>
            )}
          </div>
        )}
      </section>

      <VisionSchemas disponible={moteur.gemini} />

      <FeynmanVue chapitres={chapitres} moteur={moteur} />
    </div>
  );
}
