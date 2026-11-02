// Google Drive depuis le téléphone : connexion (OAuth 2.0 pour application web, sans serveur) et fichiers
// du dossier caché propre à Révisio (`appDataFolder`, accès `drive.appdata` : rien d'autre n'est visible).
//
// Connexion : la page part chez Google puis revient ici avec un jeton d'accès dans l'adresse (#access_token=…),
// valable environ une heure. Pour le renouveler, on refait le même aller-retour avec `prompt=none` : Google
// répond aussitôt, sans rien demander, tant que l'élève est connecté à son compte dans ce navigateur.
import { ecrire, effacer, lire } from "./stockage";

export const CLIENT_ID = "703196424411-015qtogkj20g9bg1ej46pnjpb5gle7ig.apps.googleusercontent.com";
const PORTEE = "https://www.googleapis.com/auth/drive.appdata";
const DRIVE = "https://www.googleapis.com/drive/v3/files";
const ENVOI = "https://www.googleapis.com/upload/drive/v3/files";

interface Jeton {
  valeur: string;
  fin: number;
}

/** Adresse de retour : celle de l'app (sans paramètres), déclarée dans la console Google. */
function retour(): string {
  return location.origin + location.pathname;
}

/** Part chez Google pour se connecter (`silencieux` : sans rien demander si l'élève est déjà connecté). */
export function connecter(silencieux = false): void {
  const etat = crypto.getRandomValues(new Uint32Array(2)).join("-");
  sessionStorage.setItem("revisio-etat-oauth", etat);
  const p = new URLSearchParams({
    client_id: CLIENT_ID,
    redirect_uri: retour(),
    response_type: "token",
    scope: PORTEE,
    include_granted_scopes: "true",
    state: etat,
  });
  if (silencieux) p.set("prompt", "none");
  location.assign(`https://accounts.google.com/o/oauth2/v2/auth?${p}`);
}

/**
 * Au chargement : lit le jeton renvoyé par Google dans l'adresse, s'il y en a un, et nettoie l'adresse.
 * Renvoie un message si Google a refusé.
 */
export async function lireRetourGoogle(): Promise<string | null> {
  if (!location.hash.includes("access_token=") && !location.hash.includes("error=")) return null;
  const p = new URLSearchParams(location.hash.slice(1));
  history.replaceState(null, "", retour());
  const attendu = sessionStorage.getItem("revisio-etat-oauth");
  sessionStorage.removeItem("revisio-etat-oauth");
  if (p.get("error")) {
    const e = p.get("error");
    if (e === "interaction_required" || e === "login_required" || e === "consent_required") {
      await ecrire("google-connecte", false);
      return "Reconnecte ton compte Google pour synchroniser.";
    }
    return e === "access_denied" ? "Connexion refusée dans Google." : `Google a refusé la connexion (${e}).`;
  }
  if (!attendu || p.get("state") !== attendu) return "Réponse de Google inattendue : réessaie.";
  const jeton: Jeton = { valeur: p.get("access_token")!, fin: Date.now() + Number(p.get("expires_in") ?? 3600) * 1000 };
  await ecrire("google-jeton", jeton);
  await ecrire("google-connecte", true);
  return null;
}

/** Vrai si l'élève a déjà connecté son compte (même si le jeton a expiré). */
export async function estConnecte(): Promise<boolean> {
  return (await lire<boolean>("google-connecte")) === true;
}

/** Jeton encore valable au moins une minute, sinon `null` (il faut refaire l'aller-retour chez Google). */
export async function jetonValable(): Promise<string | null> {
  const j = await lire<Jeton>("google-jeton");
  return j && j.fin - 60_000 > Date.now() ? j.valeur : null;
}

export async function deconnecter(): Promise<void> {
  const j = await lire<Jeton>("google-jeton");
  if (j) {
    fetch(`https://oauth2.googleapis.com/revoke?token=${encodeURIComponent(j.valeur)}`, { method: "POST" }).catch(() => {});
  }
  await effacer("google-jeton");
  await ecrire("google-connecte", false);
}

export class JetonExpire extends Error {
  constructor() {
    super("La connexion à Google a expiré.");
  }
}

async function requete(url: string, init: RequestInit = {}): Promise<Response> {
  const jeton = await jetonValable();
  if (!jeton) throw new JetonExpire();
  let r: Response;
  try {
    r = await fetch(url, { ...init, headers: { ...(init.headers ?? {}), Authorization: `Bearer ${jeton}` } });
  } catch {
    throw new Error("Pas de connexion Internet : la synchronisation attendra.");
  }
  if (r.status === 401) {
    await effacer("google-jeton");
    throw new JetonExpire();
  }
  if (!r.ok) throw new Error(`Google Drive a refusé (${r.status}) : ${(await r.text()).slice(0, 200)}`);
  return r;
}

export interface FichierDrive {
  id: string;
  name: string;
  modifiedTime: string;
}

export async function lister(): Promise<FichierDrive[]> {
  const p = new URLSearchParams({ spaces: "appDataFolder", fields: "files(id,name,modifiedTime)", pageSize: "1000" });
  const r = await requete(`${DRIVE}?${p}`);
  return ((await r.json()) as { files?: FichierDrive[] }).files ?? [];
}

export async function telecharger(id: string): Promise<Uint8Array<ArrayBuffer>> {
  const r = await requete(`${DRIVE}/${encodeURIComponent(id)}?alt=media`);
  return new Uint8Array(await r.arrayBuffer());
}

/** Crée le fichier `nom` dans le dossier caché, ou remplace le contenu du fichier `existant`. */
export async function envoyer(nom: string, existant: string | null, octets: Uint8Array<ArrayBuffer>): Promise<void> {
  if (existant) {
    await requete(`${ENVOI}/${encodeURIComponent(existant)}?uploadType=media`, {
      method: "PATCH",
      headers: { "Content-Type": "application/gzip" },
      body: octets,
    });
    return;
  }
  const corps = new FormData();
  corps.append("metadata", new Blob([JSON.stringify({ name: nom, parents: ["appDataFolder"] })], { type: "application/json" }));
  corps.append("file", new Blob([octets], { type: "application/gzip" }));
  await requete(`${ENVOI}?uploadType=multipart`, { method: "POST", body: corps });
}

// ---------- Compression (fichiers .json.gz, comme ceux du PC) ----------

async function transformer(octets: Uint8Array<ArrayBuffer>, flux: CompressionStream | DecompressionStream): Promise<Uint8Array<ArrayBuffer>> {
  const sortie = new Blob([octets]).stream().pipeThrough(flux);
  return new Uint8Array(await new Response(sortie).arrayBuffer());
}

export async function compresserJson(valeur: unknown): Promise<Uint8Array<ArrayBuffer>> {
  return transformer(new TextEncoder().encode(JSON.stringify(valeur)), new CompressionStream("gzip"));
}

export async function decompresserJson<T>(octets: Uint8Array<ArrayBuffer>): Promise<T> {
  return JSON.parse(new TextDecoder().decode(await transformer(octets, new DecompressionStream("gzip")))) as T;
}
