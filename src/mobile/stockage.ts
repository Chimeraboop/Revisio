// Stockage du téléphone : une petite base clé → valeur dans IndexedDB (comme un `shelve` en Python).
// Contenu reçu du PC, journal du téléphone, réglages locaux et connexion Google y sont gardés,
// pour que Révisio marche sans Internet.

const BASE = "revisio";
const TABLE = "valeurs";

let ouverture: Promise<IDBDatabase> | null = null;

function base(): Promise<IDBDatabase> {
  ouverture ??= new Promise((resolve, reject) => {
    const demande = indexedDB.open(BASE, 1);
    demande.onupgradeneeded = () => demande.result.createObjectStore(TABLE);
    demande.onsuccess = () => resolve(demande.result);
    demande.onerror = () => reject(demande.error);
  });
  return ouverture;
}

export async function lire<T>(cle: string): Promise<T | undefined> {
  const db = await base();
  return new Promise((resolve, reject) => {
    const d = db.transaction(TABLE, "readonly").objectStore(TABLE).get(cle);
    d.onsuccess = () => resolve(d.result as T | undefined);
    d.onerror = () => reject(d.error);
  });
}

export async function ecrire<T>(cle: string, valeur: T): Promise<void> {
  const db = await base();
  return new Promise((resolve, reject) => {
    const t = db.transaction(TABLE, "readwrite");
    t.objectStore(TABLE).put(valeur, cle);
    t.oncomplete = () => resolve();
    t.onerror = () => reject(t.error);
  });
}

export async function effacer(cle: string): Promise<void> {
  const db = await base();
  return new Promise((resolve, reject) => {
    const t = db.transaction(TABLE, "readwrite");
    t.objectStore(TABLE).delete(cle);
    t.oncomplete = () => resolve();
    t.onerror = () => reject(t.error);
  });
}

/** Demande au navigateur de ne pas effacer les données du site quand l'espace manque. */
export async function demanderPersistance(): Promise<void> {
  try {
    await navigator.storage?.persist?.();
  } catch {
    // Pas grave : les données sont aussi dans le Drive.
  }
}
