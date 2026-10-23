export type Theme = "systeme" | "clair" | "sombre";
export type Accent = "monochrome" | "bleu" | "violet" | "vert" | "orange" | "rose" | "turquoise";

/** Teintes de chaque accent : [thème clair, thème sombre]. Monochrome = noir en clair, blanc en sombre. */
export const ACCENTS: Record<Accent, { nom: string; clair: string; sombre: string }> = {
  monochrome: { nom: "Monochrome", clair: "#18181b", sombre: "#fafafa" },
  bleu: { nom: "Bleu", clair: "#2563eb", sombre: "#60a5fa" },
  violet: { nom: "Violet", clair: "#7c3aed", sombre: "#a78bfa" },
  vert: { nom: "Vert", clair: "#16a34a", sombre: "#4ade80" },
  orange: { nom: "Orange", clair: "#ea580c", sombre: "#fb923c" },
  rose: { nom: "Rose", clair: "#db2777", sombre: "#f472b6" },
  turquoise: { nom: "Turquoise", clair: "#0d9488", sombre: "#2dd4bf" },
};

const sombreSysteme = window.matchMedia?.("(prefers-color-scheme: dark)");
let actuel: { theme: Theme; accent: Accent } = { theme: "systeme", accent: "monochrome" };

function estSombre(theme: Theme) {
  return theme === "sombre" || (theme === "systeme" && !!sombreSysteme?.matches);
}

function appliquer() {
  const racine = document.documentElement;
  const { theme, accent } = actuel;
  if (theme === "systeme") racine.removeAttribute("data-theme");
  else racine.setAttribute("data-theme", theme === "sombre" ? "dark" : "light");
  // L'accent est posé directement sur <html> : clair ou sombre selon le thème réellement affiché.
  const sombre = estSombre(theme);
  const teinte = (ACCENTS[accent] ?? ACCENTS.monochrome)[sombre ? "sombre" : "clair"];
  racine.style.setProperty("--accent", teinte);
  racine.style.setProperty("--accent-texte", sombre ? "#0b0b0d" : "#ffffff");
  racine.dataset.accent = accent;
}

// Windows passe en clair / sombre : l'accent suit.
sombreSysteme?.addEventListener?.("change", () => actuel.theme === "systeme" && appliquer());

/** Applique le thème (`data-theme` sur <html>, couleurs dans app.css) et la couleur d'accent. */
export function appliquerTheme(theme: Theme, accent: Accent = actuel.accent) {
  actuel = { theme, accent };
  appliquer();
}
