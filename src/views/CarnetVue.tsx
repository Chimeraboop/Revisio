import { useEffect, useMemo, useState } from "react";
import { BookOpen, Check, ExternalLink, FileText, NotebookPen, Repeat, RotateCcw } from "lucide-react";
import EntetePage from "../composants/EntetePage";
import Message from "../composants/Message";
import { api, CarteSeance, ErreurCarnet, messageErreur, Reglages } from "../lib/api";
import { MarkdownObsidian } from "../markdown/rendu";
import { NOMS_GENRES, remplirTrou } from "../revision/modules";
import { TexteMaths } from "../revision/symboles";
import SeanceVue, { Bilan } from "./SeanceVue";
import { useEstObsidian } from "../lib/dossier";

interface Props {
  reglages: Reglages;
  onOuvrirNote: (chemin: string, ancre?: string | null) => void;
}

const formatDate = new Intl.DateTimeFormat("fr-FR", { day: "numeric", month: "long", hour: "2-digit", minute: "2-digit" });

/** La question telle qu'on peut la relire hors séance. */
function Question({ e }: { e: ErreurCarnet }) {
  const nav = { source: e.noteChemin, ouvrirNote: () => {} };
  if (e.genre === "trou") {
    // Le trou est remplacé par la bonne réponse, surlignée.
    return <MarkdownObsidian markdown={remplirTrou(e.question, e.reponseAttendue)} navigation={nav} />;
  }
  if (e.genre === "schema")
    return (
      <p>
        Schéma muet : <TexteMaths texte={e.question} />
      </p>
    );
  if (e.genre === "ordre") return <p>{e.section || e.question}</p>;
  if (e.genre === "examen") return <MarkdownObsidian markdown={e.question} navigation={nav} />;
  return (
    <p className="terme-petit">
      <TexteMaths texte={e.question} />
    </p>
  );
}

export default function CarnetVue({ reglages, onOuvrirNote }: Props) {
  const estObsidian = useEstObsidian();
  const [erreurs, setErreurs] = useState<ErreurCarnet[] | null>(null);
  const [matiere, setMatiere] = useState("");
  const [avecComprises, setAvecComprises] = useState(false);
  const [ouvertes, setOuvertes] = useState<Set<number>>(new Set());
  const [seance, setSeance] = useState<CarteSeance[] | null>(null);
  const [bilan, setBilan] = useState<Bilan | null>(null);
  const [message, setMessage] = useState<string | null>(null);

  const charger = () =>
    api
      .carnetErreurs(matiere || null, avecComprises)
      .then(setErreurs)
      .catch((e) => setMessage(messageErreur(e)));

  useEffect(() => {
    charger();
  }, [matiere, avecComprises]);

  const matieres = useMemo(() => {
    const m = new Set((erreurs ?? []).map((e) => e.matiere ?? "Hors matière"));
    if (matiere) m.add(matiere);
    return [...m].sort((a, b) => a.localeCompare(b, "fr"));
  }, [erreurs, matiere]);

  async function marquer(e: ErreurCarnet) {
    try {
      await api.marquerErreurComprise(e.id, !e.comprise);
      charger();
    } catch (err) {
      setMessage(messageErreur(err));
    }
  }

  async function retravailler() {
    const ids = (erreurs ?? []).filter((e) => !e.comprise && e.carteId !== null).map((e) => e.carteId as number);
    try {
      const cartes = await api.cartesChoisies(ids);
      if (cartes.length) setSeance(cartes);
      else setMessage("Ces cartes n'existent plus (note modifiée ou carte retirée).");
    } catch (err) {
      setMessage(messageErreur(err));
    }
  }

  if (seance) {
    return (
      <SeanceVue
        cartes={seance}
        reglages={reglages}
        onFin={(b) => {
          setSeance(null);
          setBilan(b);
          charger();
        }}
      />
    );
  }

  const aRetravailler = (erreurs ?? []).filter((e) => !e.comprise && e.carteId !== null).length;

  return (
    <div className="page">
      <EntetePage
        icone={NotebookPen}
        surTitre="Réviser"
        titre="Carnet d'erreurs"
        description="Chaque erreur faite en révision, en exercice ou en examen, avec le passage de ta note qui contient la bonne réponse."
      />

      {bilan && (
        <Message genre="succes">
          Séance terminée : {bilan.justes} / {bilan.vues} juste{bilan.justes > 1 ? "s" : ""}.
        </Message>
      )}

      <div className="rangee bloc">
        <select value={matiere} onChange={(e) => setMatiere(e.target.value)}>
          <option value="">Toutes les matières</option>
          {matieres.map((m) => (
            <option key={m} value={m}>
              {m}
            </option>
          ))}
        </select>
        <label className="case">
          <input type="checkbox" checked={avecComprises} onChange={(e) => setAvecComprises(e.target.checked)} />
          Afficher aussi les erreurs comprises
        </label>
        <span className="espace" />
        <button className="bouton bouton-principal" disabled={!aRetravailler} onClick={retravailler}>
          <Repeat size={16} aria-hidden /> Retravailler ces cartes ({aRetravailler})
        </button>
      </div>

      {message && <p className="message-erreur">{message}</p>}
      {erreurs && erreurs.length === 0 && (
        <div className="carte etat-vide apparait">
          <Check size={28} strokeWidth={1.8} aria-hidden />
          <p>Aucune erreur à retravailler{matiere ? ` en ${matiere}` : ""}. Bravo !</p>
        </div>
      )}

      <div className="liste-carnet">
        {erreurs?.map((e, rang) => {
          const ouverte = ouvertes.has(e.id);
          return (
            <article
              key={e.id}
              className={"carte erreur apparait" + (e.comprise ? " erreur-comprise" : "")}
              style={{ "--i": Math.min(rang, 8) } as React.CSSProperties}
            >
              <div className="rangee entre petit texte-doux">
                <span>
                  {[e.matiere, e.chapitre].filter(Boolean).join(" › ")} · {NOMS_GENRES[e.genre] ?? e.genre}
                </span>
                <span>
                  {formatDate.format(new Date(e.date))}
                  {e.fois > 1 && <strong className="fois"> · {e.fois} erreurs</strong>}
                </span>
              </div>
              <Question e={e} />
              <div className="reponses">
                <div>
                  <span className="texte-doux petit">Ta réponse</span>
                  <div className="verdict-faux">
                    {e.reponseDonnee ? (
                      <TexteMaths texte={e.reponseDonnee} />
                    ) : e.genre === "examen" ? (
                      "(pas de réponse)"
                    ) : (
                      "(carte auto-évaluée)"
                    )}
                  </div>
                </div>
                <div>
                  <span className="texte-doux petit">Bonne réponse</span>
                  <div className="verdict-juste">
                    {e.genre === "examen" ? (
                      <MarkdownObsidian markdown={e.reponseAttendue} navigation={{ source: e.noteChemin, ouvrirNote: () => {} }} />
                    ) : (
                      <TexteMaths texte={e.reponseAttendue} />
                    )}
                  </div>
                </div>
              </div>
              <div className="rangee">
                <button
                  className="lien-discret"
                  onClick={() =>
                    setOuvertes((s) => {
                      const n = new Set(s);
                      if (n.has(e.id)) n.delete(e.id);
                      else n.add(e.id);
                      return n;
                    })
                  }
                >
                  <FileText size={14} aria-hidden /> {ouverte ? "Masquer" : "Voir"} l'extrait de la note
                </button>
                <button
                  className="lien-discret"
                  onClick={() =>
                    // Une question tirée d'une source web renvoie à la page, ouverte dans le navigateur.
                    /^https?:\/\//.test(e.noteChemin)
                      ? api.ouvrirLienExterne(e.noteChemin)
                      : onOuvrirNote(e.noteChemin, e.section || null)
                  }
                >
                  <BookOpen size={14} aria-hidden /> Ouvrir « {e.titreNote} »
                </button>
                {estObsidian && (
                  <button className="lien-discret" onClick={() => api.ouvrirDansObsidian(e.noteChemin)}>
                    <ExternalLink size={14} aria-hidden /> Ouvrir dans Obsidian
                  </button>
                )}
                <span className="espace" />
                <button className="bouton" onClick={() => marquer(e)}>
                  {e.comprise ? (
                    <>
                      <RotateCcw size={15} aria-hidden /> Remettre à retravailler
                    </>
                  ) : (
                    <>
                      <Check size={15} aria-hidden /> J'ai compris
                    </>
                  )}
                </button>
              </div>
              {ouverte && (
                <div className="extrait apparait">
                  <MarkdownObsidian markdown={e.extrait} navigation={{ source: e.noteChemin, ouvrirNote: onOuvrirNote }} />
                </div>
              )}
            </article>
          );
        })}
      </div>
    </div>
  );
}
