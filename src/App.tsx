import { useCallback, useEffect, useState } from "react";
import {
  BookOpen,
  CalendarDays,
  Camera,
  Cpu,
  Clapperboard,
  ClipboardList,
  Headphones,
  Layers,
  LayoutDashboard,
  LoaderCircle,
  type LucideIcon,
  Network,
  NotebookPen,
  Search,
  Settings,
  Sparkles,
  Globe,
} from "lucide-react";
import { api, ChapitreChoisi, InfosApp, LotPhotos, messageErreur, Reglages } from "./lib/api";
import { definirModeLocalCourant, useEnLigne } from "./lib/connexion";
import { useIndexation } from "./lib/indexation";
import { listen } from "@tauri-apps/api/event";
import { definirEcranActif, signalerTacheTerminee } from "./lib/taches";
import { appliquerTheme } from "./lib/theme";
import GardeFou from "./GardeFou";
import { IndicateurTaches, NotificationsTaches } from "./composants/TachesIa";
import AccueilVue from "./views/AccueilVue";
import AtelierIaVue from "./views/AtelierIaVue";
import CarnetVue from "./views/CarnetVue";
import CartesMentalesVue from "./views/CartesMentalesVue";
import EspaceWebVue from "./views/EspaceWebVue";
import ExercicesVue from "./views/ExercicesVue";
import GuideDemarrage from "./views/GuideDemarrage";
import NotesVue from "./views/NotesVue";
import PlanningVue from "./views/PlanningVue";
import PhotosVue from "./views/PhotosVue";
import PodcastsVue from "./views/PodcastsVue";
import RapportVue from "./views/RapportVue";
import RechercheVue from "./views/RechercheVue";
import VideosVue from "./views/VideosVue";
import ReviserVue, { DemandeSeance } from "./views/ReviserVue";
import ReglagesVue from "./views/ReglagesVue";
import { TOUCHE_COMMANDE, toucheCommande } from "./lib/plateforme";
import { definirEstObsidian } from "./lib/dossier";

type Vue =
  | "accueil"
  | "reviser"
  | "planning"
  | "carnet"
  | "atelier"
  | "exercices"
  | "cartes"
  | "podcasts"
  | "videos"
  | "photos"
  | "web"
  | "notes"
  | "recherche"
  | "reglages"
  | "rapport";

interface LienNav {
  vue: Vue;
  libelle: string;
  icone: LucideIcon;
}

/** Menu de gauche, par groupes (la recherche est dans la barre du haut : Ctrl K, ou ⌘ K sur Mac). */
const NAVIGATION: { groupe: string; liens: LienNav[] }[] = [
  {
    groupe: "Réviser",
    liens: [
      { vue: "accueil", libelle: "Tableau de bord", icone: LayoutDashboard },
      { vue: "reviser", libelle: "Réviser", icone: Layers },
      { vue: "planning", libelle: "Planning", icone: CalendarDays },
      { vue: "carnet", libelle: "Carnet d'erreurs", icone: NotebookPen },
    ],
  },
  {
    groupe: "Créer avec l'IA",
    liens: [
      { vue: "atelier", libelle: "Atelier IA", icone: Sparkles },
      { vue: "exercices", libelle: "Exercices type bac", icone: ClipboardList },
      { vue: "cartes", libelle: "Cartes mentales", icone: Network },
      { vue: "podcasts", libelle: "Podcasts", icone: Headphones },
      { vue: "videos", libelle: "Vidéos", icone: Clapperboard },
      { vue: "photos", libelle: "Photos de cours", icone: Camera },
    ],
  },
  {
    groupe: "Ressources",
    liens: [
      { vue: "notes", libelle: "Notes", icone: BookOpen },
      { vue: "web", libelle: "Espace web", icone: Globe },
    ],
  },
];

function BoutonNav({ lien, actif, onClick }: { lien: LienNav; actif: boolean; onClick: () => void }) {
  const Icone = lien.icone;
  return (
    <button className={"lien-nav" + (actif ? " actif" : "")} aria-current={actif ? "page" : undefined} onClick={onClick}>
      <Icone className="icone" size={18} strokeWidth={1.8} aria-hidden />
      {lien.libelle}
    </button>
  );
}

interface Position {
  chemin: string;
  ancre: string | null;
}

export default function App() {
  const [vue, setVue] = useState<Vue>("accueil");
  const [reglages, setReglages] = useState<Reglages | null>(null);
  const [infos, setInfos] = useState<InfosApp | null>(null);
  const [erreur, setErreur] = useState<string | null>(null);
  const [historique, setHistorique] = useState<Position[]>([]);
  const [texteRecherche, setTexteRecherche] = useState("");
  const [demandeSeance, setDemandeSeance] = useState<DemandeSeance | null>(null);
  const [demandeExamen, setDemandeExamen] = useState<{ chapitres: ChapitreChoisi[] } | null>(null);
  const [demandeCarte, setDemandeCarte] = useState<ChapitreChoisi | null>(null);
  const demander = (d: DemandeSeance) => {
    setDemandeSeance(d);
    setVue("reviser");
  };
  const indexation = useIndexation();
  const enLigne = useEnLigne();

  const position = historique.at(-1) ?? null;

  const ouvrirNote = useCallback((chemin: string, ancre: string | null = null) => {
    setHistorique((h) => (h.at(-1)?.chemin === chemin && h.at(-1)?.ancre === ancre ? h : [...h, { chemin, ancre }]));
    setVue("notes");
  }, []);

  const revenir = useCallback(() => setHistorique((h) => (h.length > 1 ? h.slice(0, -1) : h)), []);

  useEffect(() => {
    Promise.all([api.lireReglages(), api.infosApp()])
      .then(([r, i]) => {
        setReglages(r);
        setInfos(i);
      })
      .catch((e) => setErreur(messageErreur(e)));
  }, []);

  // Bouton « Ouvrir dans Obsidian » seulement si le dossier de notes est un vault Obsidian.
  useEffect(() => {
    if (reglages?.cheminVault) api.estVaultObsidian().then(definirEstObsidian).catch(() => definirEstObsidian(false));
  }, [reglages?.cheminVault]);

  // Photos du dossier surveillé transformées en notes par le cœur Rust : notification.
  useEffect(() => {
    const arret = listen<LotPhotos>("photos-lot", ({ payload: l }) =>
      signalerTacheTerminee({
        genre: "photos-dossier",
        titre: `Photos · ${l.photos.length} photo${l.photos.length > 1 ? "s" : ""} du dossier`,
        ecran: "photos",
        resume: l.message ?? "Photos traitées",
        erreur: l.statut === "erreur",
      }),
    );
    return () => {
      arret.then((f) => f());
    };
  }, []);

  // Les générations par l'IA ne notifient pas la fin si l'élève regarde déjà l'écran concerné.
  useEffect(() => definirEcranActif(vue), [vue]);

  useEffect(() => {
    if (reglages) appliquerTheme(reglages.theme, reglages.accent);
  }, [reglages?.theme, reglages?.accent]);

  // Interrupteur « En ligne » : tous les écrans savent si le mode local est actif.
  useEffect(() => {
    definirModeLocalCourant(!!reglages?.modeLocal);
  }, [reglages?.modeLocal]);

  const basculerModeLocal = () =>
    reglages &&
    api
      .definirModeLocal(!reglages.modeLocal)
      .then(setReglages)
      .catch((e) => setErreur(messageErreur(e)));

  useEffect(() => {
    const touche = (e: KeyboardEvent) => {
      if (e.altKey && e.key === "ArrowLeft") {
        e.preventDefault();
        revenir();
      } else if (toucheCommande(e) && e.key.toLowerCase() === "k") {
        e.preventDefault();
        setVue("recherche");
      }
    };
    window.addEventListener("keydown", touche);
    return () => window.removeEventListener("keydown", touche);
  }, [revenir]);

  if (erreur)
    return (
      <div className="page">
        <p className="message-erreur">{erreur}</p>
      </div>
    );
  if (!reglages || !infos)
    return (
      <div className="chargement-app">
        <LoaderCircle className="icone tourne" size={22} aria-hidden /> Chargement…
      </div>
    );

  if (!reglages.cheminVault || !reglages.accueilTermine) {
    return (
      <GuideDemarrage
        reglages={reglages}
        infos={infos}
        onReglages={setReglages}
        onTermine={(r) => {
          setReglages(r);
          setVue("accueil");
        }}
      />
    );
  }

  return (
    <div className="app">
      <nav className="barre-laterale" aria-label="Menu principal">
        <div className="logo">
          <img className="logo-image" src="/logo.png" alt="" />
          Révisio
        </div>
        {NAVIGATION.map((g) => (
          <div key={g.groupe}>
            <div className="nav-groupe">{g.groupe}</div>
            {g.liens.map((l) => (
              <BoutonNav key={l.vue} lien={l} actif={vue === l.vue} onClick={() => setVue(l.vue)} />
            ))}
          </div>
        ))}
        <div className="pied-barre">
          <BoutonNav
            lien={{ vue: "reglages", libelle: "Réglages", icone: Settings }}
            actif={vue === "reglages"}
            onClick={() => setVue("reglages")}
          />
          <div className="version">Version {infos.version}</div>
        </div>
      </nav>

      <div className="zone-principale">
        <header className="barre-haut">
          <button className="recherche-rapide" onClick={() => setVue("recherche")}>
            <Search className="icone" size={16} aria-hidden />
            <span>Rechercher dans tes notes…</span>
            <kbd>{TOUCHE_COMMANDE} K</kbd>
          </button>
          <div className="etats-haut">
            <IndicateurTaches onOuvrir={setVue} />
            {indexation?.enCours && (
              <span className="indicateur" title="Révisio relit les notes du vault.">
                <LoaderCircle className="icone tourne" size={14} aria-hidden /> Indexation
              </span>
            )}
            <button
              role="switch"
              aria-checked={!reglages.modeLocal}
              className={"interrupteur-connexion" + (reglages.modeLocal ? " local" : enLigne ? "" : " hors-ligne")}
              onClick={basculerModeLocal}
              title={
                reglages.modeLocal
                  ? "Mode local : Gemini est coupé, l'IA de ton PC répond. Clique pour repasser en ligne."
                  : enLigne
                    ? "En ligne : Gemini est utilisé. Clique pour passer en mode local (rien n'est envoyé à Google)."
                    : "Pas d'Internet : Gemini est indisponible. Clique pour passer en mode local."
              }
            >
              <span className="piste" aria-hidden>
                <span className="pouce" />
              </span>
              {reglages.modeLocal ? (
                <>
                  <Cpu size={14} aria-hidden /> Local
                </>
              ) : enLigne ? (
                "En ligne"
              ) : (
                "Hors ligne"
              )}
            </button>
          </div>
        </header>

      <main className={"contenu" + (vue === "notes" || vue === "web" ? " contenu-plein" : "")}>
        <div key={vue} className={"vue-entree" + (vue === "web" ? " sans-animation" : "")}>
        <GardeFou cle={vue}>
        {vue === "reglages" ? (
          <ReglagesVue reglages={reglages} infos={infos} onChange={setReglages} />
        ) : vue === "accueil" ? (
          <AccueilVue
            indexation={indexation}
            onOuvrirNote={ouvrirNote}
            onSeanceDuJour={(groupe) => demander({ groupe })}
            onPlanning={() => setVue("planning")}
            onRapport={() => setVue("rapport")}
            onExercices={() => setVue("exercices")}
          />
        ) : vue === "reviser" ? (
          <ReviserVue reglages={reglages} indexation={indexation} demandeSeanceDuJour={demandeSeance} onReglages={setReglages} />
        ) : vue === "atelier" ? (
          <AtelierIaVue
            onReglages={() => setVue("reglages")}
            onReviserQuestions={(chapitre) => demander({ chapitre, genres: ["qcm", "ouverte"] })}
            onOuvrirCartes={(c) => {
              setDemandeCarte({ ...c });
              setVue("cartes");
            }}
          />
        ) : vue === "planning" ? (
          <PlanningVue
            onReviserChapitre={(chapitre) => demander({ chapitre })}
            onExamen={(chapitres) => {
              setDemandeExamen({ chapitres });
              setVue("exercices");
            }}
          />
        ) : vue === "exercices" ? (
          <ExercicesVue
            onReglages={() => setVue("reglages")}
            onOuvrirNote={(chemin) => ouvrirNote(chemin)}
            demandeExamen={demandeExamen}
          />
        ) : vue === "cartes" ? (
          <CartesMentalesVue
            onOuvrirNote={(chemin) => ouvrirNote(chemin)}
            onReglages={() => setVue("reglages")}
            demande={demandeCarte}
          />
        ) : vue === "rapport" ? (
          <RapportVue onRetour={() => setVue("accueil")} />
        ) : vue === "web" ? (
          <EspaceWebVue />
        ) : vue === "podcasts" ? (
          <PodcastsVue onOuvrirNote={(chemin) => ouvrirNote(chemin)} onReglages={() => setVue("reglages")} />
        ) : vue === "videos" ? (
          <VideosVue onOuvrirNote={(chemin) => ouvrirNote(chemin)} onReglages={() => setVue("reglages")} />
        ) : vue === "photos" ? (
          <PhotosVue
            reglages={reglages}
            onReglages={setReglages}
            onOuvrirNote={(chemin) => ouvrirNote(chemin)}
            onAllerAuxReglages={() => setVue("reglages")}
          />
        ) : vue === "carnet" ? (
          <CarnetVue reglages={reglages} onOuvrirNote={ouvrirNote} />
        ) : vue === "notes" ? (
          <NotesVue
            chemin={position?.chemin ?? null}
            ancre={position?.ancre ?? null}
            peutRevenir={historique.length > 1}
            onOuvrir={ouvrirNote}
            onRetour={revenir}
            indexation={indexation}
            ecritureAutorisee={reglages.ecritureAutorisee}
          />
        ) : (
          <RechercheVue onOuvrir={ouvrirNote} texte={texteRecherche} onTexte={setTexteRecherche} />
        )}
        </GardeFou>
        </div>
      </main>
      </div>
      <NotificationsTaches onOuvrir={setVue} />
    </div>
  );
}
