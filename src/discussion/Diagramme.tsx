// Diagramme écrit par l'IA en Mermaid, dessiné sur le PC (hors ligne). Mermaid n'est chargé qu'au premier
// diagramme affiché ; le dessin suit le thème clair / sombre de l'app. Le texte est nettoyé par Mermaid
// (niveau de sécurité « strict » : pas de liens cliquables ni de HTML).
import { useEffect, useId, useState } from "react";

type Mermaid = (typeof import("mermaid"))["default"];
let chargement: Promise<Mermaid> | null = null;

function charger(): Promise<Mermaid> {
  chargement ??= import("mermaid").then((m) => m.default);
  return chargement;
}

/** Le thème sombre est-il affiché ? (réglage de l'app, ou celui de Windows en mode « système ») */
function sombre(): boolean {
  return getComputedStyle(document.documentElement).colorScheme.includes("dark");
}

// Mermaid ne dessine qu'un diagramme à la fois : les rendus sont mis en file.
let file: Promise<unknown> = Promise.resolve();

export async function dessinerMermaid(code: string, id: string): Promise<string> {
  const mermaid = await charger();
  const travail = file.then(async () => {
    mermaid.initialize({
      startOnLoad: false,
      securityLevel: "strict",
      theme: sombre() ? "dark" : "neutral",
      fontFamily: '"Inter Variable", "Segoe UI", sans-serif',
    });
    const { svg } = await mermaid.render(id, code);
    return svg;
  });
  file = travail.catch(() => {});
  return travail;
}

export default function Diagramme({ code }: { code: string }) {
  const id = "mermaid-" + useId().replace(/[^a-zA-Z0-9]/g, "");
  const [svg, setSvg] = useState<string | null>(null);
  const [erreur, setErreur] = useState<string | null>(null);

  useEffect(() => {
    let actif = true;
    setErreur(null);
    dessinerMermaid(code, id)
      .then((s) => actif && setSvg(s))
      .catch((e) => {
        if (!actif) return;
        setSvg(null);
        setErreur(e instanceof Error ? e.message.split("\n")[0] : String(e));
        // Mermaid laisse parfois un élément d'erreur dans la page.
        document.getElementById("d" + id)?.remove();
      });
    return () => {
      actif = false;
    };
  }, [code, id]);

  if (erreur)
    return (
      <div className="visuel-erreur">
        <p className="message-erreur">Ce schéma contient une erreur et ne peut pas être dessiné ({erreur}).</p>
        <details className="petit">
          <summary className="lien-discret">Voir le texte du schéma</summary>
          <pre>{code}</pre>
        </details>
      </div>
    );
  if (!svg) return <p className="progression-texte">Dessin du schéma…</p>;
  return <div className="diagramme" dangerouslySetInnerHTML={{ __html: svg }} />;
}
