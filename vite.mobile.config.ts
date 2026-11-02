// Web-app « Révisio sur téléphone » : mêmes écrans que l'app de bureau, mais les modules Tauri sont remplacés
// par des versions qui répondent depuis les données du téléphone (src/mobile). Compilée dans site/app/,
// publiée avec le site sur GitHub Pages (https://chimeraboop.github.io/Revisio/app/).
//   npm run build:mobile      compiler
//   npm run dev:mobile        essayer sur ce PC (http://localhost:1430/app.html)
import { defineConfig } from "vite";
import react from "@vitejs/plugin-react";
// @ts-expect-error pas de types Node dans ce projet
import { readFileSync } from "node:fs";
// @ts-expect-error pas de types Node dans ce projet
import { fileURLToPath } from "node:url";

const ici = (chemin: string) => fileURLToPath(new URL(chemin, import.meta.url));
const version = JSON.parse(readFileSync(ici("./package.json"), "utf8")).version;

export default defineConfig({
  plugins: [
    react(),
    {
      // La page s'appelle app.html à la racine du projet (à côté d'index.html, celle de l'app de bureau) :
      // publiée en index.html pour que l'adresse …/Revisio/app/ suffise.
      name: "page-index",
      enforce: "post",
      generateBundle(_options, bundle) {
        if (bundle["app.html"]) bundle["app.html"].fileName = "index.html";
      },
    },
  ],
  base: "./",
  publicDir: "mobile-public",
  define: { __VERSION__: JSON.stringify(version) },
  resolve: {
    alias: {
      "@tauri-apps/api/core": ici("./src/mobile/tauri-core.ts"),
      "@tauri-apps/api/event": ici("./src/mobile/tauri-event.ts"),
      "@tauri-apps/plugin-dialog": ici("./src/mobile/tauri-dialog.ts"),
    },
  },
  server: { port: 1430, strictPort: true },
  build: {
    outDir: "site/app",
    emptyOutDir: true,
    rollupOptions: { input: { index: ici("./app.html") } },
  },
});
