// Point d'entrée de la web-app « Révisio sur téléphone ».
import React from "react";
import ReactDOM from "react-dom/client";
import "@fontsource-variable/inter";
import "../styles/app.css";
import "./mobile.css";
import AppMobile from "./AppMobile";
import GardeFou from "../GardeFou";
import { demarrer } from "./magasin";

// Hors ligne : la page et ses fichiers sont gardés par le « service worker ».
if ("serviceWorker" in navigator && location.protocol === "https:") {
  navigator.serviceWorker.register("sw.js").catch(() => {});
}

// Sur un téléphone, l'éditeur de notes s'ouvre en mode « Écrire » (l'aperçu à côté serait trop étroit).
try {
  if (!localStorage.getItem("revisio-vue-editeur")) localStorage.setItem("revisio-vue-editeur", "ecrire");
} catch {
  // Stockage indisponible : l'éditeur garde son affichage par défaut.
}

void demarrer();

ReactDOM.createRoot(document.getElementById("root") as HTMLElement).render(
  <React.StrictMode>
    <GardeFou>
      <AppMobile />
    </GardeFou>
  </React.StrictMode>,
);
