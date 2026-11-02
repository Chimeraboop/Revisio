// Révisio sur téléphone : données et calculs, sans réseau ni stockage (fonctions pures, testées).
//
// Le PC publie un « contenu » (cartes, état FSRS, révisions récentes, erreurs, évaluations, réglages, notes).
// Le téléphone garde son propre « journal » (révisions faites, nouveaux états, erreurs, « j'ai compris », cartes
// signalées, notes modifiées), qu'il publie à son tour et que le PC fusionne (voir src-tauri/src/synchro.rs).
// Ici, on reconstitue l'état « vu du téléphone » (contenu + journal) et on refait les calculs du PC
// (src-tauri/src/revision.rs) : choix des cartes d'une séance, compteurs, carnet d'erreurs, notes.
import type {
  Arbre,
  CarteSeance,
  ChapitreSeance,
  ErreurCarnet,
  Evaluation,
  FiltreSeance,
  GenreCarte,
  GenreNote,
  GroupeRevision,
  Note,
  NoteResume,
  NouvelleRevision,
  ResumeRevision,
  RevuesChapitre,
  Seance,
  TableauDeBord,
} from "../lib/api";

// ---------- Formats échangés (mêmes noms que synchro.rs) ----------

export interface ReglagesSynchro {
  nouvellesParJour: number;
  nouvellesParJourLangues: number;
  dureeSeance: number;
  dureeSeanceLangues: number;
  entrelacement: boolean;
  vocabVersFrancais: "retourner" | "taper";
  vocabDepuisFrancais: "retourner" | "taper";
  sensVocabulaire: "les_deux" | "etranger" | "francais";
  matieresExclues: string[];
  ecritureAutorisee: boolean;
}

export interface CarteSynchro {
  cle: string;
  genre: GenreCarte;
  noteChemin: string;
  matiere: string | null;
  chapitre: string | null;
  section: string;
  question: string;
  reponse: string;
  donnees: Record<string, unknown>;
  extrait: string;
  suspendue: boolean;
  creeeLe: number;
  etatFsrs: string | null;
  echeance: number | null;
  introduiteLe: number | null;
  etatMaj: number;
}

export interface RevisionSynchro {
  uid: string;
  cle: string;
  date: number;
  note: number;
  confiance: string;
  correcte: boolean;
  reponseDonnee?: string | null;
  dureeMs: number;
}

export interface ErreurSynchro {
  uid: string;
  cle: string | null;
  date: number;
  genre: GenreCarte;
  matiere: string | null;
  chapitre: string | null;
  noteChemin: string;
  section: string;
  question: string;
  reponseAttendue: string;
  reponseDonnee: string | null;
  extrait: string;
  comprise: boolean;
  compriseLe: number;
}

export interface NoteSynchro {
  chemin: string;
  nom: string;
  titre: string;
  matiere: string | null;
  chapitre: string | null;
  genre: GenreNote;
  tags: string[];
  frontmatter: Record<string, unknown> | null;
  contenu: string;
  empreinte: string;
}

export interface Contenu {
  version: number;
  appareil: string;
  genereLe: number;
  reglages: ReglagesSynchro;
  cartes: CarteSynchro[];
  revisions: RevisionSynchro[];
  erreurs: ErreurSynchro[];
  evaluations: Evaluation[];
  notes: NoteSynchro[];
  /** Modifications de notes déjà traitées par le PC : `appareil|chemin|date`. */
  notesTraitees: string[];
}

export interface EtatSynchro {
  cle: string;
  etatFsrs: string;
  echeance: number;
  introduiteLe: number | null;
  maj: number;
}

export interface ErreurNouvelle {
  uid: string;
  cle: string;
  date: number;
  reponseDonnee: string | null;
}

export interface Comprise {
  uid: string;
  comprise: boolean;
  date: number;
}

export interface NoteModifiee {
  chemin: string;
  /** Empreinte de la version reçue du PC à partir de laquelle la note a été modifiée. */
  base: string;
  contenu: string;
  date: number;
}

export interface Journal {
  version: number;
  appareil: string;
  nom: string;
  revisions: RevisionSynchro[];
  etats: EtatSynchro[];
  erreurs: ErreurNouvelle[];
  comprises: Comprise[];
  suspendues: string[];
  notes: NoteModifiee[];
}

export const VERSION_FORMAT = 1;

export function journalVide(appareil: string, nom = "iPhone"): Journal {
  return { version: VERSION_FORMAT, appareil, nom, revisions: [], etats: [], erreurs: [], comprises: [], suspendues: [], notes: [] };
}

// ---------- Vue du téléphone : contenu + journal ----------

/** Carte telle que le téléphone la voit (`id` : numéro local, valable pour ce contenu). */
export interface CarteLocale extends CarteSynchro {
  id: number;
}

export interface ErreurLocale extends ErreurSynchro {
  id: number;
}

export interface Modele {
  contenu: Contenu;
  journal: Journal;
  cartes: CarteLocale[];
  parId: Map<number, CarteLocale>;
  parCle: Map<string, CarteLocale>;
  /** Révisions du PC (400 derniers jours) et du téléphone, sans doublon. */
  revisions: RevisionSynchro[];
  erreurs: ErreurLocale[];
  /** Notes, avec le texte modifié sur le téléphone s'il y en a un. */
  notes: Map<string, NoteSynchro>;
}

export const HORS_MATIERE = "Hors matière";

export function construire(contenu: Contenu, journal: Journal): Modele {
  const etats = new Map<string, EtatSynchro>();
  for (const e of journal.etats) {
    const deja = etats.get(e.cle);
    if (!deja || deja.maj < e.maj) etats.set(e.cle, e);
  }
  const suspendues = new Set(journal.suspendues);
  const cartes: CarteLocale[] = contenu.cartes.map((c, i) => {
    const e = etats.get(c.cle);
    const local = e && e.maj > c.etatMaj;
    return {
      ...c,
      id: i + 1,
      suspendue: c.suspendue || suspendues.has(c.cle),
      etatFsrs: local ? e.etatFsrs : c.etatFsrs,
      echeance: local ? e.echeance : c.echeance,
      introduiteLe: c.introduiteLe ?? (local ? (e.introduiteLe ?? e.maj) : null),
      etatMaj: local ? e.maj : c.etatMaj,
    };
  });
  const parId = new Map(cartes.map((c) => [c.id, c]));
  const parCle = new Map(cartes.map((c) => [c.cle, c]));

  const vues = new Set(contenu.revisions.map((r) => r.uid));
  const revisions = [...contenu.revisions, ...journal.revisions.filter((r) => !vues.has(r.uid))].sort((a, b) => a.date - b.date);

  const comprises = new Map<string, Comprise>();
  for (const c of journal.comprises) {
    const deja = comprises.get(c.uid);
    if (!deja || deja.date < c.date) comprises.set(c.uid, c);
  }
  const connues = new Set(contenu.erreurs.map((e) => e.uid));
  const erreursBrutes: ErreurSynchro[] = [...contenu.erreurs];
  for (const x of journal.erreurs) {
    const c = parCle.get(x.cle);
    if (!c || connues.has(x.uid)) continue;
    erreursBrutes.push({
      uid: x.uid,
      cle: x.cle,
      date: x.date,
      genre: c.genre,
      matiere: c.matiere,
      chapitre: c.chapitre,
      noteChemin: c.noteChemin,
      section: c.section,
      question: c.question,
      reponseAttendue: c.reponse,
      reponseDonnee: x.reponseDonnee,
      extrait: c.extrait,
      comprise: false,
      compriseLe: 0,
    });
  }
  const erreurs: ErreurLocale[] = erreursBrutes.map((e, i) => {
    const c = comprises.get(e.uid);
    return c && c.date > e.compriseLe ? { ...e, id: i + 1, comprise: c.comprise, compriseLe: c.date } : { ...e, id: i + 1 };
  });

  const notes = new Map(contenu.notes.map((n) => [n.chemin, n]));
  for (const m of journal.notes) {
    const n = notes.get(m.chemin);
    if (n) notes.set(m.chemin, { ...n, contenu: m.contenu });
  }
  return { contenu, journal, cartes, parId, parCle, revisions, erreurs, notes };
}

// ---------- Séances (revision.rs) ----------

function sensExclu(recto: string): string | null {
  return recto === "etranger" ? "depuis_fr" : recto === "francais" ? "vers_fr" : null;
}

function dansGroupe(c: CarteLocale, g: GroupeRevision): boolean {
  return g === "langues" ? c.genre === "vocabulaire" : c.genre !== "vocabulaire";
}

function dansChapitres(c: CarteLocale, chapitres: ChapitreSeance[]): boolean {
  return chapitres.some((ch) => c.matiere === ch.matiere && c.chapitre === ch.chapitre);
}

interface Conditions extends FiltreSeance {
  exclues: string[];
  sensExclu: string | null;
}

function utilisable(c: CarteLocale, f: Conditions): boolean {
  if (c.suspendue) return false;
  if (f.matiere != null && c.matiere !== f.matiere) return false;
  if (f.chapitre != null && c.chapitre !== f.chapitre) return false;
  if (f.genres?.length && !f.genres.includes(c.genre)) return false;
  if (f.groupe && !dansGroupe(c, f.groupe)) return false;
  if (f.chapitres?.length && !dansChapitres(c, f.chapitres)) return false;
  if (f.exclues.length && f.exclues.includes(c.matiere ?? HORS_MATIERE)) return false;
  if (f.sensExclu && c.genre === "vocabulaire" && (c.donnees as { sens?: string }).sens === f.sensExclu) return false;
  return true;
}

function erreursNonComprises(m: Modele): Map<string, number> {
  const n = new Map<string, number>();
  for (const e of m.erreurs) if (!e.comprise && e.cle) n.set(e.cle, (n.get(e.cle) ?? 0) + 1);
  return n;
}

function oublis(c: CarteLocale): number {
  if (!c.etatFsrs) return 0;
  try {
    return Number(JSON.parse(c.etatFsrs).lapses ?? 0) || 0;
  } catch {
    return 0;
  }
}

function enSeance(c: CarteLocale, m: Modele): CarteSeance {
  return {
    id: c.id,
    genre: c.genre,
    noteChemin: c.noteChemin,
    titreNote: m.notes.get(c.noteChemin)?.titre ?? c.noteChemin,
    matiere: c.matiere,
    chapitre: c.chapitre,
    section: c.section,
    question: c.question,
    reponse: c.reponse,
    donnees: c.donnees,
    extrait: c.extrait,
    etatFsrs: c.etatFsrs,
    echeance: c.echeance,
  };
}

function nouvellesDuJour(m: Modele, debutJour: number, g: GroupeRevision): number {
  return m.cartes.filter((c) => c.introduiteLe != null && c.introduiteLe >= debutJour && dansGroupe(c, g)).length;
}

function quota(r: ReglagesSynchro, g: GroupeRevision): number {
  return g === "langues" ? r.nouvellesParJourLangues : r.nouvellesParJour;
}

export function preparerSeance(m: Modele, filtre: FiltreSeance, maintenant: number, reglages: ReglagesSynchro): Seance {
  const f: Conditions = {
    ...filtre,
    exclues: filtre.groupe ? reglages.matieresExclues : [],
    sensExclu: sensExclu(filtre.recto ?? reglages.sensVocabulaire),
  };
  const candidates = m.cartes.filter((c) => utilisable(c, f));
  const dues = candidates
    .filter((c) => c.echeance != null && c.echeance <= maintenant)
    .sort((a, b) => a.echeance! - b.echeance!)
    .slice(0, filtre.limiteDues ?? 500);
  const err = erreursNonComprises(m);
  const score = (c: CarteLocale) => 2 * (err.get(c.cle) ?? 0) + oublis(c);
  const faibles = candidates
    .filter((c) => c.etatFsrs && c.echeance != null && c.echeance > maintenant && score(c) > 0)
    .sort((a, b) => score(b) - score(a))
    .slice(0, 30);
  const nouvelles: CarteLocale[] = [];
  for (const g of filtre.groupe ? [filtre.groupe] : (["langues", "autres"] as GroupeRevision[])) {
    const reste = Math.max(0, quota(reglages, g) - nouvellesDuJour(m, filtre.debutJour, g));
    nouvelles.push(...candidates.filter((c) => dansGroupe(c, g) && !c.etatFsrs).slice(0, reste));
  }
  let planifiees: CarteLocale[] = [];
  if (filtre.planifies?.length) {
    const revuesAujourdhui = new Set(m.revisions.filter((r) => r.date >= filtre.debutJour).map((r) => r.cle));
    planifiees = candidates
      .filter((c) => dansChapitres(c, filtre.planifies!) && !revuesAujourdhui.has(c.cle))
      .sort((a, b) => Number(!a.etatFsrs) - Number(!b.etatFsrs) || (a.echeance ?? 0) - (b.echeance ?? 0) || a.id - b.id)
      .slice(0, 300);
  }
  const s = (l: CarteLocale[]) => l.map((c) => enSeance(c, m));
  return { planifiees: s(planifiees), dues: s(dues), faibles: s(faibles), nouvelles: s(nouvelles) };
}

export function dureeMoyenneMs(m: Modele): number {
  const dernieres = m.revisions.slice(-200);
  if (!dernieres.length) return 20_000;
  const moyenne = dernieres.reduce((t, r) => t + r.dureeMs, 0) / dernieres.length;
  return Math.min(60_000, Math.max(8_000, Math.round(moyenne)));
}

function trierChapitres(a: { matiere: string | null; chapitre: string | null }, b: { matiere: string | null; chapitre: string | null }) {
  const cle = (x: { matiere: string | null; chapitre: string | null }) =>
    [x.matiere == null ? 1 : 0, x.matiere ?? "", x.chapitre != null ? 1 : 0, x.chapitre ?? ""] as const;
  const [p, q] = [cle(a), cle(b)];
  for (let i = 0; i < 4; i++) {
    if (p[i] < q[i]) return -1;
    if (p[i] > q[i]) return 1;
  }
  return 0;
}

export function resume(m: Modele, maintenant: number, debutJour: number, reglages: ReglagesSynchro): ResumeRevision {
  const actives = m.cartes.filter((c) => !c.suspendue);
  const due = (c: CarteLocale) => c.echeance != null && c.echeance <= maintenant;
  const parMatiere = new Map<string, { matiere: string; dues: number; nouvelles: number; total: number }>();
  const parChapitre = new Map<string, ResumeRevision["chapitres"][number]>();
  const parGenre = new Map<GenreCarte, number>();
  for (const c of actives) {
    const nom = c.matiere ?? HORS_MATIERE;
    const mt = parMatiere.get(nom) ?? { matiere: nom, dues: 0, nouvelles: 0, total: 0 };
    mt.dues += Number(due(c));
    mt.nouvelles += Number(!c.etatFsrs);
    mt.total++;
    parMatiere.set(nom, mt);
    const k = JSON.stringify([c.matiere, c.chapitre]);
    const ch = parChapitre.get(k) ?? { matiere: c.matiere, chapitre: c.chapitre, dues: 0, nouvelles: 0, total: 0, vocabulaire: 0 };
    ch.dues += Number(due(c));
    ch.nouvelles += Number(!c.etatFsrs);
    ch.total++;
    ch.vocabulaire += Number(c.genre === "vocabulaire");
    parChapitre.set(k, ch);
    parGenre.set(c.genre, (parGenre.get(c.genre) ?? 0) + 1);
  }
  const f: Conditions = { debutJour, exclues: reglages.matieresExclues, sensExclu: sensExclu(reglages.sensVocabulaire) };
  const groupes = (["langues", "autres"] as GroupeRevision[]).map((g) => {
    const dans = actives.filter((c) => dansGroupe(c, g) && utilisable(c, f));
    return {
      groupe: g,
      dues: dans.filter(due).length,
      nouvellesDisponibles: dans.filter((c) => !c.etatFsrs).length,
      nouvellesRestantes: Math.max(0, quota(reglages, g) - nouvellesDuJour(m, debutJour, g)),
    };
  });
  return {
    matieres: [...parMatiere.values()].sort((a, b) => (a.matiere < b.matiere ? -1 : a.matiere > b.matiere ? 1 : 0)),
    chapitres: [...parChapitre.values()].sort(trierChapitres),
    groupes,
    parGenre: [...parGenre.entries()].sort((a, b) => (a[0] < b[0] ? -1 : 1)),
    dureeMoyenneMs: dureeMoyenneMs(m),
  };
}

/** Identifiant unique d'un évènement du téléphone. */
export function uid(appareil: string, genre: string): string {
  const hasard = Math.random().toString(36).slice(2, 10);
  return `${appareil}:${genre}${Date.now().toString(36)}${hasard}`;
}

/** Une réponse en séance : ajoutée au journal (révision, nouvel état, erreur éventuelle). Renvoie le journal modifié. */
export function enregistrerRevision(m: Modele, r: NouvelleRevision, maintenant: number): Journal {
  const c = m.parId.get(r.carteId);
  if (!c) throw new Error("Carte introuvable.");
  const j = m.journal;
  const revision: RevisionSynchro = {
    uid: uid(j.appareil, "r"),
    cle: c.cle,
    date: maintenant,
    note: r.note,
    confiance: r.confiance,
    correcte: r.correcte,
    reponseDonnee: r.reponseDonnee,
    dureeMs: Math.max(0, r.dureeMs),
  };
  const etat: EtatSynchro = { cle: c.cle, etatFsrs: r.etatFsrs, echeance: r.echeance, introduiteLe: c.introduiteLe ?? maintenant, maj: maintenant };
  const erreurs = r.correcte ? j.erreurs : [...j.erreurs, { uid: uid(j.appareil, "e"), cle: c.cle, date: maintenant, reponseDonnee: r.reponseDonnee }];
  return { ...j, revisions: [...j.revisions, revision], etats: [...j.etats.filter((e) => e.cle !== c.cle), etat], erreurs };
}

export function suspendre(m: Modele, carteId: number): Journal {
  const c = m.parId.get(carteId);
  if (!c) throw new Error("Carte introuvable.");
  return m.journal.suspendues.includes(c.cle) ? m.journal : { ...m.journal, suspendues: [...m.journal.suspendues, c.cle] };
}

// ---------- Tableau de bord, planning, carnet d'erreurs ----------

function jourLocal(ms: number): string {
  const d = new Date(ms);
  return `${d.getFullYear()}-${String(d.getMonth() + 1).padStart(2, "0")}-${String(d.getDate()).padStart(2, "0")}`;
}

export function tableauDeBord(m: Modele): TableauDeBord {
  const actives = m.cartes.filter((c) => !c.suspendue);
  const jours = new Map<string, { jour: string; revisions: number; justes: number; dureeMs: number }>();
  for (const r of m.revisions) {
    const k = jourLocal(r.date);
    const j = jours.get(k) ?? { jour: k, revisions: 0, justes: 0, dureeMs: 0 };
    j.revisions++;
    j.justes += Number(r.correcte);
    j.dureeMs += r.dureeMs;
    jours.set(k, j);
  }
  const err = erreursNonComprises(m);
  const faibles = actives
    .filter((c) => c.etatFsrs)
    .map((c) => ({ c, erreurs: err.get(c.cle) ?? 0, oublis: oublis(c) }))
    .filter((x) => 2 * x.erreurs + x.oublis > 0)
    .sort((a, b) => 2 * b.erreurs + b.oublis - (2 * a.erreurs + a.oublis))
    .slice(0, 8)
    .map(({ c, erreurs, oublis }) => ({
      carteId: c.id,
      genre: c.genre,
      question: c.question,
      reponse: c.reponse,
      matiere: c.matiere,
      chapitre: c.chapitre,
      noteChemin: c.noteChemin,
      section: c.section,
      erreurs,
      oublis,
    }));
  return {
    cartes: actives.map((c) => ({ matiere: c.matiere, chapitre: c.chapitre, genre: c.genre, etatFsrs: c.etatFsrs })),
    jours: [...jours.values()].sort((a, b) => (a.jour < b.jour ? 1 : -1)).slice(0, 365),
    faibles,
  };
}

export function evaluations(m: Modele, depuis: string): Evaluation[] {
  return m.contenu.evaluations.filter((e) => e.jour >= depuis).sort((a, b) => (a.jour < b.jour ? -1 : a.jour > b.jour ? 1 : a.id - b.id));
}

export function revuesDuJour(m: Modele, debutJour: number): RevuesChapitre[] {
  const parChapitre = new Map<string, { matiere: string | null; chapitre: string | null; cles: Set<string> }>();
  for (const r of m.revisions) {
    if (r.date < debutJour) continue;
    const c = m.parCle.get(r.cle);
    if (!c) continue;
    const k = JSON.stringify([c.matiere, c.chapitre]);
    const x = parChapitre.get(k) ?? { matiere: c.matiere, chapitre: c.chapitre, cles: new Set<string>() };
    x.cles.add(c.cle);
    parChapitre.set(k, x);
  }
  return [...parChapitre.values()].map((x) => ({ matiere: x.matiere, chapitre: x.chapitre, cartes: x.cles.size }));
}

/** Carnet d'erreurs : la dernière erreur de chaque carte, de la plus récente à la plus ancienne. */
export function carnet(m: Modele, matiere: string | null, avecComprises: boolean): ErreurCarnet[] {
  const fois = new Map<string, number>();
  const derniere = new Map<string, ErreurLocale>();
  for (const e of m.erreurs) {
    const k = e.cle ?? `sans-carte:${e.uid}`;
    fois.set(k, (fois.get(k) ?? 0) + 1);
    const d = derniere.get(k);
    if (!d || d.date <= e.date) derniere.set(k, e);
  }
  return [...derniere.entries()]
    .filter(([, e]) => (matiere == null || e.matiere === matiere) && (avecComprises || !e.comprise))
    .sort((a, b) => b[1].date - a[1].date)
    .map(([k, e]) => ({
      id: e.id,
      carteId: e.cle ? (m.parCle.get(e.cle)?.id ?? null) : null,
      date: e.date,
      genre: e.genre,
      matiere: e.matiere,
      chapitre: e.chapitre,
      noteChemin: e.noteChemin,
      titreNote: m.notes.get(e.noteChemin)?.titre ?? e.noteChemin,
      section: e.section,
      question: e.question,
      reponseAttendue: e.reponseAttendue,
      reponseDonnee: e.reponseDonnee,
      extrait: e.extrait,
      comprise: e.comprise,
      fois: fois.get(k) ?? 1,
    }));
}

/** « J'ai compris » : toutes les erreurs de la même carte. */
export function marquerComprise(m: Modele, erreurId: number, comprise: boolean, maintenant: number): Journal {
  const e = m.erreurs.find((x) => x.id === erreurId);
  if (!e) throw new Error("Erreur introuvable.");
  const memes = m.erreurs.filter((x) => x.id === erreurId || (e.cle != null && x.cle === e.cle));
  const uids = new Set(memes.map((x) => x.uid));
  return {
    ...m.journal,
    comprises: [...m.journal.comprises.filter((c) => !uids.has(c.uid)), ...memes.map((x) => ({ uid: x.uid, comprise, date: maintenant }))],
  };
}

export function cartesChoisies(m: Modele, ids: number[]): CarteSeance[] {
  return ids.map((id) => m.parId.get(id)).filter((c): c is CarteLocale => !!c && !c.suspendue).map((c) => enSeance(c, m));
}

// ---------- Notes (navigation.rs) ----------

export function arbre(m: Modele): Arbre {
  const a: Arbre = { matieres: [], autres: [] };
  const notes = [...m.notes.values()].sort((x, y) => (x.chemin < y.chemin ? -1 : 1));
  for (const n of notes) {
    const resumeNote: NoteResume = { chemin: n.chemin, titre: n.titre, genre: n.genre };
    if (n.matiere) {
      let mt = a.matieres.find((x) => x.nom === n.matiere);
      if (!mt) a.matieres.push((mt = { nom: n.matiere, notes: [], chapitres: [] }));
      if (n.chapitre) {
        let ch = mt.chapitres.find((x) => x.nom === n.chapitre);
        if (!ch) mt.chapitres.push((ch = { nom: n.chapitre, notes: [] }));
        ch.notes.push(resumeNote);
      } else {
        mt.notes.push(resumeNote);
      }
    } else {
      const dossier = n.chemin.includes("/") ? n.chemin.slice(0, n.chemin.lastIndexOf("/")) : "";
      let g = a.autres.find((x) => x.nom === dossier);
      if (!g) a.autres.push((g = { nom: dossier, notes: [] }));
      g.notes.push(resumeNote);
    }
  }
  return a;
}

export function note(m: Modele, chemin: string): Note {
  const n = m.notes.get(chemin);
  if (!n) throw new Error(`Note introuvable : ${chemin}`);
  const nom = n.nom.toLowerCase();
  const retroliens = [...m.notes.values()]
    .filter((x) => x.chemin !== chemin && new RegExp(`\\[\\[(?:[^\\]|#]*/)?${echapper(nom)}(?:[#|][^\\]]*)?\\]\\]`, "i").test(x.contenu))
    .map((x) => ({ chemin: x.chemin, titre: x.titre, genre: x.genre }));
  return {
    chemin: n.chemin,
    nom: n.nom,
    titre: n.titre,
    matiere: n.matiere,
    chapitre: n.chapitre,
    genre: n.genre,
    tags: n.tags,
    frontmatter: n.frontmatter,
    legendes: [],
    retroliens,
    contenu: n.contenu,
    urlObsidian: null,
  };
}

function echapper(t: string): string {
  return t.replace(/[.*+?^${}()|[\]\\]/g, "\\$&");
}

/** Cible d'un lien [[…]] comme Obsidian : par chemin, sinon par nom (même dossier que la source d'abord). */
export function resoudre(m: Modele, cible: string, source: string | null): { chemin: string; genre: "note" | "fichier" } | null {
  const c = cible.trim().replace(/^\/+/, "");
  const sansMd = c.replace(/\.md$/i, "");
  if (c.includes("/")) {
    const voulu = `${sansMd}.md`.toLowerCase();
    const n = [...m.notes.keys()].find((k) => k.toLowerCase() === voulu);
    if (n) return { chemin: n, genre: "note" };
  }
  const nom = sansMd.split("/").pop()!.toLowerCase();
  const candidats = [...m.notes.values()].filter((n) => n.nom.toLowerCase() === nom);
  if (!candidats.length) return null;
  const dossierSource = source?.includes("/") ? source.slice(0, source.lastIndexOf("/")) : "";
  const memeDossier = candidats.find((n) => n.chemin.slice(0, Math.max(0, n.chemin.lastIndexOf("/"))) === dossierSource);
  return { chemin: (memeDossier ?? candidats[0]).chemin, genre: "note" };
}

/** Note modifiée sur le téléphone (le PC l'écrira, ou la proposera si elle a aussi changé sur le PC). */
export function enregistrerNote(m: Modele, chemin: string, contenu: string, avant: string, maintenant: number): Journal {
  const n = m.notes.get(chemin);
  const origine = m.contenu.notes.find((x) => x.chemin === chemin);
  if (!n || !origine) throw new Error(`Note introuvable : ${chemin}`);
  const sansCr = (t: string) => t.replace(/\r\n/g, "\n");
  if (sansCr(n.contenu) !== sansCr(avant)) throw new Error("La note a changé entre-temps : recharge-la avant de la modifier.");
  // Les retours à la ligne de la note d'origine (« \r\n » sous Windows) sont gardés.
  const texte = origine.contenu.includes("\r\n") ? sansCr(contenu).replace(/\n/g, "\r\n") : sansCr(contenu);
  const autres = m.journal.notes.filter((x) => x.chemin !== chemin);
  if (texte === origine.contenu) return { ...m.journal, notes: autres };
  return { ...m.journal, notes: [...autres, { chemin, base: origine.empreinte, contenu: texte, date: maintenant }] };
}

/**
 * Après avoir reçu un nouveau contenu du PC : on retire du journal ce que le PC a déjà fusionné
 * (révisions et erreurs connues, états plus anciens que ceux du PC, notes déjà identiques, cartes déjà signalées).
 */
export function alleger(j: Journal, c: Contenu): Journal {
  const revues = new Set(c.revisions.map((r) => r.uid));
  const erreurs = new Set(c.erreurs.map((e) => e.uid));
  const etatsPc = new Map(c.cartes.map((x) => [x.cle, x.etatMaj]));
  const compriseLe = new Map(c.erreurs.map((e) => [e.uid, e.compriseLe]));
  const suspenduesPc = new Set(c.cartes.filter((x) => x.suspendue).map((x) => x.cle));
  const notesPc = new Map(c.notes.map((n) => [n.chemin, n.contenu]));
  const traitees = new Set(c.notesTraitees ?? []);
  return {
    ...j,
    revisions: j.revisions.filter((r) => !revues.has(r.uid)),
    etats: j.etats.filter((e) => (etatsPc.get(e.cle) ?? -1) < e.maj),
    erreurs: j.erreurs.filter((e) => !erreurs.has(e.uid)),
    comprises: j.comprises.filter((x) => (compriseLe.get(x.uid) ?? -1) < x.date),
    suspendues: j.suspendues.filter((k) => !suspenduesPc.has(k)),
    notes: j.notes.filter((n) => notesPc.get(n.chemin) !== n.contenu && !traitees.has(`${j.appareil}|${n.chemin}|${n.date}`)),
  };
}
