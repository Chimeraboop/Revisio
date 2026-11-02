// Sur le téléphone, il n'y a pas de cœur Rust : les écrans de l'app de bureau appellent toujours
// `invoke("commande", {…})`, et c'est ce module qui répond, avec les données gardées dans le téléphone.
// (Le module `@tauri-apps/api/core` est remplacé par `./tauri-core` lors de la compilation de la web-app.)
import type { FiltreSeance, InfosApp, NouvelleRevision, SensVocabulaire } from "../lib/api";
import * as d from "./donnees";
import { appliquer, definirLocaux, modele, reglages, reglagesSeances } from "./magasin";

type Args = Record<string, unknown>;

const SUR_LE_PC = "Disponible sur l'ordinateur seulement.";

function jourLocal(ms: number): string {
  const x = new Date(ms);
  return `${x.getFullYear()}-${String(x.getMonth() + 1).padStart(2, "0")}-${String(x.getDate()).padStart(2, "0")}`;
}

const COMMANDES: Record<string, (a: Args) => unknown> = {
  infos_app: (): InfosApp => ({ version: __VERSION__, dossierDonnees: "", vaultsDetectes: [] }),
  lire_reglages: () => reglages(),
  est_vault_obsidian: () => false,
  verifier_internet: async () => {
    if (!navigator.onLine) return false;
    try {
      await fetch("https://www.googleapis.com/generate_204", { mode: "no-cors", cache: "no-store" });
      return true;
    } catch {
      return false;
    }
  },
  etat_indexation: () => ({ enCours: false, dernier: null, cartes: null, erreur: null }),
  annuler_tache: () => undefined,

  // Séances
  resume_revision: (a) => d.resume(modele(), Date.now(), a.debutJour as number, reglagesSeances()),
  preparer_seance: (a) => d.preparerSeance(modele(), a.filtre as FiltreSeance, Date.now(), reglagesSeances()),
  enregistrer_revision: (a) => appliquer(d.enregistrerRevision(modele(), a.revision as NouvelleRevision, Date.now())),
  suspendre_carte: (a) => appliquer(d.suspendre(modele(), a.carteId as number)),
  definir_matieres_exclues: (a) => definirLocaux({ matieresExclues: [...new Set((a.matieres as string[]).map((m) => m.trim()).filter(Boolean))].sort() }),
  definir_sens_vocabulaire: (a) => definirLocaux({ sensVocabulaire: a.sens as SensVocabulaire }),
  corriger_reponse: () => {
    throw new Error("La correction par l'IA se fait sur l'ordinateur : note-toi toi-même ici.");
  },

  // Tableau de bord, planning, carnet d'erreurs
  tableau_de_bord: () => d.tableauDeBord(modele()),
  evaluations: (a) => d.evaluations(modele(), (a.depuis as string) ?? jourLocal(Date.now())),
  revues_du_jour: (a) => d.revuesDuJour(modele(), a.debutJour as number),
  carnet_erreurs: (a) => d.carnet(modele(), (a.matiere as string | null) ?? null, !!a.avecComprises),
  marquer_erreur_comprise: (a) => appliquer(d.marquerComprise(modele(), a.erreurId as number, !!a.comprise, Date.now())),
  cartes_choisies: (a) => d.cartesChoisies(modele(), a.ids as number[]),

  // Notes
  arbre: () => d.arbre(modele()),
  note: (a) => d.note(modele(), a.chemin as string),
  resoudre_lien: (a) => d.resoudre(modele(), a.cible as string, (a.source as string | null) ?? null),
  enregistrer_note: async (a) => {
    await appliquer(d.enregistrerNote(modele(), a.chemin as string, a.contenu as string, a.avant as string, Date.now()));
    return 0;
  },
  lire_fichier_vault: () => {
    throw new Error("Images : visibles sur l'ordinateur seulement pour l'instant.");
  },
  ouvrir_dans_obsidian: () => undefined,
  ouvrir_lien_externe: (a) => {
    window.open(a.url as string, "_blank", "noopener");
  },
  annuler_ecriture: () => {
    throw new Error("Pour annuler une modification, utilise Révisio sur ton ordinateur (Réglages, Dernières écritures).");
  },
  ajouter_image: () => {
    throw new Error(SUR_LE_PC);
  },
  importer_image: () => {
    throw new Error(SUR_LE_PC);
  },
};

export async function repondre<T>(commande: string, args: Args = {}): Promise<T> {
  const f = COMMANDES[commande];
  if (!f) throw new Error(`${SUR_LE_PC} (${commande})`);
  return (await f(args)) as T;
}
