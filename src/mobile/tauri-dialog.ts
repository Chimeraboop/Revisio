// Remplace `@tauri-apps/plugin-dialog` dans la web-app : pas de fenêtre de choix de fichier.
export async function open(): Promise<null> {
  return null;
}

export async function save(): Promise<null> {
  return null;
}
