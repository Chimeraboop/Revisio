// Synchronisation sur le téléphone : état, bouton, compte Google.
import { CloudOff, LogOut, RefreshCw } from "lucide-react";
import { connecter } from "./drive";
import { seDeconnecter, synchroniser, useMagasin } from "./magasin";

const DATE = new Intl.DateTimeFormat("fr-FR", { weekday: "short", day: "numeric", month: "short", hour: "2-digit", minute: "2-digit" });

/** Petit état de la synchronisation, dans la barre du haut. */
export function ResumeSynchro() {
  const { synchro } = useMagasin();
  if (synchro.enCours) return <span className="progression-texte petit">Synchro…</span>;
  if (synchro.reconnecter)
    return (
      <button className="lien-discret petit" onClick={() => connecter()}>
        Reconnecter Google
      </button>
    );
  // Connexion à Google expirée pendant que l'app était ouverte : un appui la renouvelle et va chercher les nouveautés.
  if (synchro.perime)
    return (
      <button className="lien-discret petit" onClick={() => synchroniser()}>
        <RefreshCw size={13} aria-hidden /> Synchroniser
      </button>
    );
  return null;
}

export default function CompteMobile() {
  const { modele, connecte, synchro } = useMagasin();
  const journal = modele?.journal;
  const enAttente = journal ? journal.revisions.length + journal.notes.length : 0;
  return (
    <div className="page">
      <section className="carte bloc">
        <h2 className="titre-bloc">
          <RefreshCw size={18} className="icone" aria-hidden /> Synchronisation
        </h2>
        <p className="petit">
          Tes réponses et tes notes modifiées sont gardées sur ce téléphone, puis envoyées dans ton Google Drive. Révisio sur
          ton ordinateur les récupère (toutes les 5 minutes quand il est ouvert), et t'envoie ses nouvelles cartes et notes.
        </p>
        {modele && (
          <p className="petit">
            Contenu reçu de l'ordinateur : {DATE.format(modele.contenu.genereLe)} ({modele.cartes.length} cartes, {modele.notes.size} notes).
          </p>
        )}
        {synchro.derniere && <p className="petit">Dernière synchronisation : {DATE.format(synchro.derniere)}.</p>}
        {enAttente > 0 && (
          <p className="petit">
            {enAttente} élément{enAttente > 1 ? "s" : ""} pas encore pris en compte par l'ordinateur.
          </p>
        )}
        {!navigator.onLine && (
          <p className="rangee petit">
            <CloudOff size={14} aria-hidden /> Hors ligne : tout est gardé sur le téléphone en attendant.
          </p>
        )}
        {synchro.erreur && <p className="message-erreur">{synchro.erreur}</p>}
        <div className="rangee bloc-haut">
          {connecte && !synchro.reconnecter ? (
            <button className="bouton bouton-principal" disabled={synchro.enCours} onClick={() => synchroniser()}>
              <RefreshCw size={15} aria-hidden /> {synchro.enCours ? "Synchronisation…" : "Synchroniser maintenant"}
            </button>
          ) : (
            <button className="bouton bouton-principal" onClick={() => connecter()}>
              <RefreshCw size={15} aria-hidden /> Connecter mon Google Drive
            </button>
          )}
          {connecte && (
            <button className="bouton" onClick={() => seDeconnecter()}>
              <LogOut size={15} aria-hidden /> Déconnecter
            </button>
          )}
        </div>
      </section>
    </div>
  );
}
