// Scène Excalidraw d'une note du vault (lue une seule fois par session).
import { api } from "../lib/api";
import { extraireScene, type Scene } from "./scene";

const scenes = new Map<string, Promise<Scene | null>>();

export function chargerScene(chemin: string): Promise<Scene | null> {
  if (!scenes.has(chemin)) scenes.set(chemin, api.note(chemin).then((n) => extraireScene(n.contenu)));
  return scenes.get(chemin)!;
}
