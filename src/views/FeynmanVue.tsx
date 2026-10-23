// Méthode Feynman : expliquer une notion avec ses mots, puis comparer avec ses notes.
import { useEffect, useState } from "react";
import { CircleCheck, Lightbulb, MessageSquareText, TriangleAlert } from "lucide-react";
import { api, ChapitreIa, EssaiFeynman, PointSource, ResultatFeynman, RetourFeynman } from "../lib/api";
import { MoteurIa, nomModele } from "../lib/moteurIa";
import { ListeRetour, TexteIa } from "../revision/ModulesIa";
import BadgeIa from "./BadgeIa";
import Message from "../composants/Message";
import { lancerTache, useDerniereTache } from "../lib/taches";
import { BoutonArreter } from "../composants/TachesIa";

interface Props {
  chapitres: ChapitreIa[];
  moteur: MoteurIa;
}

const cle = (c: ChapitreIa) => `${c.matiere}|${c.chapitre ?? ""}`;
const formatDate = new Intl.DateTimeFormat("fr-FR", { day: "numeric", month: "short", hour: "2-digit", minute: "2-digit" });

function Points({ titre, points, classe }: { titre: string; points: PointSource[]; classe: string }) {
  if (!points.length) return null;
  return (
    <div>
      <div className={`petit ${classe}`}>{titre}</div>
      <ul className="liste-retour">
        {points.map((p, i) => (
          <li key={i}>
            <TexteIa texte={p.texte} />
            {p.citation && (
              <blockquote className="citation-source">
                « {p.citation} »{" "}
                <span className="texte-doux petit">
                  {p.verifie ? (
                    <>
                      <CircleCheck size={13} className="icone verdict-juste" aria-hidden /> phrase retrouvée dans tes notes
                    </>
                  ) : (
                    <>
                      <TriangleAlert size={13} className="icone verdict-presque" aria-hidden /> phrase non retrouvée telle quelle
                      dans tes notes
                    </>
                  )}
                </span>
              </blockquote>
            )}
          </li>
        ))}
      </ul>
    </div>
  );
}

export function AffichageRetour({ retour }: { retour: RetourFeynman }) {
  return (
    <div className="carte retour-ia">
      <strong>{retour.note} / 20</strong>
      <ListeRetour titre="Ce qui est juste" elements={retour.ce_qui_est_juste} classe="verdict-juste" />
      <Points titre="Ce qui manque" points={retour.ce_qui_manque} classe="verdict-presque" />
      <Points titre="Erreurs ou imprécisions" points={retour.erreurs} classe="verdict-faux" />
      {retour.conseil && (
        <p className="petit">
          <strong>Conseil :</strong> <TexteIa texte={retour.conseil} />
        </p>
      )}
    </div>
  );
}

export default function FeynmanVue({ chapitres, moteur }: Props) {
  const [choix, setChoix] = useState("");
  const [notions, setNotions] = useState<string[]>([]);
  const [notion, setNotion] = useState("");
  const [explication, setExplication] = useState("");
  const [historique, setHistorique] = useState<EssaiFeynman[]>([]);
  const [erreur, setErreur] = useState<string | null>(null);
  const tache = useDerniereTache("feynman");
  const enCours = tache?.statut === "en-cours";
  const resultat = tache?.statut === "finie" ? (tache.resultat as ResultatFeynman) : null;

  const chapitre = chapitres.find((c) => cle(c) === choix) ?? null;

  useEffect(() => {
    api.historiqueFeynman().then(setHistorique).catch(() => {});
  }, [tache?.statut]);

  useEffect(() => {
    setNotions([]);
    setNotion("");
    if (chapitre) api.notionsFeynman(chapitre.matiere, chapitre.chapitre).then(setNotions).catch(() => {});
  }, [chapitre]);

  function evaluer() {
    if (!chapitre || !notion.trim()) return;
    setErreur(null);
    const n = notion.trim();
    lancerTache(
      {
        genre: "feynman",
        titre: `Méthode Feynman · ${n}`,
        ecran: "atelier",
        resume: (r: ResultatFeynman) => `Ton explication a obtenu ${r.retour.note} / 20`,
      },
      ({ id }) => api.evaluerFeynman(chapitre.matiere, chapitre.chapitre, n, explication, id),
    ).catch(() => {});
  }

  const nbMots = explication.split(/\s+/).filter(Boolean).length;

  return (
    <section className="carte bloc apparait" style={{ "--i": 3 } as React.CSSProperties}>
      <h2 className="titre-bloc">
        <Lightbulb size={18} className="icone" aria-hidden /> Méthode Feynman
      </h2>
      <p className="texte-doux petit">
        Explique une notion avec tes propres mots, comme à quelqu'un qui ne la connaît pas, sans regarder tes notes.
        L'IA compare ensuite ton explication avec tes notes : ce qui est juste, ce qui manque, ce qui est faux.
      </p>
      <div className="rangee">
        <select aria-label="Chapitre pour la méthode Feynman" value={choix} onChange={(e) => setChoix(e.target.value)}>
          <option value="">Chapitre…</option>
          {[...new Set(chapitres.map((c) => c.matiere))].map((m) => (
            <optgroup key={m} label={m}>
              {chapitres
                .filter((c) => c.matiere === m)
                .map((c) => (
                  <option key={cle(c)} value={cle(c)}>
                    {c.chapitre ?? "Notes générales de la matière"}
                  </option>
                ))}
            </optgroup>
          ))}
        </select>
        <input
          className="champ"
          list="notions-feynman"
          placeholder="Notion à expliquer (choisis ou écris)"
          value={notion}
          onChange={(e) => setNotion(e.target.value)}
          disabled={!chapitre}
        />
        <datalist id="notions-feynman">
          {notions.map((n) => (
            <option key={n} value={n} />
          ))}
        </datalist>
      </div>
      <textarea
        className="champ zone-reponse bloc-haut"
        rows={8}
        placeholder="Ton explication…"
        value={explication}
        onChange={(e) => setExplication(e.target.value)}
      />
      <div className="rangee">
        <button
          className="bouton bouton-principal"
          disabled={!moteur.simple || !chapitre || !notion.trim() || nbMots < 15 || enCours}
          onClick={evaluer}
        >
          <MessageSquareText size={16} aria-hidden />
          {enCours ? "Comparaison avec tes notes…" : "Comparer avec mes notes"}
        </button>
        <span className="texte-doux petit">
          {nbMots} mot{nbMots > 1 ? "s" : ""}
          {nbMots < 15 ? " (15 au moins)" : ""} · 1 appel à {moteur.local ? "l'IA locale" : "Gemini"}
        </span>
        <BoutonArreter tache={tache} />
        <BadgeIa moteur={moteur} />
      </div>
      {erreur && <p className="message-erreur">{erreur}</p>}
      {tache?.statut === "erreur" && <p className="message-erreur">{tache.erreur}</p>}
      {tache?.statut === "interrompue" && <Message genre="info">Comparaison interrompue.</Message>}
      {resultat && (
        <>
          <p className="petit">
            <strong>{tache?.titre}</strong>
          </p>
          <AffichageRetour retour={resultat.retour} />
          <p className="texte-doux petit">Comparaison faite par {nomModele(resultat.modele)}.</p>
        </>
      )}

      {historique.length > 0 && (
        <details className="bloc-haut">
          <summary className="lien-discret">Mes explications précédentes ({historique.length})</summary>
          <ul className="liste-faibles">
            {historique.map((h) => (
              <li key={h.id}>
                <details>
                  <summary>
                    <strong>{h.notion}</strong> · {h.note} / 20 ·{" "}
                    <span className="texte-doux petit">
                      {h.matiere} · {formatDate.format(new Date(h.date))}
                    </span>
                  </summary>
                  <p className="petit explication-passee">{h.explication}</p>
                  <AffichageRetour retour={h.retour} />
                </details>
              </li>
            ))}
          </ul>
        </details>
      )}
    </section>
  );
}
