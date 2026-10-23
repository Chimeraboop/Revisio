// Affichage d'un dessin Excalidraw en SVG, hors ligne.
// La bibliothèque (lourde) n'est chargée qu'au premier dessin affiché.
import { useEffect, useRef, useState } from "react";
import type { Scene } from "./scene";

declare global {
  interface Window {
    EXCALIDRAW_ASSET_PATH?: string | string[];
  }
}

let chargement: Promise<typeof import("@excalidraw/excalidraw")> | null = null;

function bibliotheque() {
  // Les polices sont copiées dans public/excalidraw/fonts (voir scripts/copier-polices-excalidraw.mjs) :
  // elles sont donc servies par l'app elle-même, sans Internet.
  window.EXCALIDRAW_ASSET_PATH = `${window.location.origin}/excalidraw/`;
  chargement ??= import("@excalidraw/excalidraw");
  return chargement;
}

export async function versSvg(scene: Scene): Promise<SVGSVGElement> {
  const { exportToSvg } = await bibliotheque();
  return exportToSvg({
    elements: scene.elements as never,
    appState: { exportBackground: true, viewBackgroundColor: "#ffffff", exportWithDarkMode: false },
    files: (scene.files ?? null) as never,
    exportPadding: 16,
  });
}

/** Le dessin en image PNG : côté le plus long ≤ `taille` pixels. */
export async function versPng(scene: Scene, taille = 1600): Promise<Blob> {
  const { exportToBlob } = await bibliotheque();
  return exportToBlob({
    elements: scene.elements as never,
    appState: { exportBackground: true, viewBackgroundColor: "#ffffff", exportWithDarkMode: false },
    files: (scene.files ?? null) as never,
    exportPadding: 16,
    maxWidthOrHeight: taille,
    mimeType: "image/png",
  });
}

/** Le dessin en PNG (base64, sans en-tête) pour l'envoyer à Gemini. */
export async function versPngBase64(scene: Scene, taille = 1600): Promise<string> {
  const blob = await versPng(scene, taille);
  const octets = new Uint8Array(await blob.arrayBuffer());
  let binaire = "";
  for (let i = 0; i < octets.length; i += 0x8000) binaire += String.fromCharCode(...octets.subarray(i, i + 0x8000));
  return btoa(binaire);
}

export default function Dessin({ scene, titre }: { scene: Scene | null; titre?: string }) {
  const conteneur = useRef<HTMLDivElement>(null);
  const [etat, setEtat] = useState<"chargement" | "ok" | "erreur">("chargement");

  useEffect(() => {
    let actif = true;
    if (!scene) {
      setEtat("erreur");
      return;
    }
    setEtat("chargement");
    versSvg(scene)
      .then((svg) => {
        if (!actif || !conteneur.current) return;
        svg.removeAttribute("width");
        svg.removeAttribute("height");
        svg.setAttribute("role", "img");
        if (titre) svg.setAttribute("aria-label", titre);
        conteneur.current.replaceChildren(svg);
        setEtat("ok");
      })
      .catch(() => actif && setEtat("erreur"));
    return () => {
      actif = false;
    };
  }, [scene, titre]);

  return (
    <div className="dessin">
      {etat === "chargement" && <span className="texte-doux petit">Dessin en cours d'affichage…</span>}
      {etat === "erreur" && <span className="message-erreur petit">Dessin illisible.</span>}
      <div ref={conteneur} className="dessin-svg" />
    </div>
  );
}
