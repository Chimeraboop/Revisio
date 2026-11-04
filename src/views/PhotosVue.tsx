// Photos de cours → notes du vault (Gemini avec vision, ou une extension installée). Import manuel ou dossier surveillé
// (par exemple un dossier Google Drive synchronisé : une photo prise au lycée devient une note).
// Les nouvelles notes sont écrites aussitôt (annulables) ; une note existante n'est modifiée
// qu'après l'accord de l'élève.
import { useEffect, useState } from "react";
import { listen } from "@tauri-apps/api/event";
import { open } from "@tauri-apps/plugin-dialog";
import { Camera, Check, FilePen, FolderSync, ImagePlus, KeyRound, RotateCcw, Sparkles, X } from "lucide-react";
import EntetePage from "../composants/EntetePage";
import JournalVault from "../composants/JournalVault";
import Message from "../composants/Message";
import { BoutonArreter } from "../composants/TachesIa";
import { api, EtatSurveillancePhotos, Extension, LotPhotos, messageErreur, PropositionVault, Reglages } from "../lib/api";
import { useEnLigne, useModeLocal } from "../lib/connexion";
import { nomModele } from "../lib/moteurIa";
import { diffLignes, replier } from "../lib/diff";
import { lancerTache, useDerniereTache } from "../lib/taches";

interface Props {
  reglages: Reglages;
  onReglages: (r: Reglages) => void;
  onOuvrirNote: (chemin: string) => void;
  onAllerAuxReglages: () => void;
}

const EXTENSIONS = ["jpg", "jpeg", "png", "webp", "heic", "heif", "pdf"];

/** « 3 photos », « 1 PDF », « 2 photos et 1 PDF ». */
function decrireFichiers(chemins: string[]): string {
  const pdf = chemins.filter((c) => /\.pdf$/i.test(c)).length;
  const photos = chemins.length - pdf;
  const morceaux = [];
  if (photos > 0) morceaux.push(`${photos} photo${photos > 1 ? "s" : ""}`);
  if (pdf > 0) morceaux.push(`${pdf} PDF`);
  return morceaux.join(" et ");
}
const DATE = new Intl.DateTimeFormat("fr-FR", { weekday: "short", day: "numeric", month: "short", hour: "2-digit", minute: "2-digit" });
const STATUTS: Record<string, string> = {
  attente: "En attente",
  en_cours: "En cours",
  termine: "Terminé",
  illisible: "Rien à noter",
  erreur: "Erreur",
};

export default function PhotosVue({ reglages, onReglages, onOuvrirNote, onAllerAuxReglages }: Props) {
  const enLigne = useEnLigne();
  const modeLocal = useModeLocal();
  const [clePresente, setClePresente] = useState<boolean | null>(null);
  const [lots, setLots] = useState<LotPhotos[]>([]);
  const [propositions, setPropositions] = useState<PropositionVault[]>([]);
  const [surveillance, setSurveillance] = useState<EtatSurveillancePhotos | null>(null);
  const [extensions, setExtensions] = useState<Extension[]>([]);
  const [erreur, setErreur] = useState<string | null>(null);
  const [version, setVersion] = useState(0);
  const tache = useDerniereTache("photos");
  const enCours = tache?.statut === "en-cours";

  const recharger = () => {
    api.lotsPhotos().then(setLots).catch((e) => setErreur(messageErreur(e)));
    api.propositionsVault().then(setPropositions).catch((e) => setErreur(messageErreur(e)));
    api.etatPhotos().then(setSurveillance).catch(() => {});
    setVersion((v) => v + 1);
  };

  useEffect(() => {
    recharger();
    api.etatIa().then((e) => setClePresente(e.clePresente)).catch(() => setClePresente(null));
    api
      .extensionsInstallees()
      .then((l) => setExtensions(l.filter((e) => e.taches.includes("photos"))))
      .catch(() => setExtensions([]));
    const arrets = [
      listen("photos-lot", () => recharger()),
      listen<EtatSurveillancePhotos>("photos-surveillance", (e) => setSurveillance(e.payload)),
    ];
    return () => arrets.forEach((a) => a.then((f) => f()));
  }, []);

  useEffect(() => {
    if (tache && tache.statut !== "en-cours") recharger();
  }, [tache?.statut]);

  // Rédacteur des notes : Gemini, ou l'extension choisie (qui n'a pas besoin de la clé Gemini).
  const extension = extensions.find((e) => e.id === reglages.extensionPhotos) ?? null;
  const pret = enLigne && !modeLocal && (extension !== null || clePresente !== false);

  async function choisirRedacteur(id: string | null) {
    setErreur(null);
    try {
      onReglages(await api.definirExtensionPhotos(id));
    } catch (e) {
      setErreur(messageErreur(e));
    }
  }

  async function importer() {
    setErreur(null);
    const choix = await open({
      multiple: true,
      title: "Choisir les photos ou PDF d'un même cours",
      filters: [{ name: "Photos et PDF", extensions: EXTENSIONS }],
    });
    const chemins = Array.isArray(choix) ? choix : typeof choix === "string" ? [choix] : [];
    if (chemins.length === 0) return;
    traiter(decrireFichiers(chemins), (id) => api.importerPhotos(chemins, id));
  }

  function traiter(titre: string, travail: (tache: string) => Promise<LotPhotos>) {
    lancerTache(
      { genre: "photos", titre: `Photos · ${titre}`, ecran: "photos", resume: (l: LotPhotos) => l.message ?? STATUTS[l.statut] },
      ({ id }) => travail(id),
    ).catch((e) => setErreur(messageErreur(e)));
  }

  async function autoriser() {
    try {
      onReglages(await api.definirEcritureAutorisee(true));
    } catch (e) {
      setErreur(messageErreur(e));
    }
  }

  return (
    <div className="page">
      <EntetePage
        icone={Camera}
        surTitre="Créer avec l'IA"
        titre="Photos de cours"
        description="Tes photos ou PDF de cours deviennent des notes dans ton vault, rédigées comme les tiennes, puis des cartes de révision."
      />

      {!reglages.ecritureAutorisee && (
        <Message genre="attention">
          Révisio n'a pas le droit d'écrire dans ton vault : aucune note ne peut être créée. Chaque écriture est
          sauvegardée et annulable, et une note existante n'est jamais modifiée sans ton accord.{" "}
          <button className="lien-discret" onClick={autoriser}>
            <FilePen size={13} aria-hidden /> Autoriser l'écriture
          </button>
        </Message>
      )}
      {modeLocal && (
        <Message genre="info">Mode local : la lecture des photos demande {extension ? extension.nom : "Gemini"}. Repasse « En ligne ».</Message>
      )}
      {!enLigne && !modeLocal && (
        <Message genre="hors-ligne">Hors ligne : la lecture des photos demande Internet. Les photos du dossier attendront.</Message>
      )}
      {reglages.extensionPhotos && extensions.length > 0 && !extension && (
        <Message genre="attention">
          L'extension choisie pour les photos n'est plus installée.{" "}
          <button className="lien-discret" onClick={() => choisirRedacteur(null)}>
            Revenir à Gemini
          </button>
        </Message>
      )}
      {enLigne && !modeLocal && !extension && clePresente === false && (
        <Message genre="attention">
          Aucune clé Gemini.{" "}
          <button className="lien-discret" onClick={onAllerAuxReglages}>
            <KeyRound size={13} aria-hidden /> Ajouter une clé dans les Réglages
          </button>
        </Message>
      )}
      {erreur && <p className="message-erreur">{erreur}</p>}

      {propositions.length > 0 && (
        <section className="carte bloc">
          <h2 className="titre-bloc">
            <FilePen size={18} className="icone" aria-hidden /> Modifications à valider ({propositions.length})
          </h2>
          <p className="texte-doux petit">
            L'IA propose de compléter des notes que tu avais déjà. Rien n'est changé tant que tu n'as pas accepté ; une
            modification acceptée peut encore être annulée.
          </p>
          {propositions.map((p) => (
            <PropositionCarte key={p.id} proposition={p} onOuvrir={onOuvrirNote} onFini={recharger} />
          ))}
        </section>
      )}

      <section className="carte bloc">
        <h2 className="titre-bloc">
          <ImagePlus size={18} className="icone" aria-hidden /> Importer des photos ou PDF
        </h2>
        <p className="texte-doux petit">
          Choisis les photos ou les PDF d'un même cours (10 fichiers au plus). Révisio reconnaît la matière et le chapitre, range les notes
          dans le bon dossier et en fait une note par notion.
        </p>
        {extensions.length > 0 && (
          <div className="rangee">
            <span className="petit texte-doux">Rédigées par :</span>
            <div className="segmente" role="radiogroup" aria-label="Qui lit les photos et rédige les notes">
              {[{ id: null, nom: "Gemini" }, ...extensions].map((e) => (
                <button
                  key={e.id ?? "gemini"}
                  role="radio"
                  aria-checked={(extension?.id ?? null) === e.id}
                  className={"segment" + ((extension?.id ?? null) === e.id ? " actif" : "")}
                  disabled={enCours}
                  onClick={() => choisirRedacteur(e.id)}
                >
                  {e.id && <Sparkles size={13} aria-hidden />} {e.nom}
                </button>
              ))}
            </div>
            {extension?.description && <span className="petit texte-doux">{extension.description}</span>}
          </div>
        )}
        <div className="rangee">
          <button className="bouton bouton-principal" disabled={!reglages.ecritureAutorisee || !pret || enCours} onClick={importer}>
            <ImagePlus size={15} aria-hidden /> Choisir des photos ou PDF…
          </button>
          {enCours && tache && (
            <>
              <span className="progression-texte">Lecture des photos et rédaction des notes…</span>
              <BoutonArreter tache={tache} />
            </>
          )}
        </div>
        {tache?.statut === "erreur" && <p className="message-erreur">{tache.erreur}</p>}
      </section>

      <DossierSurveille reglages={reglages} surveillance={surveillance} onReglages={onReglages} onChange={recharger} />

      <section className="carte bloc">
        <h2 className="titre-bloc">
          <Camera size={18} className="icone" aria-hidden /> Derniers lots de photos
        </h2>
        {lots.length === 0 ? (
          <p className="texte-doux petit">Aucune photo traitée pour l'instant.</p>
        ) : (
          <ul className="lots-photos">
            {lots.map((l) => (
              <li key={l.id}>
                <div className="rangee entre">
                  <span>
                    <strong>{decrireFichiers(l.photos)}</strong>{" "}
                    <span className="texte-doux petit">
                      {l.source === "dossier" ? "du dossier surveillé" : "import manuel"}, {DATE.format(l.date)}
                      {l.chapitre ? `, ${l.matiere} › ${l.chapitre}` : ""}
                      {l.modele ? `, par ${nomModele(l.modele)}` : ""}
                    </span>
                  </span>
                  <span className={`etiquette statut-lot statut-${l.statut}`}>{STATUTS[l.statut] ?? l.statut}</span>
                </div>
                {l.message && <p className={l.statut === "erreur" ? "message-erreur" : "petit"}>{l.message}</p>}
                {(l.statut === "erreur" || l.statut === "illisible") && (
                  <button
                    className="bouton bouton-petit"
                    disabled={!reglages.ecritureAutorisee || !pret || enCours}
                    onClick={() => traiter(decrireFichiers(l.photos), (id) => api.relancerLotPhotos(l.id, id))}
                  >
                    <RotateCcw size={14} aria-hidden /> Réessayer
                  </button>
                )}
                <JournalVault lot={l.id} version={version} onOuvrir={onOuvrirNote} />
              </li>
            ))}
          </ul>
        )}
      </section>
    </div>
  );
}

function PropositionCarte({
  proposition: p,
  onOuvrir,
  onFini,
}: {
  proposition: PropositionVault;
  onOuvrir: (chemin: string) => void;
  onFini: () => void;
}) {
  const [erreur, setErreur] = useState<string | null>(null);
  const nom = p.chemin.split("/").pop()?.replace(/\.md$/i, "");

  async function agir(f: () => Promise<unknown>) {
    setErreur(null);
    try {
      await f();
      onFini();
    } catch (e) {
      setErreur(messageErreur(e));
      onFini();
    }
  }

  return (
    <div className="proposition">
      <div className="rangee entre">
        <span>
          <a href="#" onClick={(e) => (e.preventDefault(), onOuvrir(p.chemin))}>
            <strong>{nom}</strong>
          </a>{" "}
          <span className="texte-doux petit">{p.raison}</span>
        </span>
        <span className="rangee">
          <button className="bouton bouton-principal bouton-petit" onClick={() => agir(() => api.accepterProposition(p.id))}>
            <Check size={14} aria-hidden /> Accepter
          </button>
          <button className="bouton bouton-petit" onClick={() => agir(() => api.refuserProposition(p.id))}>
            <X size={14} aria-hidden /> Refuser
          </button>
        </span>
      </div>
      <pre className="diff" aria-label="Différences entre la note actuelle et la version proposée">
        {replier(diffLignes(p.avant, p.apres)).map((m, i) =>
          m.genre === "replie" ? (
            <span key={i} className="diff-replie">
              {m.nombre} ligne{m.nombre > 1 ? "s" : ""} inchangée{m.nombre > 1 ? "s" : ""}
            </span>
          ) : (
            <span key={i} className={`diff-${m.genre}`}>
              {m.genre === "ajoutee" ? "+ " : m.genre === "retiree" ? "− " : "  "}
              {m.texte || " "}
            </span>
          ),
        )}
      </pre>
      {erreur && <p className="message-erreur">{erreur}</p>}
    </div>
  );
}

export function DossierSurveille({
  reglages,
  surveillance,
  onReglages,
  onChange,
}: {
  reglages: Reglages;
  surveillance: EtatSurveillancePhotos | null;
  onReglages: (r: Reglages) => void;
  onChange: () => void;
}) {
  const [drives, setDrives] = useState<string[]>([]);
  // Dossier choisi qui contient déjà des photos : l'élève dit quoi en faire.
  const [enAttente, setEnAttente] = useState<{ chemin: string; nombre: number } | null>(null);
  const [erreur, setErreur] = useState<string | null>(null);

  useEffect(() => {
    api.dossiersDrive().then(setDrives).catch(() => {});
  }, []);

  async function choisir(depart?: string) {
    setErreur(null);
    const choix = await open({ directory: true, defaultPath: depart, title: "Dossier où arrivent tes photos de cours" });
    if (typeof choix !== "string") return;
    const nombre = await api.compterPhotosDossier(choix);
    if (nombre > 0) setEnAttente({ chemin: choix, nombre });
    else definir(choix, false);
  }

  async function definir(chemin: string | null, traiterExistantes: boolean) {
    setErreur(null);
    setEnAttente(null);
    try {
      onReglages(await api.definirDossierPhotos(chemin, traiterExistantes));
      onChange();
    } catch (e) {
      setErreur(messageErreur(e));
    }
  }

  return (
    <section className="carte bloc">
      <h2 className="titre-bloc">
        <FolderSync size={18} className="icone" aria-hidden /> Dossier surveillé
      </h2>
      <p className="texte-doux petit">
        Avec Google Drive pour ordinateur (ou OneDrive, Dropbox), envoie tes photos (ou tes PDF) depuis ton téléphone dans un
        dossier : tant que Révisio est ouvert, il regarde ce dossier toutes les 15 secondes et transforme les nouveaux
        fichiers en notes. Ils ne sont jamais supprimés.
      </p>
      {reglages.dossierPhotos ? (
        <>
          <p>
            Dossier : <code>{reglages.dossierPhotos}</code>
          </p>
          {surveillance?.message ? (
            <p className={surveillance.enCours ? "progression-texte" : "texte-doux petit"}>{surveillance.message}</p>
          ) : (
            <p className="texte-doux petit">
              {surveillance?.enAttente
                ? `${surveillance.enAttente} nouvelle${surveillance.enAttente > 1 ? "s" : ""} photo${surveillance.enAttente > 1 ? "s" : ""} : traitement dès qu'aucune autre n'arrive pendant une minute.`
                : "En attente de nouvelles photos."}
            </p>
          )}
          <div className="rangee">
            <button className="bouton" onClick={() => choisir(reglages.dossierPhotos ?? undefined)}>
              <FolderSync size={15} aria-hidden /> Changer de dossier
            </button>
            <button className="bouton" onClick={() => definir(null, false)}>
              <X size={15} aria-hidden /> Arrêter la surveillance
            </button>
          </div>
        </>
      ) : (
        <div className="rangee">
          {drives.map((d) => (
            <button key={d} className="bouton" onClick={() => choisir(d)}>
              <FolderSync size={15} aria-hidden /> Choisir dans {d}
            </button>
          ))}
          <button className="bouton" onClick={() => choisir()}>
            <FolderSync size={15} aria-hidden /> Choisir un dossier…
          </button>
        </div>
      )}
      {enAttente && (
        <Message genre="info">
          Ce dossier contient déjà {enAttente.nombre} photo{enAttente.nombre > 1 ? "s" : ""}.{" "}
          <span className="rangee">
            <button className="bouton bouton-principal bouton-petit" onClick={() => definir(enAttente.chemin, false)}>
              Ne traiter que les nouvelles
            </button>
            <button className="bouton bouton-petit" onClick={() => definir(enAttente.chemin, true)}>
              Les transformer aussi en notes
            </button>
          </span>
        </Message>
      )}
      {erreur && <p className="message-erreur">{erreur}</p>}
    </section>
  );
}
