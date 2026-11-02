// Révisio sur téléphone : coque de la web-app (premier lancement, onglets en bas de l'écran).
// Les écrans de séance, de carnet et de lecture des notes sont ceux de l'app de bureau.
import { useEffect, useState } from "react";
import { BookOpen, CloudOff, Layers, NotebookPen, RefreshCw, Smartphone } from "lucide-react";
import ReviserVue from "../views/ReviserVue";
import CarnetVue from "../views/CarnetVue";
import { appliquerTheme } from "../lib/theme";
import { connecter } from "./drive";
import { reglages, synchroniser, useMagasin } from "./magasin";
import NotesMobile from "./NotesMobile";
import CompteMobile, { ResumeSynchro } from "./CompteMobile";

type Onglet = "reviser" | "notes" | "erreurs" | "compte";

const ONGLETS: { id: Onglet; libelle: string; icone: typeof Layers }[] = [
  { id: "reviser", libelle: "Réviser", icone: Layers },
  { id: "notes", libelle: "Notes", icone: BookOpen },
  { id: "erreurs", libelle: "Erreurs", icone: NotebookPen },
  { id: "compte", libelle: "Synchro", icone: RefreshCw },
];

export default function AppMobile() {
  const magasin = useMagasin();
  const [onglet, setOnglet] = useState<Onglet>("reviser");
  const [noteOuverte, setNoteOuverte] = useState<string | null>(null);
  // Les écrans réutilisés reçoivent les réglages au format de l'app de bureau.
  const [, setVersion] = useState(0);

  useEffect(() => appliquerTheme("systeme", "monochrome"), []);

  if (!magasin.pret) return <div className="mobile-chargement progression-texte">Chargement…</div>;

  if (!magasin.modele) {
    return (
      <main className="mobile-accueil">
        <img src="logo.png" alt="" width="72" height="72" />
        <h1>Révisio</h1>
        <p>
          Révise tes cartes et lis tes notes sur ton téléphone, même sans Internet. Tout passe par ton propre Google Drive,
          dans un dossier caché réservé à Révisio.
        </p>
        <ol>
          <li>Sur ton ordinateur, ouvre Révisio, puis Réglages, et connecte ton Google Drive.</li>
          <li>Ici, connecte le même compte Google.</li>
        </ol>
        {magasin.connecte ? (
          <button className="bouton bouton-principal" disabled={magasin.synchro.enCours} onClick={() => synchroniser()}>
            <RefreshCw size={16} aria-hidden /> {magasin.synchro.enCours ? "Réception de tes cartes…" : "Recevoir mes cartes et mes notes"}
          </button>
        ) : (
          <button className="bouton bouton-principal" onClick={() => connecter()}>
            <Smartphone size={16} aria-hidden /> Connecter mon Google Drive
          </button>
        )}
        {magasin.synchro.erreur && <p className="message-erreur">{magasin.synchro.erreur}</p>}
        <p className="texte-doux petit">
          Astuce : ajoute Révisio à ton écran d'accueil (Safari : bouton Partager, puis « Sur l'écran d'accueil ») pour
          l'ouvrir comme une app.
        </p>
      </main>
    );
  }

  const r = reglages();
  return (
    <div className="mobile">
      <header className="mobile-barre">
        <span className="marque">
          <img src="logo.png" alt="" width="24" height="24" /> Révisio
        </span>
        <ResumeSynchro />
        {!navigator.onLine && <CloudOff size={16} aria-label="Hors ligne" />}
      </header>
      <main className="mobile-contenu">
        {onglet === "reviser" ? (
          <ReviserVue reglages={r} indexation={null} onReglages={() => setVersion((v) => v + 1)} />
        ) : onglet === "notes" ? (
          <NotesMobile chemin={noteOuverte} onOuvrir={setNoteOuverte} ecritureAutorisee={r.ecritureAutorisee} />
        ) : onglet === "erreurs" ? (
          <CarnetVue
            reglages={r}
            onOuvrirNote={(chemin) => {
              setNoteOuverte(chemin);
              setOnglet("notes");
            }}
          />
        ) : (
          <CompteMobile />
        )}
      </main>
      <nav className="mobile-onglets" aria-label="Sections">
        {ONGLETS.map((o) => (
          <button
            key={o.id}
            className={onglet === o.id ? "actif" : ""}
            aria-current={onglet === o.id ? "page" : undefined}
            onClick={() => {
              if (o.id === "notes" && onglet === "notes") setNoteOuverte(null);
              setOnglet(o.id);
            }}
          >
            <o.icone size={20} aria-hidden />
            <span>{o.libelle}</span>
          </button>
        ))}
      </nav>
    </div>
  );
}
