// La copie corrigée : note sur 20, détail par question, extrait de cours qui justifie la réponse.
import { useEffect, useRef, useState } from "react";
import { ArrowLeft, RotateCcw, Sparkles } from "lucide-react";
import { api, Copie, messageErreur, ResultatQuestion, SujetEnregistre } from "../lib/api";
import { useEnLigne, useModeLocal } from "../lib/connexion";
import { MarkdownObsidian } from "../markdown/rendu";
import { ListeRetour, TexteIa } from "../revision/ModulesIa";
import { formatDuree, navigationSujet, titrePartie } from "./Copie";
import Graphique from "./Graphique";
import { lancerTache, oublierTache, useDerniereTache } from "../lib/taches";
import { BoutonArreter } from "../composants/TachesIa";

interface Props {
  sujet: SujetEnregistre;
  copie: Copie;
  /** Lancer tout de suite la correction des rédactions (copie qui vient d'être rendue). */
  corrigerAuto: boolean;
  onCopie: (c: Copie) => void;
  onRefaire: () => void;
  onRetour: () => void;
  onOuvrirNote: (chemin: string) => void;
}

const LIBELLES: Record<string, string> = {
  juste: "Juste",
  partiel: "Partiellement juste",
  faux: "Faux",
  a_corriger: "Rédaction à corriger",
};

const format = new Intl.NumberFormat("fr-FR", { maximumFractionDigits: 1 });

function classeVerdict(r: ResultatQuestion): string {
  if (r.retiree) return "texte-doux";
  return r.verdict === "juste" ? "verdict-juste" : r.verdict === "faux" ? "verdict-faux" : "verdict-presque";
}

export default function Resultat({ sujet, copie, corrigerAuto, onCopie, onRefaire, onRetour, onOuvrirNote }: Props) {
  // Les rédactions demandent Gemini : pas hors ligne, ni en mode local.
  const reseau = useEnLigne();
  const modeLocal = useModeLocal();
  const enLigne = reseau && !modeLocal;
  const [erreur, setErreur] = useState<string | null>(null);
  const dejaLance = useRef(false);
  const s = sujet.sujet;
  const aCorriger = copie.resultats.filter((r) => r.verdict === "a_corriger").length;
  // Correction des rédactions en arrière-plan : elle est enregistrée même si l'élève quitte l'écran.
  const tache = useDerniereTache("redactions", String(copie.id));
  const enCours = tache?.statut === "en-cours";

  useEffect(() => {
    if (tache?.statut === "finie") {
      onCopie(tache.resultat as Copie);
      oublierTache(tache.id);
    } else if (tache?.statut === "erreur") setErreur(tache.erreur ?? null);
  }, [tache?.statut]);

  function corriger() {
    setErreur(null);
    lancerTache(
      {
        genre: "redactions",
        cle: String(copie.id),
        titre: `Correction des rédactions · ${s.titre}`,
        ecran: "exercices",
        resume: (c: Copie) => (c.note === null ? "Rédactions corrigées" : `Note : ${format.format(c.note)} / 20`),
      },
      ({ id }) => api.corrigerRedactions(copie.id, id),
    ).catch(() => {});
  }

  useEffect(() => {
    // Déjà corrigée en arrière-plan (ou en cours) : ne pas relancer.
    if (tache) dejaLance.current = true;
    if (corrigerAuto && aCorriger > 0 && enLigne && !dejaLance.current) {
      dejaLance.current = true;
      corriger();
    }
  }, [corrigerAuto, aCorriger, enLigne]);

  async function retirer(r: ResultatQuestion) {
    try {
      onCopie(await api.retirerQuestion(copie.id, r.numero, !r.retiree));
    } catch (e) {
      setErreur(messageErreur(e));
    }
  }

  const comptees = copie.resultats.filter((r) => !r.retiree && r.verdict !== "a_corriger");
  const points = comptees.reduce((t, r) => t + r.points, 0);
  const max = comptees.reduce((t, r) => t + r.points_max, 0);

  return (
    <div className="page copie">
      <section className="carte-sombre bloc resultat-copie apparait">
        <div className="texte-doux petit">
          {sujet.typeNom} · {sujet.matiere} · rendue en {formatDuree(copie.dureeMs)}
        </div>
        <h1>{s.titre}</h1>
        <div className="rangee">
          <span className="note-copie">{copie.note === null ? "—" : `${format.format(copie.note)} / 20`}</span>
          <span className="texte-doux">
            {format.format(points)} / {format.format(max)} points
            {aCorriger > 0 && ` · note provisoire : ${aCorriger} rédaction${aCorriger > 1 ? "s" : ""} pas encore corrigée${aCorriger > 1 ? "s" : ""}`}
          </span>
        </div>
        {aCorriger > 0 && (
          <div className="rangee bloc-haut">
            <button className="bouton bouton-principal" disabled={!enLigne || enCours} onClick={corriger}>
              <Sparkles size={16} className={enCours ? "icone pulse" : "icone"} aria-hidden />
              {enCours ? "Correction des rédactions par l'IA…" : "Faire corriger les rédactions par l'IA"}
            </button>
            <BoutonArreter tache={tache} />
            <span className="texte-doux petit">
              {enLigne
                ? "1 appel à Gemini"
                : modeLocal
                  ? "Mode local : la correction des rédactions attendra le retour « En ligne »."
                  : "Hors ligne : la correction des rédactions attendra Internet."}
            </span>
          </div>
        )}
        {erreur && <p className="message-erreur">{erreur}</p>}
        {copie.appreciation && (
          <p className="appreciation">
            <TexteIa texte={copie.appreciation} />
          </p>
        )}
      </section>

      {(s.contexte || s.graphique) && (
        <details className="carte bloc">
          <summary className="lien-discret">Revoir le document du sujet</summary>
          {s.contexte && (
            <div className="document-sujet">
              <MarkdownObsidian markdown={s.contexte} navigation={navigationSujet} />
            </div>
          )}
          {s.graphique && <Graphique graphique={s.graphique} valeursAuSurvol />}
        </details>
      )}

      {copie.resultats.map((r, i) => {
        const q = s.questions[i];
        if (!q) return null;
        const extrait = sujet.extraits.find((e) => e.id === q.source);
        const reponse = copie.reponses[i] ?? "";
        const calculVerifie = q.type_reponse === "nombre" && q.valeur !== null;
        return (
          <section key={r.numero} className={"carte bloc question-sujet" + (r.retiree ? " retiree" : "")}>
            {q.partie && (i === 0 || s.questions[i - 1].partie !== q.partie) && (
              <div className="partie-epreuve">{titrePartie(q.partie)}</div>
            )}
            <div className="rangee entre">
              <strong>Question {r.numero}</strong>
              <strong className={classeVerdict(r)}>
                {r.retiree
                  ? "Retirée de la note"
                  : r.verdict === "a_corriger"
                    ? `? / ${r.points_max}`
                    : `${format.format(r.points)} / ${r.points_max} — ${LIBELLES[r.verdict]}`}
              </strong>
            </div>
            <div className="enonce">
              <MarkdownObsidian markdown={q.enonce} navigation={navigationSujet} />
            </div>

            <div className="texte-doux petit">Ta réponse</div>
            <div className={"reponse-eleve" + (reponse.trim() ? "" : " vide")}>{reponse.trim() || "(pas de réponse)"}</div>

            {r.commentaire && !(calculVerifie && r.commentaire.startsWith("Résultat attendu")) && (
              <p className="petit">
                <TexteIa texte={r.commentaire} />
              </p>
            )}
            <ListeRetour titre="Ce qui est présent" elements={r.presents} classe="verdict-juste" />
            <ListeRetour titre="Ce qui manque" elements={r.manquants} classe="verdict-presque" />

            {calculVerifie && (
              <p className="petit">
                Résultat attendu : <strong>{q.reponse_attendue}</strong>{" "}
                <span className="texte-doux">
                  (recalculé par Révisio
                  {q.valeur_ia ? ` ; l'IA avait annoncé « ${q.valeur_ia} », c'est le calcul de Révisio qui compte` : ""})
                </span>
              </p>
            )}
            {q.type_reponse !== "nombre" && (
              <details>
                <summary className="lien-discret petit">
                  {q.type_reponse === "texte" ? "Voir une réponse modèle" : "Voir la réponse attendue"}
                </summary>
                <div className="reponse-modele">
                  <MarkdownObsidian markdown={q.reponse_attendue} navigation={navigationSujet} />
                  {q.elements_attendus.length > 0 && (
                    <ListeRetour titre="Barème" elements={q.elements_attendus} classe="texte-doux" />
                  )}
                </div>
              </details>
            )}
            {extrait && (
              <details>
                <summary className="lien-discret petit">
                  <TexteIa
                    texte={`Dans tes notes : ${extrait.titreNote}${
                      extrait.section && extrait.section !== extrait.titreNote ? ` › ${extrait.section}` : ""
                    }`}
                  />
                </summary>
                <div className="extrait-source">
                  <MarkdownObsidian
                    markdown={extrait.markdown}
                    navigation={{ source: extrait.noteChemin, ouvrirNote: () => onOuvrirNote(extrait.noteChemin) }}
                  />
                  <button className="lien-discret petit" onClick={() => onOuvrirNote(extrait.noteChemin)}>
                    Ouvrir la note
                  </button>
                </div>
              </details>
            )}
            <div className="rangee fin">
              <button className="lien-discret petit" onClick={() => retirer(r)}>
                {r.retiree ? "Compter de nouveau cette question" : "Question mal posée ? La retirer de la note"}
              </button>
            </div>
          </section>
        );
      })}

      <div className="rangee copie-pied">
        <button className="bouton bouton-principal" onClick={onRefaire}>
          <RotateCcw size={15} aria-hidden /> Refaire ce sujet
        </button>
        <button className="bouton" onClick={onRetour}>
          <ArrowLeft size={15} aria-hidden /> Retour aux exercices
        </button>
      </div>
    </div>
  );
}
