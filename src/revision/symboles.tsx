// Réponses qui contiennent des maths : exposants et indices Unicode tapés dans les notes ou les schémas
// (N₀, ²³⁵U, ᴬ⁻⁴Z₋₂Y, C₀×V₀) et formules LaTeX $…$. Les caractères Unicode d'exposant/indice manquent
// souvent dans les polices : on les affiche en vrais exposants (<sup>) et indices (<sub>) avec des
// caractères ordinaires, et les formules avec KaTeX.
import katex from "katex";
import { Fragment } from "react";

const EXPOSANTS: Record<string, string> = {
  "⁰": "0", "¹": "1", "²": "2", "³": "3", "⁴": "4", "⁵": "5", "⁶": "6", "⁷": "7", "⁸": "8", "⁹": "9",
  "⁺": "+", "⁻": "−", "⁼": "=", "⁽": "(", "⁾": ")", "ⁿ": "n", "ⁱ": "i",
  "ᴬ": "A", "ᴮ": "B", "ᴰ": "D", "ᴱ": "E", "ᴳ": "G", "ᴴ": "H", "ᴵ": "I", "ᴶ": "J", "ᴷ": "K", "ᴸ": "L",
  "ᴹ": "M", "ᴺ": "N", "ᴼ": "O", "ᴾ": "P", "ᴿ": "R", "ᵀ": "T", "ᵁ": "U", "ᵂ": "W",
  "ᵃ": "a", "ᵇ": "b", "ᶜ": "c", "ᵈ": "d", "ᵉ": "e", "ᶠ": "f", "ᵍ": "g", "ʰ": "h", "ʲ": "j", "ᵏ": "k",
  "ˡ": "l", "ᵐ": "m", "ᵒ": "o", "ᵖ": "p", "ʳ": "r", "ˢ": "s", "ᵗ": "t", "ᵘ": "u", "ᵛ": "v", "ʷ": "w",
  "ˣ": "x", "ʸ": "y", "ᶻ": "z",
};

const INDICES: Record<string, string> = {
  "₀": "0", "₁": "1", "₂": "2", "₃": "3", "₄": "4", "₅": "5", "₆": "6", "₇": "7", "₈": "8", "₉": "9",
  "₊": "+", "₋": "−", "₌": "=", "₍": "(", "₎": ")",
  "ₐ": "a", "ₑ": "e", "ₒ": "o", "ₓ": "x", "ₕ": "h", "ₖ": "k", "ₗ": "l", "ₘ": "m", "ₙ": "n", "ₚ": "p",
  "ₛ": "s", "ₜ": "t", "ᵢ": "i", "ⱼ": "j", "ᵣ": "r", "ᵤ": "u", "ᵥ": "v",
};

export type Morceau = { type: "texte" | "exposant" | "indice" | "formule"; texte: string };

/** Découpe un texte en morceaux : formules $…$, suites d'exposants, suites d'indices, texte ordinaire. */
export function decouper(texte: string): Morceau[] {
  const morceaux: Morceau[] = [];
  const ajouter = (type: Morceau["type"], t: string) => {
    const dernier = morceaux.at(-1);
    if (dernier && dernier.type === type && type !== "formule") dernier.texte += t;
    else morceaux.push({ type, texte: t });
  };
  for (const partie of texte.split(/(\$[^$\n]+\$)/g)) {
    if (!partie) continue;
    if (/^\$[^$\n]+\$$/.test(partie)) {
      ajouter("formule", partie.slice(1, -1));
      continue;
    }
    for (const c of partie) {
      if (EXPOSANTS[c]) ajouter("exposant", EXPOSANTS[c]);
      else if (INDICES[c]) ajouter("indice", INDICES[c]);
      else ajouter("texte", c);
    }
  }
  return morceaux;
}

const COMMANDES: Record<string, string> = {
  alpha: "α", beta: "β", gamma: "γ", delta: "δ", Delta: "Δ", epsilon: "ε", theta: "θ", lambda: "λ", mu: "µ",
  pi: "π", rho: "ρ", sigma: "σ", Sigma: "Σ", tau: "τ", phi: "φ", omega: "ω", Omega: "Ω",
  rightarrow: "→", to: "→", leftarrow: "←", Rightarrow: "⇒", Leftrightarrow: "⇔", longrightarrow: "⟶",
  times: "×", cdot: "·", div: "÷", pm: "±", leq: "≤", le: "≤", geq: "≥", ge: "≥", neq: "≠", ne: "≠",
  approx: "≈", infty: "∞", in: "∈", sqrt: "√", exists: "∃", forall: "∀", ldots: "…", dots: "…",
};

/**
 * Morceau de formule LaTeX coupé (extrait de recherche) → texte lisible :
 * `\text{He}` → He, `\alpha` → α, `\rightarrow` → →, `\mathbb{R}` → ℝ ; les `$` restants disparaissent.
 */
export function latexEnTexte(t: string): string {
  return t
    .replace(/\\mathbb\{R\}/g, "ℝ")
    .replace(/\\mathbb\{N\}/g, "ℕ")
    .replace(/\\mathbb\{Z\}/g, "ℤ")
    .replace(/\\frac\{([^{}]*)\}\{([^{}]*)\}/g, "$1/$2")
    .replace(/\\(?:text|mathrm|mathbf|operatorname)\{([^{}]*)\}/g, "$1")
    .replace(/\\([A-Za-z]+)/g, (tout, nom: string) => COMMANDES[nom] ?? (nom.length > 1 ? "" : tout))
    .replace(/\\[,;!: ]/g, " ")
    .replace(/[{}$]/g, "");
}

/** Affiche un texte de réponse avec ses exposants, indices et formules. */
export function TexteMaths({ texte }: { texte: string }) {
  return (
    <>
      {decouper(texte).map((m, i) =>
        m.type === "formule" ? (
          <span
            key={i}
            dangerouslySetInnerHTML={{ __html: katex.renderToString(m.texte, { throwOnError: false, output: "html" }) }}
          />
        ) : m.type === "exposant" ? (
          <sup key={i}>{m.texte}</sup>
        ) : m.type === "indice" ? (
          <sub key={i}>{m.texte}</sub>
        ) : (
          <Fragment key={i}>{m.texte}</Fragment>
        ),
      )}
    </>
  );
}
