// Remplace `@tauri-apps/api/core` dans la web-app : les commandes sont traitées sur le téléphone.
import { repondre } from "./commandes";

export function invoke<T>(commande: string, args?: unknown): Promise<T> {
  return repondre<T>(commande, (args ?? {}) as Record<string, unknown>);
}
