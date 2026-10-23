// Montage des vidéos de révision : chaque diapositive est dessinée sur un canevas 1280×720 (les points
// apparaissent au fil de la narration), puis images et narration sont encodées par WebView2 (WebCodecs, via
// Mediabunny chargé à la demande) : H.264 + AAC dans un MP4 lisible partout ; à défaut, VP9/VP8 + Opus en WebM.
import { chargerScene } from "../excalidraw/chargement";
import { versPng } from "../excalidraw/Dessin";
import type { Diapo, Video } from "../lib/api";
import { bouts, couperEnLignes, type Bout } from "./texte";

export const LARGEUR = 1280;
export const HAUTEUR = 720;
/** Silences autour de chaque narration : mêmes valeurs que dans `src-tauri/src/videos.rs`. */
export const PAUSE_AVANT_MS = 500;
export const PAUSE_APRES_MS = 700;
const IMAGES_PAR_SECONDE = 2;
const FREQUENCE_AUDIO = 48_000;

const COULEURS = {
  fond: "#f7f7f5",
  carte: "#ffffff",
  bord: "#e2e2dc",
  texte: "#1f1f1d",
  doux: "#6b6b66",
  accent: "#3d5ccc",
  blanc: "#ffffff",
};

export interface Segment {
  debut: number;
  duree: number;
}

/** Début et durée (ms) de chaque diapositive : pause, narration, pause. */
export function segments(diapos: Diapo[]): Segment[] {
  let debut = 0;
  return diapos.map((d) => {
    const s = { debut, duree: PAUSE_AVANT_MS + (d.dureeMs ?? 0) + PAUSE_APRES_MS };
    debut += s.duree;
    return s;
  });
}

/** Points visibles `t` ms après le début de la diapositive : le premier d'emblée, les autres au fil de la narration. */
export function pointsVisibles(n: number, t: number, dureeNarration: number): number {
  if (n === 0) return 0;
  const avancement = Math.max(0, t - PAUSE_AVANT_MS) / Math.max(1, dureeNarration * 0.85);
  return Math.min(n, 1 + Math.floor(avancement * n));
}

// ---------- Dessin ----------

type Ctx = CanvasRenderingContext2D;

function police(taille: number, position: Bout["position"], gras: boolean): string {
  const t = position === "normal" ? taille : Math.round(taille * 0.62);
  return `${gras ? "600 " : ""}${t}px "Segoe UI", "Segoe UI Symbol", sans-serif`;
}

function lignes(ctx: Ctx, texte: string, largeur: number, taille: number, gras = false): Bout[][] {
  return couperEnLignes(bouts(texte), largeur, (b) => {
    ctx.font = police(taille, b.position, gras);
    return ctx.measureText(b.texte).width;
  });
}

function dessinerLigne(ctx: Ctx, ligne: Bout[], x: number, y: number, taille: number, couleur: string, gras = false) {
  ctx.fillStyle = couleur;
  for (const b of ligne) {
    ctx.font = police(taille, b.position, gras);
    const decalage = b.position === "exposant" ? -taille * 0.38 : b.position === "indice" ? taille * 0.2 : 0;
    ctx.fillText(b.texte, x, y + decalage);
    x += ctx.measureText(b.texte).width;
  }
}

/** Coupe le texte à une seule ligne avec « … » si besoin (en-têtes). */
function uneLigne(ctx: Ctx, texte: string, largeur: number): string {
  if (ctx.measureText(texte).width <= largeur) return texte;
  let t = texte;
  while (t.length > 1 && ctx.measureText(`${t}…`).width > largeur) t = t.slice(0, -1);
  return `${t.trimEnd()}…`;
}

function rectangleArrondi(ctx: Ctx, x: number, y: number, l: number, h: number, r: number) {
  ctx.beginPath();
  ctx.roundRect(x, y, l, h, r);
}

interface Etat {
  video: Video;
  k: number;
  /** Temps écoulé dans la diapositive (ms). */
  t: number;
  segs: Segment[];
  image: ImageBitmap | null;
}

/** Points d'une diapositive à partir de `y` ; la taille baisse jusqu'à ce que tout tienne. */
function dessinerPoints(ctx: Ctx, points: string[], visibles: number, x: number, y: number, largeur: number, bas: number, clair: boolean) {
  let taille = 32;
  let decoupe: Bout[][][] = [];
  for (; taille >= 20; taille -= 2) {
    decoupe = points.map((p) => lignes(ctx, p, largeur - 34, taille));
    const hauteur = decoupe.reduce((s, l) => s + l.length * taille * 1.3 + taille * 0.65, 0);
    if (y + hauteur <= bas) break;
  }
  const interligne = taille * 1.3;
  decoupe.forEach((ls, i) => {
    if (i >= visibles) return;
    ctx.fillStyle = clair ? COULEURS.blanc : COULEURS.accent;
    ctx.beginPath();
    ctx.arc(x + 8, y + interligne * 0.62, taille * 0.2, 0, Math.PI * 2);
    ctx.fill();
    ls.forEach((l, j) => dessinerLigne(ctx, l, x + 34, y + interligne * (j + 0.85), taille, clair ? COULEURS.blanc : COULEURS.texte));
    y += ls.length * interligne + taille * 0.65;
  });
}

export function dessinerDiapo(ctx: Ctx, { video, k, t, segs, image }: Etat) {
  const d = video.diapos[k];
  const n = video.diapos.length;
  const titre = k === 0;
  const total = segs.at(-1)!.debut + segs.at(-1)!.duree;
  const visibles = pointsVisibles(d.points.length, t, d.dureeMs ?? 0);
  ctx.textBaseline = "alphabetic";
  ctx.textAlign = "left";

  // Fond (la première diapositive, qui présente le chapitre, est en couleur).
  ctx.fillStyle = titre ? COULEURS.accent : COULEURS.fond;
  ctx.fillRect(0, 0, LARGEUR, HAUTEUR);
  const couleurDouce = titre ? "rgba(255,255,255,0.75)" : COULEURS.doux;

  // En-tête : matière › chapitre, numéro de la diapositive.
  ctx.font = police(20, "normal", false);
  ctx.fillStyle = couleurDouce;
  ctx.fillText(uneLigne(ctx, `${video.matiere} › ${video.chapitre ?? "notes générales"}`, 900), 64, 52);
  ctx.textAlign = "right";
  ctx.fillText(`${k + 1} / ${n}`, LARGEUR - 64, 52);
  ctx.textAlign = "left";

  let y = 96;
  if (titre) {
    for (const l of lignes(ctx, video.titre, LARGEUR - 128, 58, true).slice(0, 2)) {
      y += 70;
      dessinerLigne(ctx, l, 64, y, 58, COULEURS.blanc, true);
    }
    y += 26;
  }
  const largeurTexte = image ? 560 : LARGEUR - 128;
  const tailleTitre = titre ? 32 : 44;
  // Sur la première diapositive, le titre de la diapositive répète souvent celui de la vidéo.
  const memeTitre = titre && d.titre.trim().toLowerCase() === video.titre.trim().toLowerCase();
  for (const l of memeTitre ? [] : lignes(ctx, d.titre, largeurTexte, tailleTitre, true).slice(0, 2)) {
    y += tailleTitre * 1.25;
    dessinerLigne(ctx, l, 64, y, tailleTitre, titre ? "rgba(255,255,255,0.92)" : COULEURS.texte, true);
  }
  ctx.fillStyle = titre ? COULEURS.blanc : COULEURS.accent;
  rectangleArrondi(ctx, 64, y + 18, 72, 6, 3);
  ctx.fill();
  y += 56;

  dessinerPoints(ctx, d.points, visibles, 64, y, largeurTexte, HAUTEUR - 70, titre);

  // Schéma à droite, dans une carte blanche.
  if (image) {
    // Zone disponible à droite ; la carte prend les proportions du dessin (centrée verticalement).
    const [x0, l0, haut, bas] = [680, LARGEUR - 64 - 680, 110, HAUTEUR - 70];
    const h0 = Math.min(bas - haut, ((l0 - 32) * image.height) / image.width + 32);
    const y0 = haut + (bas - haut - h0) / 2;
    ctx.save();
    ctx.shadowColor = "rgba(0,0,0,0.08)";
    ctx.shadowBlur = 18;
    ctx.fillStyle = COULEURS.carte;
    rectangleArrondi(ctx, x0, y0, l0, h0, 14);
    ctx.fill();
    ctx.restore();
    ctx.strokeStyle = COULEURS.bord;
    ctx.lineWidth = 1;
    rectangleArrondi(ctx, x0, y0, l0, h0, 14);
    ctx.stroke();
    const echelle = Math.min((l0 - 32) / image.width, (h0 - 32) / image.height);
    const [l, h] = [image.width * echelle, image.height * echelle];
    ctx.drawImage(image, x0 + (l0 - l) / 2, y0 + (h0 - h) / 2, l, h);
  }

  // Pied : note d'où vient la diapositive, barre de progression de la vidéo.
  const source = video.extraits.find((e) => e.id === d.source);
  if (source) {
    ctx.font = police(17, "normal", false);
    ctx.fillStyle = couleurDouce;
    ctx.fillText(uneLigne(ctx, `D'après tes notes : ${source.titreNote}`, LARGEUR - 260), 64, HAUTEUR - 30);
  }
  ctx.font = police(17, "normal", true);
  ctx.fillStyle = couleurDouce;
  ctx.textAlign = "right";
  ctx.fillText("Révisio", LARGEUR - 64, HAUTEUR - 30);
  ctx.textAlign = "left";
  ctx.fillStyle = titre ? "rgba(255,255,255,0.25)" : COULEURS.bord;
  ctx.fillRect(0, HAUTEUR - 6, LARGEUR, 6);
  ctx.fillStyle = titre ? COULEURS.blanc : COULEURS.accent;
  ctx.fillRect(0, HAUTEUR - 6, (LARGEUR * (segs[k].debut + t)) / total, 6);
}

/** Les schémas des diapositives, en images (un schéma illisible est simplement omis). */
async function chargerImages(diapos: Diapo[]): Promise<(ImageBitmap | null)[]> {
  return Promise.all(
    diapos.map(async (d) => {
      if (!d.schema) return null;
      try {
        const scene = await chargerScene(d.schema);
        return scene ? await createImageBitmap(await versPng(scene, 1200)) : null;
      } catch {
        return null;
      }
    }),
  );
}

// ---------- Encodage ----------

export interface Montage {
  octets: Uint8Array;
  format: "mp4" | "webm";
  dureeMs: number;
}

/** Monte la vidéo : `wav` = toute la narration (silences compris), `progression` reçoit une fraction de 0 à 1. */
export async function monterVideo(video: Video, wav: ArrayBuffer, progression: (fraction: number) => void): Promise<Montage> {
  const mb = await import("mediabunny");
  const segs = segments(video.diapos);
  const dureeMs = segs.at(-1)!.debut + segs.at(-1)!.duree;
  const qualiteVideo = new mb.Quality({ bitrate: 800_000 });
  const optionsVideo = { width: LARGEUR, height: HAUTEUR, quality: qualiteVideo };
  // L'encodeur AAC de Windows n'accepte que 96 à 192 kbit/s ; Opus est très bon dès 48 kbit/s.
  const audio = (debit: number) => ({ numberOfChannels: 1, sampleRate: FREQUENCE_AUDIO, quality: new mb.Quality({ bitrate: debit }) });

  type Choix = { format: "mp4" | "webm"; video: "avc" | "vp9" | "vp8"; audio: "aac" | "opus"; qualiteAudio: InstanceType<typeof mb.Quality> };
  let choix: Choix | null = null;
  if ((await mb.canEncodeVideo("avc", optionsVideo)) && (await mb.canEncodeAudio("aac", audio(96_000)))) {
    choix = { format: "mp4", video: "avc", audio: "aac", qualiteAudio: audio(96_000).quality };
  } else if (await mb.canEncodeAudio("opus", audio(48_000))) {
    for (const codec of ["vp9", "vp8"] as const) {
      if (!choix && (await mb.canEncodeVideo(codec, optionsVideo)))
        choix = { format: "webm", video: codec, audio: "opus", qualiteAudio: audio(48_000).quality };
    }
  }
  if (!choix) throw new Error("L'encodeur vidéo de WebView2 n'est pas disponible sur ce PC.");

  const images = await chargerImages(video.diapos);
  const contexteAudio = new OfflineAudioContext({ numberOfChannels: 1, length: 1, sampleRate: FREQUENCE_AUDIO });
  const narration = await contexteAudio.decodeAudioData(wav);
  const voix = narration.getChannelData(0);

  const canevas = document.createElement("canvas");
  canevas.width = LARGEUR;
  canevas.height = HAUTEUR;
  const ctx = canevas.getContext("2d")!;
  const sortie = new mb.Output({
    format: choix.format === "mp4" ? new mb.Mp4OutputFormat({ fastStart: "in-memory" }) : new mb.WebMOutputFormat(),
    target: new mb.BufferTarget(),
  });
  const pistes = {
    video: new mb.CanvasSource(canevas, { codec: choix.video, quality: qualiteVideo, keyFrameInterval: 5 }),
    audio: new mb.AudioBufferSource({ codec: choix.audio, quality: choix.qualiteAudio }),
  };
  sortie.addVideoTrack(pistes.video, { frameRate: IMAGES_PAR_SECONDE });
  sortie.addAudioTrack(pistes.audio);
  await sortie.start();

  const pas = 1000 / IMAGES_PAR_SECONDE;
  try {
    for (const [k, seg] of segs.entries()) {
      // Son de la diapositive, puis ses images : les deux pistes avancent ensemble.
      const [a, b] = [seg.debut, seg.debut + seg.duree].map((ms) => Math.min(voix.length, Math.round((ms * FREQUENCE_AUDIO) / 1000)));
      if (b > a) {
        const morceau = new AudioBuffer({ length: b - a, numberOfChannels: 1, sampleRate: FREQUENCE_AUDIO });
        morceau.copyToChannel(voix.subarray(a, b), 0);
        await pistes.audio.add(morceau);
      }
      for (let t = 0; t < seg.duree; t += pas) {
        dessinerDiapo(ctx, { video, k, t, segs, image: images[k] });
        await pistes.video.add((seg.debut + t) / 1000, Math.min(pas, seg.duree - t) / 1000, t === 0 ? { keyFrame: true } : undefined);
      }
      // `progression` peut lever une erreur pour arrêter le montage (bouton « Arrêter ») : les encodeurs sont alors libérés.
      progression((k + 1) / segs.length);
    }
  } catch (e) {
    await sortie.cancel().catch(() => {});
    throw e;
  } finally {
    images.forEach((i) => i?.close());
  }
  await sortie.finalize();
  const tampon = sortie.target.buffer;
  if (!tampon) throw new Error("La vidéo n'a pas pu être assemblée.");
  return { octets: new Uint8Array(tampon), format: choix.format, dureeMs };
}

/** Type MIME d'après le nom du fichier vidéo enregistré. */
export function typeVideo(fichier: string): string {
  return fichier.endsWith(".webm") ? "video/webm" : "video/mp4";
}
