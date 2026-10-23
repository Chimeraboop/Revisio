// Appels vers le cœur Rust. Chaque fonction correspond à une commande de `src-tauri/src/commandes.rs`.
import { invoke } from "@tauri-apps/api/core";
import type { Accent, Theme } from "./theme";

export interface InfosApp {
  version: string;
  dossierDonnees: string;
  /** Vaults qu'Obsidian connaît sur ce PC, du plus récemment ouvert au plus ancien. */
  vaultsDetectes: VaultDetecte[];
}

export interface VaultDetecte {
  chemin: string;
  nom: string;
}

export type ModeReponse = "retourner" | "taper";

export interface Reglages {
  cheminVault: string | null;
  nomVaultObsidian: string;
  /** Guide de démarrage terminé (toujours renseigné par le cœur Rust à la lecture). */
  accueilTermine: boolean | null;
  theme: Theme;
  accent: Accent;
  nouvellesParJour: number;
  entrelacement: boolean;
  vocabVersFrancais: ModeReponse;
  vocabDepuisFrancais: ModeReponse;
  dureeSeance: number;
  nouvellesParJourLangues: number;
  dureeSeanceLangues: number;
  modeleIa: string;
  modeleIaRapide: string;
  iaParMinute: number;
  iaParJour: number;
  modeleTts: string;
  voixA: string;
  voixB: string;
  voixVideo: string;
  iaLocale: ModeIaLocale;
  modeleLocal: string;
  adresseOllama: string;
  /** Interrupteur « En ligne » coupé : Gemini désactivé, l'IA locale répond. */
  modeLocal: boolean;
  /** Matières laissées de côté dans les séances du jour (« Hors matière » : notes sans matière). */
  matieresExclues: string[];
  /** L'élève autorise Révisio à écrire dans le vault (désactivé par défaut). */
  ecritureAutorisee: boolean;
  /** Dossier surveillé où arrivent les photos de cours. */
  dossierPhotos: string | null;
}

/** Un lot de photos de cours (un cours) et ce qu'il a donné. */
export interface LotPhotos {
  id: number;
  date: number;
  source: "import" | "dossier";
  photos: string[];
  statut: "attente" | "en_cours" | "termine" | "illisible" | "erreur";
  matiere: string | null;
  chapitre: string | null;
  dossier: string | null;
  message: string | null;
}

/** Modification d'une note existante proposée par l'IA (en attente de l'accord de l'élève). */
export interface PropositionVault {
  id: number;
  date: number;
  lotId: number | null;
  chemin: string;
  avant: string;
  apres: string;
  raison: string;
}

export interface EtatSurveillancePhotos {
  dossier: string | null;
  enAttente: number;
  derniereVerification: number | null;
  message: string | null;
  enCours: boolean;
}

/** Une écriture de Révisio dans le vault (annulable). */
export interface EntreeHistorique {
  id: number;
  date: number;
  chemin: string;
  genre: "creation" | "modification";
  origine: "manuel" | "photos";
  lotId: number | null;
  raison: string | null;
  annuleeLe: number | null;
}

/** IA locale (Ollama) pour les tâches simples. */
export type ModeIaLocale = "jamais" | "secours" | "toujours";

export interface ModeleLocal {
  nom: string;
  /** Octets sur le disque. */
  taille: number;
  parametres: string;
  quantification: string;
  famille: string;
}

export interface EtatIaLocale {
  adresse: string;
  version: string | null;
  modeles: ModeleLocal[];
  erreur: string | null;
  telechargement: string | null;
}

export interface AvancementTelechargement {
  modele: string;
  statut: string;
  total: number;
  fait: number;
}

export type ReglagesIa = Pick<
  Reglages,
  "modeleIa" | "modeleIaRapide" | "iaParMinute" | "iaParJour" | "modeleTts" | "voixA" | "voixB" | "voixVideo"
>;

export interface EtatIa {
  clePresente: boolean;
  appelsAujourdhui: number;
  jetonsAujourdhui: number;
}

export interface ModeleIa {
  id: string;
  nom: string;
  description: string;
}

export interface ChapitreIa {
  matiere: string;
  chapitre: string | null;
  notes: number;
  questionsIa: number;
  /** Questions de l'IA à revoir maintenant. */
  questionsDues: number;
  cartesMentales: number;
}

export interface EstimationIa {
  appels: number;
  extraits: number;
  caracteres: number;
}

export interface CorrectionIa {
  note: number;
  verdict: "juste" | "partiel" | "faux";
  points_justes: string[];
  manques: string[];
  erreurs: string[];
  conseil: string;
  /** Modèle qui a corrigé (« local:… » pour l'IA locale). */
  modele: string;
}

export interface PointSource {
  texte: string;
  source: string;
  citation: string;
  verifie: boolean;
}

export interface RetourFeynman {
  note: number;
  ce_qui_est_juste: string[];
  ce_qui_manque: PointSource[];
  erreurs: PointSource[];
  conseil: string;
}

export interface ExtraitIa {
  id: string;
  noteChemin: string;
  titreNote: string;
  section: string;
}

export interface ExtraitComplet extends ExtraitIa {
  texte: string;
  markdown: string;
}

// ---------- Exercices type bac et mode examen ----------

export interface TypeExercice {
  id: string;
  nom: string;
  description: string;
  dureeMinutes: number;
}

export interface ChapitreChoisi {
  matiere: string;
  chapitre: string | null;
}

export type TypeReponse = "texte" | "court" | "choix" | "nombre";

export interface QuestionSujet {
  enonce: string;
  type_reponse: TypeReponse;
  choix: string[];
  reponse_attendue: string;
  expression: string;
  points: number;
  elements_attendus: string[];
  partie: string;
  source: string;
  /** Valeur recalculée par Révisio (questions « nombre »). */
  valeur: number | null;
  /** Valeur annoncée par l'IA quand elle différait du calcul. */
  valeur_ia: string | null;
}

export interface SerieGraphique {
  nom: string;
  points: [number, number][];
}

export interface GraphiqueSujet {
  titre: string;
  axe_x: string;
  axe_y: string;
  series: SerieGraphique[];
}

export interface Sujet {
  titre: string;
  consigne: string;
  contexte: string;
  variables: { nom: string; valeur: number }[];
  graphique: GraphiqueSujet | null;
  questions: QuestionSujet[];
}

export type VerdictQuestion = "juste" | "partiel" | "faux" | "a_corriger";

export interface ResultatQuestion {
  numero: number;
  points: number;
  points_max: number;
  verdict: VerdictQuestion;
  retiree: boolean;
  commentaire: string;
  presents: string[];
  manquants: string[];
  correcteur: "revisio" | "ia";
}

export interface Copie {
  id: number;
  sujetId: number;
  date: number;
  dureeMs: number;
  reponses: string[];
  resultats: ResultatQuestion[];
  appreciation: string;
  note: number | null;
}

export interface SujetEnregistre {
  id: number;
  date: number;
  typeId: string;
  typeNom: string;
  matiere: string;
  chapitres: ChapitreChoisi[];
  modele: string;
  dureeMinutes: number;
  sujet: Sujet;
  extraits: ExtraitComplet[];
  copies: Copie[];
}

export interface NoteCopie {
  date: number;
  sujetId: number;
  titre: string;
  typeNom: string;
  matiere: string;
  note: number;
}

export interface ResumeSujet {
  id: number;
  date: number;
  typeId: string;
  typeNom: string;
  matiere: string;
  chapitres: ChapitreChoisi[];
  dureeMinutes: number;
  titre: string;
  questions: number;
  copies: number;
  derniereNote: number | null;
}

export interface ResultatFeynman {
  id: number;
  retour: RetourFeynman;
  extraits: ExtraitIa[];
  modele: string;
}

export interface EssaiFeynman {
  id: number;
  date: number;
  matiere: string;
  chapitre: string | null;
  notion: string;
  explication: string;
  note: number;
  retour: RetourFeynman;
}

export interface ResultatGeneration {
  creees: number;
  rejetees: number;
  raisons: string[];
  appels: number;
  modele: string;
}

export type ReglagesRevision = Pick<
  Reglages,
  | "nouvellesParJour"
  | "entrelacement"
  | "vocabVersFrancais"
  | "vocabDepuisFrancais"
  | "dureeSeance"
  | "nouvellesParJourLangues"
  | "dureeSeanceLangues"
>;

/** Les deux séances du jour : « Langues » (tout le vocabulaire) et « Autres matières ». */
export type GroupeRevision = "langues" | "autres";

export const NOMS_GROUPES: Record<GroupeRevision, string> = {
  langues: "Langues",
  autres: "Autres matières",
};

/** « examen » : erreur faite dans un exercice ou un examen (sans carte de révision). */
export type GenreCarte = "vocabulaire" | "trou" | "schema" | "ordre" | "qcm" | "ouverte" | "examen";

export interface CarteSeance {
  id: number;
  genre: GenreCarte;
  noteChemin: string;
  titreNote: string;
  matiere: string | null;
  chapitre: string | null;
  section: string;
  question: string;
  reponse: string;
  donnees: Record<string, unknown>;
  extrait: string;
  etatFsrs: string | null;
  echeance: number | null;
}

export interface Seance {
  /** Cartes des chapitres du planning d'aujourd'hui (révision anticipée). */
  planifiees: CarteSeance[];
  dues: CarteSeance[];
  faibles: CarteSeance[];
  nouvelles: CarteSeance[];
}

export interface CarteStat {
  matiere: string | null;
  chapitre: string | null;
  genre: GenreCarte;
  etatFsrs: string | null;
}

export interface JourActivite {
  jour: string;
  revisions: number;
  justes: number;
  dureeMs: number;
}

export interface NotionFaible {
  carteId: number;
  genre: GenreCarte;
  question: string;
  reponse: string;
  matiere: string | null;
  chapitre: string | null;
  noteChemin: string;
  section: string;
  erreurs: number;
  oublis: number;
}

export interface TableauDeBord {
  cartes: CarteStat[];
  jours: JourActivite[];
  faibles: NotionFaible[];
}

export interface FiltreSeance {
  matiere?: string | null;
  chapitre?: string | null;
  genres?: GenreCarte[];
  debutJour: number;
  limiteDues?: number;
  groupe?: GroupeRevision | null;
  /** Chapitres du planning d'aujourd'hui : leurs cartes passent en premier (même pas encore dues). */
  planifies?: ChapitreSeance[];
  /** Séance ciblée : seulement ces chapitres (vide = pas de restriction). */
  chapitres?: ChapitreSeance[];
}

/** Un chapitre pour une séance (`matiere` nulle : notes sans matière ; `chapitre` nul : notes générales). */
export interface ChapitreSeance {
  matiere: string | null;
  chapitre: string | null;
}

export interface ComptesChapitre extends ChapitreSeance {
  dues: number;
  nouvelles: number;
  total: number;
}

// ---------- Dates d'évaluation et planning ----------

export type GenreEvaluation = "controle" | "ds" | "oral" | "bac_blanc" | "autre";

export const NOMS_EVALUATIONS: Record<GenreEvaluation, string> = {
  controle: "Contrôle",
  ds: "Devoir surveillé",
  oral: "Oral",
  bac_blanc: "Bac blanc",
  autre: "Autre",
};

export interface Evaluation {
  /** 0 pour une nouvelle évaluation. */
  id: number;
  /** Jour local AAAA-MM-JJ. */
  jour: string;
  matiere: string;
  titre: string;
  genre: GenreEvaluation;
  /** `null` = notes générales de la matière. */
  chapitres: (string | null)[];
}

// ---------- Espace web ----------

export interface Onglet {
  id: number;
  url: string;
  titre: string;
  actif: boolean;
}

/** Zone de la fenêtre où s'affichent les pages (pixels logiques). */
export interface ZoneWeb {
  x: number;
  y: number;
  largeur: number;
  hauteur: number;
}

export interface Favori {
  id: number;
  matiere: string | null;
  titre: string;
  url: string;
}

export interface SourceWeb {
  id: number;
  date: number;
  url: string;
  titre: string;
  texte: string;
  matiere: string;
  chapitre: string | null;
}

// ---------- Vision : questions sur les schémas ----------

export interface SchemaInfo {
  chemin: string;
  titre: string;
  matiere: string | null;
  chapitre: string | null;
  legendes: string[];
  noteLiee: string | null;
  questionsIa: number;
}

// ---------- Cartes mentales ----------

export interface Noeud {
  texte: string;
  /** Phrase d'explication (affichée au survol). */
  detail: string;
  /** Note source (chemin dans le vault). */
  note: string | null;
  enfants: Noeud[];
}

export interface CarteMentale {
  id: number;
  date: number;
  matiere: string;
  chapitre: string | null;
  modele: string;
  arbre: Noeud;
}

// ---------- Podcasts ----------

export interface OptionsPodcast {
  dureeMinutes: number;
  ton: "detendu" | "scolaire" | "dynamique";
  detail: "essentiel" | "approfondi";
  langue: "francais" | "allemand" | "anglais";
}

export interface Replique {
  voix: "A" | "B";
  nom: string;
  texte: string;
  style: string;
  source: string;
  morceau: number;
  debutMs: number | null;
}

export interface Podcast {
  id: number;
  date: number;
  matiere: string;
  chapitre: string | null;
  titre: string;
  options: OptionsPodcast;
  modeleScript: string;
  modeleTts: string;
  orateurs: { nom: string; voix: string }[];
  repliques: Replique[];
  extraits: ExtraitComplet[];
  morceaux: number;
  morceauxFaits: number;
  fichier: string | null;
  dureeMs: number | null;
  positionMs: number;
}

// ---------- Vidéos ----------

export interface OptionsVideo {
  diapos: number;
  langue: "francais" | "allemand" | "anglais";
}

export interface Diapo {
  titre: string;
  points: string[];
  /** Chemin du schéma Excalidraw affiché sur la diapositive. */
  schema: string | null;
  narration: string;
  source: string;
  /** Durée de la narration enregistrée. */
  dureeMs: number | null;
}

export interface Video {
  id: number;
  date: number;
  matiere: string;
  chapitre: string | null;
  titre: string;
  options: OptionsVideo;
  modeleScript: string;
  modeleTts: string;
  voix: string;
  diapos: Diapo[];
  extraits: ExtraitComplet[];
  /** Narrations déjà enregistrées. */
  faites: number;
  fichier: string | null;
  dureeMs: number | null;
}

export interface RevuesChapitre {
  matiere: string | null;
  chapitre: string | null;
  cartes: number;
}

export type Confiance = "sur" | "hesitant" | "hasard";

export interface NouvelleRevision {
  carteId: number;
  note: 1 | 2 | 3 | 4;
  confiance: Confiance;
  correcte: boolean;
  reponseDonnee: string | null;
  dureeMs: number;
  etatFsrs: string;
  echeance: number;
}

export interface ResumeGroupe {
  groupe: GroupeRevision;
  dues: number;
  nouvellesDisponibles: number;
  nouvellesRestantes: number;
}

export interface ResumeRevision {
  matieres: { matiere: string; dues: number; nouvelles: number; total: number }[];
  chapitres: ComptesChapitre[];
  groupes: ResumeGroupe[];
  parGenre: [GenreCarte, number][];
  dureeMoyenneMs: number;
}

export interface ErreurCarnet {
  id: number;
  carteId: number | null;
  date: number;
  genre: GenreCarte;
  matiere: string | null;
  chapitre: string | null;
  noteChemin: string;
  titreNote: string;
  section: string;
  question: string;
  reponseAttendue: string;
  reponseDonnee: string | null;
  extrait: string;
  comprise: boolean;
  fois: number;
}

/** Début de la journée locale, en millisecondes. */
export function debutDuJour(date = new Date()): number {
  return new Date(date.getFullYear(), date.getMonth(), date.getDate()).getTime();
}

export interface RapportIndexation {
  ajoutees: number;
  modifiees: number;
  supprimees: number;
  total: number;
  fichiersAnnexes: number;
  erreurs: string[];
  dureeMs: number;
}

export interface EtatIndexation {
  enCours: boolean;
  dernier: RapportIndexation | null;
  cartes: { actives: number; nouvelles: number; desactivees: number; parGenre: Record<string, number> } | null;
  erreur: string | null;
}

export interface StatsMatiere {
  matiere: string;
  notes: number;
  chapitres: number;
  fichesSynthese: number;
  schemas: number;
}

export interface Statistiques {
  totalNotes: number;
  matieres: StatsMatiere[];
}

export type GenreNote =
  | "cours"
  | "fiche_synthese"
  | "schema"
  | "vocabulaire"
  | "methodologie"
  | "reference"
  | "etude_lineaire"
  | "accueil"
  | "revision";

export interface NoteResume {
  chemin: string;
  titre: string;
  genre: GenreNote;
}

export interface Groupe {
  nom: string;
  notes: NoteResume[];
}

export interface Matiere {
  nom: string;
  notes: NoteResume[];
  chapitres: Groupe[];
}

export interface Arbre {
  matieres: Matiere[];
  autres: Groupe[];
}

export interface Note {
  chemin: string;
  nom: string;
  titre: string;
  matiere: string | null;
  chapitre: string | null;
  genre: GenreNote;
  tags: string[];
  frontmatter: Record<string, unknown> | null;
  legendes: string[];
  retroliens: NoteResume[];
  contenu: string;
  urlObsidian: string;
}

export interface Cible {
  chemin: string;
  genre: "note" | "fichier";
}

export interface ResultatRecherche {
  chemin: string;
  titre: string;
  matiere: string | null;
  chapitre: string | null;
  genre: GenreNote;
  section: string;
  extrait: string;
}

export const api = {
  infosApp: () => invoke<InfosApp>("infos_app"),
  lireReglages: () => invoke<Reglages>("lire_reglages"),
  definirVault: (chemin: string) => invoke<Reglages>("definir_vault", { chemin }),
  definirAccueilTermine: (termine: boolean) => invoke<Reglages>("definir_accueil_termine", { termine }),
  definirEcritureAutorisee: (autorisee: boolean) => invoke<Reglages>("definir_ecriture_autorisee", { autorisee }),
  enregistrerNote: (chemin: string, contenu: string, avant: string) =>
    invoke<number>("enregistrer_note", { chemin, contenu, avant }),
  historiqueVault: () => invoke<EntreeHistorique[]>("historique_vault"),
  annulerEcriture: (id: number) => invoke<string>("annuler_ecriture", { id }),
  importerPhotos: (chemins: string[], tache?: string) => invoke<LotPhotos>("importer_photos", { chemins, tache }),
  relancerLotPhotos: (id: number, tache?: string) => invoke<LotPhotos>("relancer_lot_photos", { id, tache }),
  lotsPhotos: () => invoke<LotPhotos[]>("lots_photos"),
  propositionsVault: () => invoke<PropositionVault[]>("propositions_vault"),
  accepterProposition: (id: number) => invoke<string>("accepter_proposition", { id }),
  refuserProposition: (id: number) => invoke<void>("refuser_proposition", { id }),
  definirDossierPhotos: (chemin: string | null, traiterExistantes: boolean) =>
    invoke<Reglages>("definir_dossier_photos", { chemin, traiterExistantes }),
  compterPhotosDossier: (chemin: string) => invoke<number>("compter_photos_dossier", { chemin }),
  etatPhotos: () => invoke<EtatSurveillancePhotos>("etat_photos"),
  dossiersDrive: () => invoke<string[]>("dossiers_drive"),
  definirTheme: (theme: Theme) => invoke<Reglages>("definir_theme", { theme }),
  definirAccent: (accent: Accent) => invoke<Reglages>("definir_accent", { accent }),
  definirModeLocal: (actif: boolean) => invoke<Reglages>("definir_mode_local", { actif }),
  etatIndexation: () => invoke<EtatIndexation>("etat_indexation"),
  reindexer: () => invoke<void>("reindexer"),
  statistiques: () => invoke<Statistiques>("statistiques"),
  arbre: () => invoke<Arbre>("arbre"),
  note: (chemin: string) => invoke<Note>("note", { chemin }),
  resoudreLien: (cible: string, source: string | null) => invoke<Cible | null>("resoudre_lien", { cible, source }),
  lireFichierVault: (chemin: string) => invoke<ArrayBuffer>("lire_fichier_vault", { chemin }),
  ouvrirDansObsidian: (chemin: string) => invoke<void>("ouvrir_dans_obsidian", { chemin }),
  ouvrirLienExterne: (url: string) => invoke<void>("ouvrir_lien_externe", { url }),
  rechercher: (texte: string, matiere: string | null) =>
    invoke<ResultatRecherche[]>("rechercher", { texte, matiere }),
  definirReglagesRevision: (valeurs: ReglagesRevision) => invoke<Reglages>("definir_reglages_revision", { valeurs }),
  resumeRevision: () => invoke<ResumeRevision>("resume_revision", { debutJour: debutDuJour() }),
  preparerSeance: (filtre: FiltreSeance) => invoke<Seance>("preparer_seance", { filtre }),
  definirMatieresExclues: (matieres: string[]) => invoke<Reglages>("definir_matieres_exclues", { matieres }),
  enregistrerRevision: (revision: NouvelleRevision) => invoke<void>("enregistrer_revision", { revision }),
  suspendreCarte: (carteId: number) => invoke<void>("suspendre_carte", { carteId }),
  carnetErreurs: (matiere: string | null, avecComprises: boolean) =>
    invoke<ErreurCarnet[]>("carnet_erreurs", { matiere, avecComprises }),
  marquerErreurComprise: (erreurId: number, comprise: boolean) =>
    invoke<void>("marquer_erreur_comprise", { erreurId, comprise }),
  cartesChoisies: (ids: number[]) => invoke<CarteSeance[]>("cartes_choisies", { ids }),
  tableauDeBord: () => invoke<TableauDeBord>("tableau_de_bord"),
  enregistrerEvaluation: (evaluation: Evaluation) => invoke<number>("enregistrer_evaluation", { evaluation }),
  evaluations: (depuis: string) => invoke<Evaluation[]>("evaluations", { depuis }),
  supprimerEvaluation: (id: number) => invoke<void>("supprimer_evaluation", { id }),
  revuesDuJour: () => invoke<RevuesChapitre[]>("revues_du_jour", { debutJour: debutDuJour() }),
  schemasIa: (matiere: string | null) => invoke<SchemaInfo[]>("schemas_ia", { matiere }),
  webOnglets: () => invoke<Onglet[]>("web_onglets"),
  webOuvrir: (adresse: string, zone: ZoneWeb) => invoke<number>("web_ouvrir", { adresse, zone }),
  webNaviguer: (id: number, adresse: string) => invoke<void>("web_naviguer", { id, adresse }),
  webHistorique: (id: number, sens: -1 | 0 | 1) => invoke<void>("web_historique", { id, sens }),
  webActiver: (id: number) => invoke<void>("web_activer", { id }),
  webFermer: (id: number) => invoke<void>("web_fermer", { id }),
  webZone: (zone: ZoneWeb) => invoke<void>("web_zone", { zone }),
  webMasquer: () => invoke<void>("web_masquer"),
  webAccueil: () => invoke<void>("web_accueil"),
  webSelection: (id: number) => invoke<string>("web_selection", { id }),
  favoris: () => invoke<Favori[]>("favoris"),
  ajouterFavori: (favori: Favori) => invoke<number>("ajouter_favori", { favori }),
  supprimerFavori: (id: number) => invoke<void>("supprimer_favori", { id }),
  sourcesWeb: (matiere: string | null) => invoke<SourceWeb[]>("sources_web", { matiere }),
  ajouterSourceWeb: (source: SourceWeb) => invoke<number>("ajouter_source_web", { source }),
  supprimerSourceWeb: (id: number) => invoke<void>("supprimer_source_web", { id }),
  genererQuestionsSchema: (chemin: string, image: string, nombre: number, tache?: string) =>
    invoke<ResultatGeneration>("generer_questions_schema", { chemin, image, nombre, tache }),
  planChapitre: (matiere: string, chapitre: string | null) => invoke<Noeud | null>("plan_chapitre", { matiere, chapitre }),
  cartesMentalesChapitre: (matiere: string, chapitre: string | null) =>
    invoke<CarteMentale[]>("cartes_mentales_chapitre", { matiere, chapitre }),
  genererCarteMentale: (matiere: string, chapitre: string | null, tache?: string) =>
    invoke<CarteMentale>("generer_carte_mentale", { matiere, chapitre, tache }),
  supprimerCarteMentale: (id: number) => invoke<void>("supprimer_carte_mentale", { id }),
  estimerPodcast: (dureeMinutes: number) =>
    invoke<{ appelsScript: number; appelsVoix: number }>("estimer_podcast", { dureeMinutes }),
  podcasts: () => invoke<Podcast[]>("podcasts"),
  ecrirePodcast: (matiere: string, chapitre: string | null, options: OptionsPodcast, tache?: string) =>
    invoke<Podcast>("ecrire_podcast", { matiere, chapitre, options, tache }),
  enregistrerMorceauPodcast: (id: number, tache?: string) => invoke<Podcast>("enregistrer_morceau_podcast", { id, tache }),
  assemblerPodcast: (id: number) => invoke<Podcast>("assembler_podcast", { id }),
  lireAudioPodcast: (id: number) => invoke<ArrayBuffer>("lire_audio_podcast", { id }),
  /** Les octets partent en corps brut (pas en JSON), l'identifiant dans un en-tête. */
  remplacerAudioPodcast: (id: number, octets: Uint8Array) =>
    invoke<Podcast>("remplacer_audio_podcast", octets, { headers: { "x-podcast": String(id) } }),
  positionPodcast: (id: number, positionMs: number) => invoke<void>("position_podcast", { id, positionMs }),
  supprimerPodcast: (id: number) => invoke<void>("supprimer_podcast", { id }),
  videos: () => invoke<Video[]>("videos"),
  ecrireVideo: (matiere: string, chapitre: string | null, options: OptionsVideo, tache?: string) =>
    invoke<Video>("ecrire_video", { matiere, chapitre, options, tache }),
  enregistrerNarrationVideo: (id: number, tache?: string) => invoke<Video>("enregistrer_narration_video", { id, tache }),
  narrationVideo: (id: number) => invoke<ArrayBuffer>("narration_video", { id }),
  /** La vidéo part en corps brut ; identifiant et durée dans les en-têtes. */
  enregistrerFichierVideo: (id: number, octets: Uint8Array, dureeMs: number) =>
    invoke<Video>("enregistrer_fichier_video", octets, {
      headers: { "x-video": String(id), "x-duree": String(Math.round(dureeMs)) },
    }),
  lireFichierVideo: (id: number) => invoke<ArrayBuffer>("lire_fichier_video", { id }),
  cheminVideo: (id: number) => invoke<string>("chemin_video", { id }),
  supprimerVideo: (id: number) => invoke<void>("supprimer_video", { id }),
  marquerCartesApprises: () => invoke<number>("marquer_cartes_apprises"),
  etatIa: () => invoke<EtatIa>("etat_ia"),
  etatIaLocale: () => invoke<EtatIaLocale>("etat_ia_locale"),
  definirReglagesLocaux: (iaLocale: ModeIaLocale, modeleLocal: string, adresseOllama: string) =>
    invoke<Reglages>("definir_reglages_locaux", { iaLocale, modeleLocal, adresseOllama }),
  testerIaLocale: () => invoke<string>("tester_ia_locale"),
  telechargerModeleLocal: (modele: string) => invoke<void>("telecharger_modele_local", { modele }),
  annulerTelechargementLocal: () => invoke<void>("annuler_telechargement_local"),
  supprimerModeleLocal: (modele: string) => invoke<void>("supprimer_modele_local", { modele }),
  enregistrerCleIa: (cle: string) => invoke<string>("enregistrer_cle_ia", { cle }),
  supprimerCleIa: () => invoke<void>("supprimer_cle_ia"),
  listerModelesIa: () => invoke<ModeleIa[]>("lister_modeles_ia"),
  definirReglagesIa: (valeurs: ReglagesIa) => invoke<Reglages>("definir_reglages_ia", { valeurs }),
  testerIa: () => invoke<string>("tester_ia"),
  chapitresIa: () => invoke<ChapitreIa[]>("chapitres_ia"),
  estimerQuestions: (matiere: string, chapitre: string | null) =>
    invoke<EstimationIa>("estimer_questions", { matiere, chapitre }),
  genererQuestions: (matiere: string, chapitre: string | null, nombre: number, avecSourcesWeb = false, tache?: string) =>
    invoke<ResultatGeneration>("generer_questions", { matiere, chapitre, nombre, avecSourcesWeb, tache }),
  corrigerReponse: (carteId: number, reponse: string, tache?: string) =>
    invoke<CorrectionIa>("corriger_reponse", { carteId, reponse, tache }),
  notionsFeynman: (matiere: string, chapitre: string | null) =>
    invoke<string[]>("notions_feynman", { matiere, chapitre }),
  evaluerFeynman: (matiere: string, chapitre: string | null, notion: string, explication: string, tache?: string) =>
    invoke<ResultatFeynman>("evaluer_feynman", { matiere, chapitre, notion, explication, tache }),
  historiqueFeynman: () => invoke<EssaiFeynman[]>("historique_feynman"),
  typesExercices: (matiere: string) => invoke<TypeExercice[]>("types_exercices", { matiere }),
  genererSujet: (typeId: string, chapitres: ChapitreChoisi[], dureeMinutes: number | null, tache?: string) =>
    invoke<SujetEnregistre>("generer_sujet", { typeId, chapitres, dureeMinutes, tache }),
  sujetsEnregistres: () => invoke<ResumeSujet[]>("sujets_enregistres"),
  dernieresNotes: () => invoke<NoteCopie[]>("dernieres_notes"),
  enregistrerExport: (nom: string, contenu: string, dossier: string | null) =>
    invoke<string>("enregistrer_export", { nom, contenu, dossier }),
  montrerFichier: (chemin: string) => invoke<void>("montrer_fichier", { chemin }),
  sujetEnregistre: (id: number) => invoke<SujetEnregistre>("sujet_enregistre", { id }),
  supprimerSujet: (id: number) => invoke<void>("supprimer_sujet", { id }),
  rendreCopie: (sujetId: number, reponses: string[], dureeMs: number) =>
    invoke<Copie>("rendre_copie", { sujetId, reponses, dureeMs }),
  corrigerRedactions: (copieId: number, tache?: string) => invoke<Copie>("corriger_redactions", { copieId, tache }),
  retirerQuestion: (copieId: number, numero: number, retiree: boolean) =>
    invoke<Copie>("retirer_question", { copieId, numero, retiree }),
};

/** Les erreurs Rust arrivent sous forme de texte ; on garantit un message lisible. */
export function messageErreur(e: unknown): string {
  if (typeof e === "string") return e;
  if (e instanceof Error) return e.message;
  return "Erreur inconnue.";
}
