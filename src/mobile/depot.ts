// Photos de cours envoyées depuis le téléphone : réduites ici (comme le fait le PC), déposées dans le dossier caché
// du Google Drive, puis reprises par Révisio sur le PC qui en fait des notes avec l'IA.
//
// Un dépôt = `depot-<id>_<rang>.<jpg|pdf>` pour chaque fichier, puis EN DERNIER `depot-<id>.json` ({"n": nombre}) :
// le PC n'y touche que lorsque ce dernier fichier est là, donc jamais sur un dépôt incomplet.

export const FICHIERS_MAX = 10;
const COTE_MAX = 2000;
const PDF_MAX = 15 * 1024 * 1024;

export function idDepot(appareil: string, maintenant: number): string {
  return `${appareil}_${maintenant}`;
}

export function nomFichierDepot(id: string, rang: number, ext: string): string {
  return `depot-${id}_${rang}.${ext}`;
}

export function nomFinDepot(id: string): string {
  return `depot-${id}.json`;
}

export function estPdf(f: { name: string; type: string }): boolean {
  return f.type === "application/pdf" || /\.pdf$/i.test(f.name);
}

/** Où en est un dépôt, en français, d'après ce que le PC a publié (`pc` : statut de son lot, absent s'il ne l'a pas encore reçu). */
export function etatDuDepot(pc: { statut: string; message: string | null } | undefined): { texte: string; fini: boolean; probleme: boolean } {
  if (!pc) return { texte: "Envoyé. Ton PC le récupère à sa prochaine synchronisation (Révisio doit y être ouvert).", fini: false, probleme: false };
  switch (pc.statut) {
    case "attente":
      return { texte: pc.message ?? "Reçu par ton PC, en attente de traitement.", fini: false, probleme: false };
    case "en_cours":
      return { texte: "Ton PC rédige les notes…", fini: false, probleme: false };
    case "termine":
      return { texte: pc.message ?? "Notes ajoutées : elles arrivent ici à la prochaine synchronisation.", fini: true, probleme: false };
    case "illisible":
      return { texte: pc.message ?? "Rien d'exploitable dans ces photos.", fini: true, probleme: true };
    default:
      return { texte: pc.message ?? "Le traitement a échoué sur ton PC (voir Photos de cours).", fini: true, probleme: true };
  }
}

/** Taille de l'image réduite : le plus grand côté fait au plus `max` pixels, jamais agrandie. */
export function tailleReduite(largeur: number, hauteur: number, max = COTE_MAX): { largeur: number; hauteur: number } {
  const plusGrand = Math.max(largeur, hauteur);
  if (plusGrand <= max) return { largeur, hauteur };
  const f = max / plusGrand;
  return { largeur: Math.max(1, Math.round(largeur * f)), hauteur: Math.max(1, Math.round(hauteur * f)) };
}

/** Prépare un fichier choisi : un PDF part tel quel, une photo est tournée dans le bon sens, réduite et mise en JPEG. */
export async function preparerFichier(f: File): Promise<{ blob: Blob; ext: "jpg" | "pdf" }> {
  if (estPdf(f)) {
    if (f.size > PDF_MAX) throw new Error(`« ${f.name} » est trop gros (15 Mo au plus).`);
    return { blob: f, ext: "pdf" };
  }
  let image: ImageBitmap;
  try {
    image = await createImageBitmap(f, { imageOrientation: "from-image" });
  } catch {
    throw new Error(`« ${f.name} » n'est pas une photo que ce téléphone sait lire (essaie JPEG, PNG ou un PDF).`);
  }
  const { largeur, hauteur } = tailleReduite(image.width, image.height);
  const toile = document.createElement("canvas");
  toile.width = largeur;
  toile.height = hauteur;
  toile.getContext("2d")!.drawImage(image, 0, 0, largeur, hauteur);
  image.close();
  const blob = await new Promise<Blob | null>((ok) => toile.toBlob(ok, "image/jpeg", 0.85));
  if (!blob) throw new Error(`« ${f.name} » n'a pas pu être préparée.`);
  return { blob, ext: "jpg" };
}
