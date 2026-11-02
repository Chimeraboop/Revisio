// Remplace `@tauri-apps/api/event` dans la web-app : aucun évènement ne vient d'un cœur Rust.
export type UnlistenFn = () => void;
export interface Event<T> {
  payload: T;
}

export async function listen<T>(_nom: string, _rappel: (e: Event<T>) => void): Promise<UnlistenFn> {
  return () => {};
}

export async function emit(): Promise<void> {}
