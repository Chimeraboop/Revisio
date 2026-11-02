// Révisio sur téléphone : état gardé en mémoire et dans le téléphone (IndexedDB), et synchronisation
// avec le Google Drive de l'élève. Les écrans s'y abonnent avec `useMagasin()`.
import { useSyncExternalStore } from "react";
import type { Reglages, SensVocabulaire } from "../lib/api";
import * as drive from "./drive";
import { alleger, construire, journalVide, type Contenu, type Journal, type Modele } from "./donnees";
import { demanderPersistance, ecrire, lire } from "./stockage";

const FICHIER_CONTENU = "contenu.json.gz";

/** Réglages propres au téléphone (le reste vient du PC). */
interface ReglagesLocaux {
  sensVocabulaire?: SensVocabulaire;
  matieresExclues?: string[];
}

export interface EtatMagasin {
  pret: boolean;
  modele: Modele | null;
  connecte: boolean;
  synchro: { enCours: boolean; derniere: number | null; erreur: string | null; reconnecter: boolean };
  locaux: ReglagesLocaux;
}

let etat: EtatMagasin = {
  pret: false,
  modele: null,
  connecte: false,
  synchro: { enCours: false, derniere: null, erreur: null, reconnecter: false },
  locaux: {},
};
let appareil = "";
const abonnes = new Set<() => void>();

function changer(modif: Partial<EtatMagasin>) {
  etat = { ...etat, ...modif };
  abonnes.forEach((f) => f());
}

export function useMagasin(): EtatMagasin {
  return useSyncExternalStore(
    (f) => {
      abonnes.add(f);
      return () => abonnes.delete(f);
    },
    () => etat,
  );
}

export function lireEtat(): EtatMagasin {
  return etat;
}

/** Modèle courant, ou erreur si rien n'a encore été reçu du PC. */
export function modele(): Modele {
  if (!etat.modele) throw new Error("Aucune donnée : synchronise d'abord avec ton Google Drive.");
  return etat.modele;
}

/** Réglages au format de l'app de bureau (les écrans réutilisés les attendent tels quels). */
export function reglages(): Reglages {
  const r = etat.modele?.contenu.reglages;
  return {
    cheminVault: "telephone",
    nomVaultObsidian: "",
    accueilTermine: true,
    theme: "systeme",
    accent: "monochrome",
    nouvellesParJour: r?.nouvellesParJour ?? 20,
    entrelacement: r?.entrelacement ?? true,
    vocabVersFrancais: r?.vocabVersFrancais ?? "retourner",
    vocabDepuisFrancais: r?.vocabDepuisFrancais ?? "taper",
    sensVocabulaire: etat.locaux.sensVocabulaire ?? r?.sensVocabulaire ?? "les_deux",
    dureeSeance: r?.dureeSeance ?? 20,
    nouvellesParJourLangues: r?.nouvellesParJourLangues ?? 20,
    dureeSeanceLangues: r?.dureeSeanceLangues ?? 10,
    modeleIa: "",
    modeleIaRapide: "",
    iaParMinute: 0,
    iaParJour: 0,
    modeleTts: "",
    voixA: "",
    voixB: "",
    voixVideo: "",
    iaLocale: "jamais",
    modeleLocal: "",
    adresseOllama: "",
    modeLocal: false,
    matieresExclues: etat.locaux.matieresExclues ?? r?.matieresExclues ?? [],
    ecritureAutorisee: r?.ecritureAutorisee ?? false,
    dossierPhotos: null,
    extensionPhotos: null,
    cartesIaAuto: false,
  };
}

/** Réglages des séances tels que le téléphone les applique (avec ses propres choix). */
export function reglagesSeances() {
  const m = modele();
  const r = reglages();
  return { ...m.contenu.reglages, sensVocabulaire: r.sensVocabulaire, matieresExclues: r.matieresExclues };
}

export async function definirLocaux(modif: ReglagesLocaux): Promise<Reglages> {
  const locaux = { ...etat.locaux, ...modif };
  await ecrire("reglages-locaux", locaux);
  changer({ locaux });
  return reglages();
}

/** Au lancement : données du téléphone, retour éventuel de Google, puis synchronisation si possible. */
export async function demarrer(): Promise<void> {
  await demanderPersistance();
  appareil = (await lire<string>("appareil")) ?? "";
  if (!appareil) {
    appareil = `iphone-${crypto.getRandomValues(new Uint32Array(2)).join("")}`;
    await ecrire("appareil", appareil);
  }
  const erreurGoogle = await drive.lireRetourGoogle();
  const [contenu, journal, locaux, connecte] = await Promise.all([
    lire<Contenu>("contenu"),
    lire<Journal>("journal"),
    lire<ReglagesLocaux>("reglages-locaux"),
    drive.estConnecte(),
  ]);
  const j = journal ?? journalVide(appareil, nomAppareil());
  changer({
    pret: true,
    modele: contenu ? construire(contenu, j) : null,
    connecte,
    locaux: locaux ?? {},
    synchro: { ...etat.synchro, erreur: erreurGoogle, reconnecter: !!erreurGoogle && connecte === false && !!contenu },
  });
  if (connecte && navigator.onLine) void synchroniser({ silencieux: true });
}

function nomAppareil(): string {
  const ua = navigator.userAgent;
  return /iPhone/.test(ua) ? "ton iPhone" : /iPad/.test(ua) ? "ton iPad" : /Android/.test(ua) ? "ton téléphone Android" : "ton téléphone";
}

/** Nouveau journal (après une réponse, une note modifiée…) : gardé tout de suite, envoyé un peu plus tard. */
export async function appliquer(journal: Journal): Promise<void> {
  await ecrire("journal", journal);
  const m = etat.modele;
  if (m) changer({ modele: construire(m.contenu, journal) });
  programmerEnvoi();
}

let minuteur: ReturnType<typeof setTimeout> | null = null;
function programmerEnvoi() {
  if (minuteur) clearTimeout(minuteur);
  // Pas d'envoi à chaque réponse : 30 s après la dernière, si Internet et la connexion Google le permettent.
  minuteur = setTimeout(() => void synchroniser({ silencieux: true }), 30_000);
}

/**
 * Synchronisation : reçoit le contenu du PC s'il a changé, puis envoie le journal du téléphone.
 * `silencieux` : sans repartir chez Google si la connexion a expiré (on attend que l'élève le demande).
 */
export async function synchroniser({ silencieux = false } = {}): Promise<void> {
  if (etat.synchro.enCours) return;
  changer({ synchro: { ...etat.synchro, enCours: true, erreur: null } });
  try {
    if (!(await drive.jetonValable())) {
      if (silencieux) {
        changer({ synchro: { ...etat.synchro, enCours: false } });
        return;
      }
      // Aller-retour chez Google (sans rien demander si l'élève y est encore connecté) : la page se recharge.
      drive.connecter(true);
      return;
    }
    const fichiers = await drive.lister();
    let contenu = etat.modele?.contenu ?? null;
    const distant = fichiers.find((f) => f.name === FICHIER_CONTENU);
    if (distant && distant.modifiedTime !== (await lire<string>("contenu-version"))) {
      contenu = await drive.decompresserJson<Contenu>(await drive.telecharger(distant.id));
      await ecrire("contenu", contenu);
      await ecrire("contenu-version", distant.modifiedTime);
    }
    if (!contenu) throw new Error("Rien à recevoir pour l'instant : ouvre Révisio sur ton PC et connecte-le à ton Google Drive (Réglages).");
    const avant = (await lire<Journal>("journal")) ?? journalVide(appareil, nomAppareil());
    let journal = alleger(avant, contenu);
    const signature = JSON.stringify(journal);
    const nom = `journal-${appareil}.json.gz`;
    const existant = fichiers.find((f) => f.name === nom);
    if (signature !== (await lire<string>("journal-envoye")) || !existant) {
      await drive.envoyer(nom, existant?.id ?? null, await drive.compresserJson(journal));
      await ecrire("journal-envoye", signature);
    }
    // Une réponse donnée pendant l'envoi ne doit pas être perdue : elle partira la prochaine fois.
    const maintenant = (await lire<Journal>("journal")) ?? avant;
    if (JSON.stringify(maintenant) !== JSON.stringify(avant)) {
      journal = alleger(maintenant, contenu);
      programmerEnvoi();
    }
    await ecrire("journal", journal);
    changer({
      modele: construire(contenu, journal),
      connecte: true,
      synchro: { enCours: false, derniere: Date.now(), erreur: null, reconnecter: false },
    });
  } catch (e) {
    const expire = e instanceof drive.JetonExpire;
    changer({
      synchro: {
        ...etat.synchro,
        enCours: false,
        erreur: expire ? "Reconnecte ton compte Google pour synchroniser." : e instanceof Error ? e.message : String(e),
        reconnecter: expire,
      },
    });
  }
}

export async function seDeconnecter(): Promise<void> {
  await drive.deconnecter();
  changer({ connecte: false });
}
