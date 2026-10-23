// Guide de démarrage : s'affiche au premier lancement (et depuis les Réglages).
// Étapes : bienvenue → vault Obsidian → clé Gemini (facultative) → photos de cours (facultatif) → récapitulatif.
import { useEffect, useState } from "react";
import {
  ArrowLeft,
  ArrowRight,
  Camera,
  CircleCheck,
  ExternalLink,
  KeyRound,
  LoaderCircle,
  Lock,
  Sparkles,
  WifiOff,
} from "lucide-react";
import { api, EtatIa, InfosApp, messageErreur, Reglages } from "../lib/api";
import { useIndexation } from "../lib/indexation";
import Message from "../composants/Message";
import ChoixVault from "./ChoixVault";
import { DossierSurveille } from "./PhotosVue";

interface Props {
  reglages: Reglages;
  infos: InfosApp;
  onReglages: (r: Reglages) => void;
  onTermine: (r: Reglages) => void;
}

const ETAPES = ["Bienvenue", "Ton vault", "Clé Gemini", "Photos de cours", "C'est prêt"];
const ADRESSE_CLE = "https://aistudio.google.com/apikey";

export default function GuideDemarrage({ reglages, infos, onReglages, onTermine }: Props) {
  const [etape, setEtape] = useState(0);
  const [etatIa, setEtatIa] = useState<EtatIa | null>(null);
  const [erreur, setErreur] = useState<string | null>(null);

  useEffect(() => {
    api.etatIa().then(setEtatIa).catch(() => setEtatIa(null));
  }, [etape]);

  async function terminer() {
    try {
      onTermine(await api.definirAccueilTermine(true));
    } catch (e) {
      setErreur(messageErreur(e));
    }
  }

  return (
    <div className="page guide">
      <ol className="guide-etapes" aria-label="Étapes du guide">
        {ETAPES.map((nom, i) => (
          <li
            key={nom}
            className={i === etape ? "actuelle" : i < etape ? "faite" : undefined}
            aria-current={i === etape ? "step" : undefined}
          >
            <span className="guide-pastille">{i < etape ? <CircleCheck size={14} aria-hidden /> : i + 1}</span>
            {nom}
          </li>
        ))}
      </ol>

      <div className="carte guide-carte apparait" key={etape}>
        {etape === 0 && <Bienvenue />}
        {etape === 1 && <EtapeVault reglages={reglages} infos={infos} onReglages={onReglages} />}
        {etape === 2 && <EtapeCle etatIa={etatIa} onChange={() => api.etatIa().then(setEtatIa)} />}
        {etape === 3 && <EtapePhotos reglages={reglages} onReglages={onReglages} />}
        {etape === 4 && <Recapitulatif reglages={reglages} etatIa={etatIa} />}

        {erreur && <p className="message-erreur">{erreur}</p>}

        <div className="guide-actions">
          {etape > 0 && (
            <button className="bouton" onClick={() => setEtape(etape - 1)}>
              <ArrowLeft size={15} aria-hidden /> Retour
            </button>
          )}
          <span className="espace" />
          {etape === 0 && (
            <button className="bouton bouton-principal" onClick={() => setEtape(1)}>
              Commencer <ArrowRight size={15} aria-hidden />
            </button>
          )}
          {etape === 1 && (
            <button className="bouton bouton-principal" disabled={!reglages.cheminVault} onClick={() => setEtape(2)}>
              Continuer <ArrowRight size={15} aria-hidden />
            </button>
          )}
          {etape === 2 && (
            <button className={"bouton" + (etatIa?.clePresente ? " bouton-principal" : "")} onClick={() => setEtape(3)}>
              {etatIa?.clePresente ? "Continuer" : "Passer cette étape"} <ArrowRight size={15} aria-hidden />
            </button>
          )}
          {etape === 3 && (
            <button className={"bouton" + (reglages.ecritureAutorisee ? " bouton-principal" : "")} onClick={() => setEtape(4)}>
              {reglages.ecritureAutorisee ? "Continuer" : "Passer cette étape"} <ArrowRight size={15} aria-hidden />
            </button>
          )}
          {etape === 4 && (
            <button className="bouton bouton-principal" onClick={terminer}>
              Ouvrir Révisio <ArrowRight size={15} aria-hidden />
            </button>
          )}
        </div>
      </div>
    </div>
  );
}

function Bienvenue() {
  return (
    <div className="guide-bienvenue">
      <img className="logo-image logo-grand" src="/logo.png" alt="" />
      <h1>Bienvenue dans Révisio</h1>
      <p className="texte-doux">
        Révisio transforme tes notes Obsidian en cartes de révision et te prépare chaque jour une séance. Deux minutes
        suffisent pour tout régler.
      </p>
      <ul className="guide-promesses">
        <li>
          <Lock size={18} className="icone" aria-hidden />
          <span>
            <strong>Tes notes ne sont jamais modifiées.</strong> Révisio les lit, rien de plus.
          </span>
        </li>
        <li>
          <WifiOff size={18} className="icone" aria-hidden />
          <span>
            <strong>Tes révisions marchent sans Internet.</strong> Tout reste sur ton ordinateur.
          </span>
        </li>
        <li>
          <Sparkles size={18} className="icone" aria-hidden />
          <span>
            <strong>L'IA est facultative.</strong> Tu peux l'ajouter maintenant ou plus tard.
          </span>
        </li>
        <li>
          <Camera size={18} className="icone" aria-hidden />
          <span>
            <strong>Tes photos de cours peuvent devenir des notes,</strong> si tu autorises Révisio à écrire dans ton
            vault.
          </span>
        </li>
      </ul>
    </div>
  );
}

function EtapeVault({
  reglages,
  infos,
  onReglages,
}: {
  reglages: Reglages;
  infos: InfosApp;
  onReglages: (r: Reglages) => void;
}) {
  const indexation = useIndexation();
  return (
    <>
      <h2>Où sont tes notes ?</h2>
      <p className="texte-doux">
        Choisis le dossier de ton vault Obsidian : c'est celui qui contient un sous-dossier caché <code>.obsidian</code>.
      </p>
      <ChoixVault cheminActuel={reglages.cheminVault} suggestions={infos.vaultsDetectes} onChoisi={onReglages} />
      {reglages.cheminVault &&
        (indexation?.enCours || !indexation?.dernier ? (
          <p className="progression-texte">Lecture de tes notes…</p>
        ) : (
          <Message genre="succes">
            {indexation.dernier.total} note{indexation.dernier.total > 1 ? "s" : ""} trouvée
            {indexation.dernier.total > 1 ? "s" : ""}
            {indexation.cartes ? `, ${indexation.cartes.actives} cartes de révision prêtes` : ""}.
          </Message>
        ))}
      {indexation?.erreur && <p className="message-erreur">{indexation.erreur}</p>}
    </>
  );
}

function EtapeCle({ etatIa, onChange }: { etatIa: EtatIa | null; onChange: () => void }) {
  const [cle, setCle] = useState("");
  const [enCours, setEnCours] = useState(false);
  const [message, setMessage] = useState<{ texte: string; erreur: boolean } | null>(null);

  async function enregistrer() {
    if (!cle.trim()) return;
    setEnCours(true);
    setMessage(null);
    try {
      setMessage({ texte: await api.enregistrerCleIa(cle.trim()), erreur: false });
      setCle("");
    } catch (e) {
      setMessage({ texte: messageErreur(e), erreur: true });
    } finally {
      setEnCours(false);
      onChange();
    }
  }

  return (
    <>
      <h2>Ta clé Gemini (facultatif)</h2>
      <p className="texte-doux">
        Les questions générées, les corrections, la méthode Feynman, les podcasts et les vidéos utilisent Gemini, l'IA
        de Google. Elle est gratuite avec une clé personnelle.
      </p>

      {etatIa?.clePresente && !message && (
        <Message genre="succes">Une clé est déjà enregistrée. Tu peux la remplacer ou continuer.</Message>
      )}

      <ol className="guide-consignes">
        <li>
          Ouvre Google AI Studio et connecte-toi avec ton compte Google.
          <button className="lien-discret" onClick={() => api.ouvrirLienExterne(ADRESSE_CLE)}>
            <ExternalLink size={14} aria-hidden /> aistudio.google.com/apikey
          </button>
        </li>
        <li>
          Clique sur <strong>Create API key</strong> (Créer une clé API), puis copie la clé.
        </li>
        <li>
          Colle-la ici :
          <span className="rangee guide-champ-cle">
            <span className="champ-icone">
              <KeyRound size={15} className="icone" aria-hidden />
              <input
                className="champ"
                type="password"
                autoComplete="off"
                spellCheck={false}
                placeholder="Ta clé AI Studio"
                aria-label="Clé API Gemini"
                value={cle}
                onChange={(e) => setCle(e.target.value)}
                onKeyDown={(e) => e.key === "Enter" && enregistrer()}
              />
            </span>
            <button className="bouton bouton-principal" disabled={!cle.trim() || enCours} onClick={enregistrer}>
              {enCours ? <LoaderCircle size={15} className="tourne" aria-hidden /> : <CircleCheck size={15} aria-hidden />}
              Vérifier et enregistrer
            </button>
          </span>
        </li>
      </ol>

      {message &&
        (message.erreur ? (
          <p className="message-erreur">{message.texte}</p>
        ) : (
          <Message genre="succes">{message.texte}</Message>
        ))}

      <p className="texte-doux petit">
        Ta clé est rangée dans le Gestionnaire d'identification de Windows, jamais dans un fichier. Avec le niveau
        gratuit, Google peut utiliser les extraits de cours envoyés pour améliorer ses produits ; ta clé et tes
        statistiques ne sont jamais envoyées.
      </p>
    </>
  );
}

function EtapePhotos({ reglages, onReglages }: { reglages: Reglages; onReglages: (r: Reglages) => void }) {
  const [erreur, setErreur] = useState<string | null>(null);

  async function basculer(autorisee: boolean) {
    setErreur(null);
    try {
      onReglages(await api.definirEcritureAutorisee(autorisee));
    } catch (e) {
      setErreur(messageErreur(e));
    }
  }

  return (
    <>
      <h2>Tes photos de cours (facultatif)</h2>
      <p className="texte-doux">
        Prends ton cours en photo : Révisio le lit avec Gemini et le range dans ton vault sous forme de petites notes,
        rédigées comme les tiennes. Pour cela, il doit avoir le droit d'écrire dans ton vault.
      </p>
      <ul className="guide-promesses">
        <li>
          <Lock size={18} className="icone" aria-hidden />
          <span>
            <strong>Chaque écriture est sauvegardée</strong> et s'annule en un clic depuis le journal.
          </span>
        </li>
        <li>
          <CircleCheck size={18} className="icone" aria-hidden />
          <span>
            <strong>Une note existante n'est jamais modifiée sans ton accord :</strong> tu vois les changements proposés
            et tu acceptes ou refuses.
          </span>
        </li>
      </ul>
      <label className="rangee case-a-cocher">
        <input type="checkbox" checked={reglages.ecritureAutorisee} onChange={(e) => basculer(e.target.checked)} />
        Autoriser Révisio à écrire dans mon vault
      </label>
      {erreur && <p className="message-erreur">{erreur}</p>}
      {reglages.ecritureAutorisee && (
        <DossierSurveille reglages={reglages} surveillance={null} onReglages={onReglages} onChange={() => {}} />
      )}
    </>
  );
}

function Recapitulatif({ reglages, etatIa }: { reglages: Reglages; etatIa: EtatIa | null }) {
  return (
    <>
      <h2>C'est prêt</h2>
      <ul className="guide-recap">
        <li>
          <CircleCheck size={18} className="icone verdict-juste" aria-hidden />
          <span>
            <strong>Vault</strong>
            <code>{reglages.cheminVault}</code>
          </span>
        </li>
        <li>
          {etatIa?.clePresente ? (
            <CircleCheck size={18} className="icone verdict-juste" aria-hidden />
          ) : (
            <KeyRound size={18} className="icone" aria-hidden />
          )}
          <span>
            <strong>Gemini</strong>
            {etatIa?.clePresente
              ? "Clé enregistrée dans Windows."
              : "Pas de clé pour l'instant. Tu pourras l'ajouter dans Réglages, Intelligence artificielle."}
          </span>
        </li>
        <li>
          {reglages.ecritureAutorisee ? (
            <CircleCheck size={18} className="icone verdict-juste" aria-hidden />
          ) : (
            <Lock size={18} className="icone" aria-hidden />
          )}
          <span>
            <strong>Photos de cours</strong>
            {reglages.ecritureAutorisee
              ? reglages.dossierPhotos
                ? `Écriture autorisée, dossier surveillé : ${reglages.dossierPhotos}`
                : "Écriture autorisée. Importe tes photos depuis l'écran Photos de cours."
              : "Vault en lecture seule. Tu pourras autoriser l'écriture dans Réglages."}
          </span>
        </li>
      </ul>
      <p className="texte-doux petit">
        Sans Internet, Révisio peut aussi utiliser une IA installée sur ton ordinateur (Ollama) : voir Réglages, IA
        locale. Tu peux revoir ce guide à tout moment depuis les Réglages.
      </p>
    </>
  );
}
