// Cartes mentales : le plan du chapitre (sans IA, toujours disponible) ou une carte rédigée par
// l'IA (Gemini, ou l'IA locale) à partir des notes, enregistrée pour être consultée hors ligne.
import { useEffect, useMemo, useState } from "react";
import { KeyRound, ListTree, Network, Sparkles, Trash } from "lucide-react";
import EntetePage from "../composants/EntetePage";
import { api, CarteMentale as CarteEnregistree, ChapitreIa, messageErreur, Noeud } from "../lib/api";
import { nomModele, useMoteurIa } from "../lib/moteurIa";
import CarteMentale from "../cartes/CarteMentale";
import BadgeIa from "./BadgeIa";
import Message from "../composants/Message";
import { lancerTache, useDerniereTache } from "../lib/taches";
import { BoutonArreter } from "../composants/TachesIa";

interface Props {
  onOuvrirNote: (chemin: string) => void;
  onReglages: () => void;
  /** Chapitre à afficher en arrivant (depuis l'Atelier IA ; nouvel objet à chaque demande). */
  demande?: { matiere: string; chapitre: string | null } | null;
}

const cle = (c: { matiere: string; chapitre: string | null }) => `${c.matiere}|${c.chapitre ?? ""}`;
const formatDate = new Intl.DateTimeFormat("fr-FR", { day: "numeric", month: "short", hour: "2-digit", minute: "2-digit" });

export default function CartesMentalesVue({ onOuvrirNote, onReglages, demande }: Props) {
  const moteur = useMoteurIa();
  const { enLigne, etat } = moteur;
  const [chapitres, setChapitres] = useState<ChapitreIa[]>([]);
  // Génération en arrière-plan : au retour sur l'écran, on retrouve le chapitre de la dernière carte demandée.
  const tache = useDerniereTache("carte-mentale");
  const enCours = tache?.statut === "en-cours";
  const [choix, setChoix] = useState(() => (demande ? cle(demande) : (tache?.cle ?? "")));
  const [plan, setPlan] = useState<Noeud | null>(null);
  const [cartes, setCartes] = useState<CarteEnregistree[]>([]);
  /** « plan » ou l'identifiant d'une carte IA. */
  const [affichee, setAffichee] = useState<"plan" | number>("plan");
  const [aSupprimer, setASupprimer] = useState(false);
  const [erreur, setErreur] = useState<string | null>(null);

  useEffect(() => {
    api.chapitresIa().then(setChapitres).catch((e) => setErreur(messageErreur(e)));
  }, []);

  const chapitre = useMemo(() => chapitres.find((c) => cle(c) === choix) ?? null, [chapitres, choix]);
  const matieres = [...new Set(chapitres.map((c) => c.matiere))];

  useEffect(() => {
    setPlan(null);
    setCartes([]);
    setAffichee("plan");
    setASupprimer(false);
    if (!chapitre) return;
    api.planChapitre(chapitre.matiere, chapitre.chapitre).then(setPlan).catch((e) => setErreur(messageErreur(e)));
    api
      .cartesMentalesChapitre(chapitre.matiere, chapitre.chapitre)
      .then((c) => {
        setCartes(c);
        // La carte IA la plus récente est montrée en premier quand il y en a une.
        if (c.length) setAffichee(c[0].id);
      })
      .catch((e) => setErreur(messageErreur(e)));
  }, [chapitre]);

  // Carte terminée pendant qu'on regarde ce chapitre : elle s'affiche aussitôt.
  useEffect(() => {
    if (tache?.statut !== "finie" || tache.cle !== choix) return;
    const c = tache.resultat as CarteEnregistree;
    setCartes((l) => (l.some((x) => x.id === c.id) ? l : [c, ...l]));
    setAffichee(c.id);
  }, [tache?.statut]);

  function generer() {
    if (!chapitre) return;
    setErreur(null);
    lancerTache(
      {
        genre: "carte-mentale",
        cle: cle(chapitre),
        titre: `Carte mentale · ${chapitre.chapitre ?? chapitre.matiere}`,
        ecran: "cartes",
        resume: () => "Ta carte mentale est prête",
      },
      ({ id }) => api.genererCarteMentale(chapitre.matiere, chapitre.chapitre, id),
    ).catch(() => {});
  }

  async function supprimer(id: number) {
    setASupprimer(false);
    try {
      await api.supprimerCarteMentale(id);
      setCartes((l) => l.filter((c) => c.id !== id));
      setAffichee("plan");
    } catch (e) {
      setErreur(messageErreur(e));
    }
  }

  const carteIa = typeof affichee === "number" ? cartes.find((c) => c.id === affichee) : undefined;
  const arbre = carteIa?.arbre ?? plan;
  const disponible = moteur.simple;

  return (
    <div className="page page-large">
      <EntetePage
        icone={Network}
        surTitre="Créer avec l'IA"
        titre="Cartes mentales"
        description="Le plan de chaque chapitre, sans IA, ou une carte qui relie les notions entre elles."
      />
      <section className="carte bloc apparait" style={{ "--i": 1 } as React.CSSProperties}>
        <div className="rangee">
          <select aria-label="Chapitre de la carte mentale" value={choix} onChange={(e) => setChoix(e.target.value)}>
            <option value="">Choisis un chapitre…</option>
            {matieres.map((m) => (
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
          {chapitre && (
            <>
              <button className="bouton bouton-principal" disabled={!disponible || enCours} onClick={generer}>
                <Sparkles size={16} className={enCours ? "icone pulse" : "icone"} aria-hidden />
                {enCours
                  ? moteur.local
                    ? "L'IA locale dessine la carte… (plusieurs minutes)"
                    : "Gemini dessine la carte…"
                  : cartes.length
                    ? "Nouvelle carte avec l'IA"
                    : "Créer la carte avec l'IA"}
              </button>
              <span className="texte-doux petit">
                {moteur.local
                  ? "1 appel à l'IA locale"
                  : !enLigne
                    ? "Hors ligne : le plan et les cartes déjà créées restent disponibles."
                    : etat && !etat.clePresente
                      ? ""
                      : "1 appel à Gemini"}
              </span>
              <BoutonArreter tache={tache} />
              <BadgeIa moteur={moteur} />
              {enLigne && etat && !etat.clePresente && !moteur.local && (
                <button className="lien-discret petit" onClick={onReglages}>
                  <KeyRound size={13} aria-hidden /> Ajouter une clé Gemini
                </button>
              )}
            </>
          )}
        </div>
        {chapitre && (
          <div className="rangee bloc-haut onglets-carte" role="tablist" aria-label="Carte affichée">
            <button
              role="tab"
              aria-selected={affichee === "plan"}
              className={"onglet" + (affichee === "plan" ? " actif" : "")}
              onClick={() => setAffichee("plan")}
            >
              <ListTree size={15} aria-hidden /> Plan du chapitre (sans IA)
            </button>
            {cartes.map((c, i) => (
              <button
                key={c.id}
                role="tab"
                aria-selected={affichee === c.id}
                className={"onglet" + (affichee === c.id ? " actif" : "")}
                onClick={() => {
                  setAffichee(c.id);
                  setASupprimer(false);
                }}
              >
                <Sparkles size={14} aria-hidden /> Carte IA{cartes.length > 1 ? ` n° ${cartes.length - i}` : ""} · {formatDate.format(new Date(c.date))}
              </button>
            ))}
          </div>
        )}
        {enCours && tache?.cle !== choix && <p className="texte-doux petit">En cours : {tache?.titre}.</p>}
        {erreur && <p className="message-erreur">{erreur}</p>}
        {tache?.cle === choix && tache?.statut === "erreur" && <p className="message-erreur">{tache.erreur}</p>}
        {tache?.cle === choix && tache?.statut === "interrompue" && <Message genre="info">Création interrompue.</Message>}
      </section>

      {!chapitre ? (
        <div className="carte etat-vide apparait" style={{ "--i": 2 } as React.CSSProperties}>
          <Network size={30} strokeWidth={1.6} aria-hidden />
          <p>
            Choisis un chapitre : le plan du chapitre s'affiche tout de suite, sans IA. Avec l'IA, tu peux aussi créer
            une carte qui relie les notions entre elles, à partir de tes notes uniquement.
          </p>
        </div>
      ) : arbre ? (
        <section className="carte bloc apparait" key={String(affichee)}>
          <CarteMentale arbre={arbre} onOuvrirNote={onOuvrirNote} />
          {carteIa && (
            <div className="rangee fin bloc-haut">
              <span className="texte-doux petit">Rédigée par {nomModele(carteIa.modele)} à partir de tes notes.</span>
              {aSupprimer ? (
                <>
                  <button className="bouton bouton-danger" onClick={() => supprimer(carteIa.id)}>
                    <Trash size={14} aria-hidden /> Supprimer cette carte
                  </button>
                  <button className="lien-discret petit" onClick={() => setASupprimer(false)}>
                    Annuler
                  </button>
                </>
              ) : (
                <button className="lien-discret petit" onClick={() => setASupprimer(true)}>
                  <Trash size={13} aria-hidden /> Supprimer cette carte
                </button>
              )}
            </div>
          )}
        </section>
      ) : (
        <p className="texte-doux">Pas de notes dans ce chapitre.</p>
      )}
    </div>
  );
}
