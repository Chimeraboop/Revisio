// Questions sur un schéma : le dessin Excalidraw est rendu en image et envoyé à Gemini (vision).
// Les questions rejoignent les révisions et affichent le schéma au-dessus de l'énoncé.
import { useEffect, useMemo, useState } from "react";
import { ScanEye, Sparkles } from "lucide-react";
import Message from "../composants/Message";
import { api, messageErreur, ResultatGeneration, SchemaInfo } from "../lib/api";
import { chargerScene } from "../excalidraw/chargement";
import Dessin, { versPngBase64 } from "../excalidraw/Dessin";
import type { Scene } from "../excalidraw/scene";
import { TexteMaths } from "../revision/symboles";
import { lancerTache, useDerniereTache } from "../lib/taches";
import { BoutonArreter } from "../composants/TachesIa";

export default function VisionSchemas({ disponible }: { disponible: boolean }) {
  const [schemas, setSchemas] = useState<SchemaInfo[]>([]);
  const [choix, setChoix] = useState("");
  const [scene, setScene] = useState<Scene | null | undefined>(undefined);
  const [nombre, setNombre] = useState(5);
  const [erreur, setErreur] = useState<string | null>(null);
  const tache = useDerniereTache("schema");
  const enCours = tache?.statut === "en-cours";
  const resultat = tache?.statut === "finie" ? (tache.resultat as ResultatGeneration) : null;

  const charger = () => api.schemasIa(null).then(setSchemas).catch((e) => setErreur(messageErreur(e)));
  useEffect(() => {
    charger();
  }, []);

  const schema = useMemo(() => schemas.find((s) => s.chemin === choix) ?? null, [schemas, choix]);
  const matieres = [...new Set(schemas.map((s) => s.matiere ?? "Hors matière"))];

  useEffect(() => {
    setScene(undefined);
    if (schema) chargerScene(schema.chemin).then(setScene);
  }, [schema]);

  useEffect(() => {
    if (tache?.statut === "finie") charger();
  }, [tache?.statut]);

  function generer() {
    if (!schema || !scene) return;
    setErreur(null);
    lancerTache(
      {
        genre: "schema",
        titre: `Questions sur un schéma · ${schema.titre.replace(/\s*\(schéma\)$/i, "")}`,
        ecran: "atelier",
        resume: (r: ResultatGeneration) => `${r.creees} question${r.creees > 1 ? "s" : ""} ajoutée${r.creees > 1 ? "s" : ""} à tes révisions`,
      },
      async ({ id }) => api.genererQuestionsSchema(schema.chemin, await versPngBase64(scene), nombre, id),
    ).catch(() => {});
  }

  return (
    <section className="carte bloc apparait" style={{ "--i": 2 } as React.CSSProperties}>
      <h2 className="titre-bloc">
        <ScanEye size={18} className="icone" aria-hidden /> Questions sur un schéma
      </h2>
      <p className="texte-doux petit">
        Gemini regarde ton dessin (avec ses légendes et la note de cours liée) et pose des questions dessus. Quand le
        schéma a des légendes, chaque question doit porter sur l'une d'elles, sinon elle est rejetée. Les questions
        rejoignent ta séance « Autres matières », avec le schéma affiché.
      </p>
      <div className="rangee">
        <select aria-label="Schéma pour les questions" value={choix} onChange={(e) => setChoix(e.target.value)}>
          <option value="">Choisis un schéma…</option>
          {matieres.map((m) => (
            <optgroup key={m} label={m}>
              {schemas
                .filter((s) => (s.matiere ?? "Hors matière") === m)
                .map((s) => (
                  <option key={s.chemin} value={s.chemin}>
                    {s.titre.replace(/\s*\(schéma\)$/i, "")}
                    {s.questionsIa ? ` (${s.questionsIa} questions déjà créées)` : ""}
                  </option>
                ))}
            </optgroup>
          ))}
        </select>
        <label className="rangee">
          Nombre
          <select value={nombre} onChange={(e) => setNombre(Number(e.target.value))}>
            {[3, 5, 8].map((n) => (
              <option key={n} value={n}>
                {n}
              </option>
            ))}
          </select>
        </label>
      </div>
      {schema && (
        <div className="apercu-schema bloc-haut">
          {scene === undefined ? (
            <p className="texte-doux petit">Chargement du dessin…</p>
          ) : (
            <Dessin scene={scene} titre={schema.titre} />
          )}
          <p className="texte-doux petit">
            {schema.legendes.length ? (
              <>
                {schema.legendes.length} légende{schema.legendes.length > 1 ? "s" : ""} :{" "}
                {schema.legendes.slice(0, 8).map((l, i) => (
                  <span key={i}>
                    {i > 0 && " · "}
                    <TexteMaths texte={l} />
                  </span>
                ))}
                {schema.legendes.length > 8 && " …"}
              </>
            ) : (
              "Pas de légende écrite sur ce schéma : les questions porteront sur le dessin lui-même."
            )}
            {schema.noteLiee ? " · note de cours liée trouvée" : ""}
          </p>
        </div>
      )}
      <div className="rangee bloc-haut">
        <button className="bouton bouton-principal" disabled={!disponible || !scene || enCours} onClick={generer}>
          <Sparkles size={16} className={enCours ? "icone pulse" : "icone"} aria-hidden />
          {enCours ? "Gemini regarde le schéma…" : "Créer les questions"}
        </button>
        <BoutonArreter tache={tache} />
        <span className="texte-doux petit">1 appel à Gemini (image + légendes)</span>
      </div>
      {erreur && <p className="message-erreur">{erreur}</p>}
      {tache?.statut === "erreur" && <p className="message-erreur">{tache.erreur}</p>}
      {tache?.statut === "interrompue" && <Message genre="info">Génération interrompue.</Message>}
      {resultat && (
        <div className="bloc-haut">
          <Message genre="succes">
            {tache?.titre} : <strong>{resultat.creees}</strong> question{resultat.creees > 1 ? "s" : ""} ajoutée
            {resultat.creees > 1 ? "s" : ""} à tes révisions
            {resultat.rejetees > 0 && `, ${resultat.rejetees} rejetée${resultat.rejetees > 1 ? "s" : ""}`}.
          </Message>
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
  );
}
