// Réglages : synchronisation avec le téléphone par le Google Drive de l'élève (dossier caché propre à Révisio).
import { useEffect, useState } from "react";
import { listen } from "@tauri-apps/api/event";
import { Check, Copy, LogOut, RefreshCw, Smartphone } from "lucide-react";
import { api, EtatSynchro, messageErreur, RapportSynchro } from "../lib/api";
import { useEnLigne } from "../lib/connexion";

const DATE = new Intl.DateTimeFormat("fr-FR", { weekday: "short", day: "numeric", month: "short", hour: "2-digit", minute: "2-digit" });

function pluriel(n: number, mot: string) {
  return `${n} ${mot}${n > 1 ? "s" : ""}`;
}

/** Résumé d'une synchronisation en une phrase. */
export function resumeRapport(r: RapportSynchro): string {
  if (r.erreur) return r.erreur;
  const parties: string[] = [];
  if (r.revisions) parties.push(`${pluriel(r.revisions, "révision")} reçue${r.revisions > 1 ? "s" : ""} du téléphone`);
  if (r.notesEcrites) parties.push(`${pluriel(r.notesEcrites, "note")} modifiée${r.notesEcrites > 1 ? "s" : ""}`);
  if (r.notesAValider) parties.push(`${pluriel(r.notesAValider, "modification")} à valider (Photos de cours)`);
  if (r.notesEnAttente) parties.push(`${pluriel(r.notesEnAttente, "note")} en attente : autorise l'écriture dans le vault`);
  if (r.envoye) parties.push(`notes et cartes envoyées (${Math.max(1, Math.round(r.taille / 1024))} Ko)`);
  return parties.length ? parties.join(", ") + "." : "Tout était déjà à jour.";
}

export default function ReglagesSynchroVue() {
  const enLigne = useEnLigne();
  const [etat, setEtat] = useState<EtatSynchro | null>(null);
  const [enCours, setEnCours] = useState<"connexion" | "synchro" | null>(null);
  const [erreur, setErreur] = useState<string | null>(null);
  const [copie, setCopie] = useState(false);

  const recharger = () => api.etatSynchro().then(setEtat).catch((e) => setErreur(messageErreur(e)));

  useEffect(() => {
    recharger();
    const arret = listen("synchro", () => recharger());
    return () => {
      arret.then((f) => f());
    };
  }, []);

  async function agir(genre: "connexion" | "synchro", f: () => Promise<unknown>) {
    setErreur(null);
    setEnCours(genre);
    try {
      await f();
    } catch (e) {
      setErreur(messageErreur(e));
    } finally {
      setEnCours(null);
      recharger();
    }
  }

  async function copier() {
    if (!etat) return;
    try {
      await navigator.clipboard.writeText(etat.adresseApp);
      setCopie(true);
      setTimeout(() => setCopie(false), 2000);
    } catch {
      setErreur("Impossible de copier l'adresse.");
    }
  }

  return (
    <section className="carte bloc">
      <h2 className="titre-bloc">
        <Smartphone size={18} className="icone" aria-hidden /> Révisio sur ton téléphone
      </h2>
      <p className="texte-doux petit">
        Révise et lis tes notes sur ton téléphone, même sans Internet. Tes cartes, ta progression et tes notes passent par
        ton propre Google Drive, dans un dossier caché réservé à Révisio : Révisio ne voit aucun autre fichier de ton Drive,
        et rien n'est stocké ailleurs.
      </p>
      {etat?.connecte ? (
        <>
          <p className="rangee">
            <Check size={15} aria-hidden /> Google Drive connecté. Synchronisation automatique toutes les 5 minutes tant que
            Révisio est ouvert.
          </p>
          {etat.dernier && (
            <p className={etat.dernier.erreur ? "message-erreur" : "petit"}>
              Dernière synchronisation, {DATE.format(etat.dernier.date)} : {resumeRapport(etat.dernier)}
            </p>
          )}
          <div className="rangee bloc-haut">
            <button
              className="bouton bouton-principal"
              disabled={enCours !== null || !enLigne}
              onClick={() => agir("synchro", api.synchroniserMaintenant)}
            >
              <RefreshCw size={15} aria-hidden /> Synchroniser maintenant
            </button>
            {enCours === "synchro" && <span className="progression-texte">Synchronisation…</span>}
            <button className="bouton" disabled={enCours !== null} onClick={() => agir("connexion", api.deconnecterGoogle)}>
              <LogOut size={15} aria-hidden /> Déconnecter
            </button>
          </div>
          <h3 className="sous-titre-bloc">Sur ton téléphone</h3>
          <ol className="petit">
            <li>
              Ouvre cette adresse dans Safari (iPhone) ou Chrome (Android) : <code>{etat.adresseApp}</code>{" "}
              <button className="lien-discret" onClick={copier}>
                {copie ? <Check size={13} aria-hidden /> : <Copy size={13} aria-hidden />} {copie ? "Copiée" : "Copier"}
              </button>
            </li>
            <li>Ajoute-la à l'écran d'accueil (iPhone : bouton Partager, puis « Sur l'écran d'accueil »).</li>
            <li>Ouvre Révisio depuis l'écran d'accueil et connecte le même compte Google.</li>
          </ol>
        </>
      ) : (
        <div className="rangee bloc-haut">
          <button
            className="bouton bouton-principal"
            disabled={enCours !== null || !enLigne}
            onClick={() => agir("connexion", api.connecterGoogle)}
          >
            <Smartphone size={15} aria-hidden /> Connecter mon Google Drive
          </button>
          {enCours === "connexion" && (
            <span className="progression-texte">Accepte la connexion dans ton navigateur, puis reviens ici…</span>
          )}
          {!enLigne && <span className="texte-doux petit">Il faut Internet pour se connecter.</span>}
        </div>
      )}
      {erreur && <p className="message-erreur">{erreur}</p>}
    </section>
  );
}
