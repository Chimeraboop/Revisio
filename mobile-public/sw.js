// Révisio sur téléphone : garde les fichiers de la web-app pour qu'elle s'ouvre sans Internet.
// La page est demandée au réseau d'abord (pour recevoir les mises à jour), sinon prise dans le cache ;
// les autres fichiers (noms changés à chaque version) sont pris dans le cache s'ils y sont.
// Les échanges avec Google (autre site) ne passent jamais par ici.
const CACHE = "revisio-app-v1";

self.addEventListener("install", () => self.skipWaiting());

self.addEventListener("activate", (e) => {
  e.waitUntil(
    caches
      .keys()
      .then((noms) => Promise.all(noms.filter((n) => n.startsWith("revisio-app-") && n !== CACHE).map((n) => caches.delete(n))))
      .then(() => self.clients.claim()),
  );
});

self.addEventListener("fetch", (e) => {
  const req = e.request;
  if (req.method !== "GET" || new URL(req.url).origin !== location.origin) return;
  if (req.mode === "navigate") {
    e.respondWith(
      fetch(req)
        .then((r) => {
          const copie = r.clone();
          caches.open(CACHE).then((c) => c.put("./", copie));
          return r;
        })
        .catch(() => caches.match("./").then((r) => r || caches.match(req))),
    );
    return;
  }
  e.respondWith(
    caches.match(req).then(
      (deja) =>
        deja ||
        fetch(req).then((r) => {
          if (r.ok) {
            const copie = r.clone();
            caches.open(CACHE).then((c) => c.put(req, copie));
          }
          return r;
        }),
    ),
  );
});
