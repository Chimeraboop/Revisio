// Texte des diapositives dessiné sur un canevas : pas de KaTeX possible, donc les formules LaTeX
// ($x^2$, $\frac{a}{b}$, $\Delta$…) et les exposants/indices Unicode (N₀, ²³⁵U) deviennent des « bouts »
// de texte normal, en exposant ou en indice, puis sont coupés en lignes.
import { decouper, latexEnTexte } from "../revision/symboles";

export type Position = "normal" | "exposant" | "indice";
export type Bout = { texte: string; position: Position };

/** Contenu du groupe `{…}` qui commence à `debut` (accolades équilibrées) et position après le groupe. */
function groupe(t: string, debut: number): [string, number] {
  if (t[debut] !== "{") {
    // Un seul caractère, ou une commande (\alpha).
    const commande = /^\\[A-Za-z]+/.exec(t.slice(debut));
    const fin = debut + (commande ? commande[0].length : 1);
    return [t.slice(debut, fin), fin];
  }
  let profondeur = 0;
  for (let i = debut; i < t.length; i++) {
    if (t[i] === "{") profondeur++;
    else if (t[i] === "}" && --profondeur === 0) return [t.slice(debut + 1, i), i + 1];
  }
  return [t.slice(debut + 1), t.length];
}

/** `\frac{a}{b}` → a/b (parenthèses si besoin) et `\sqrt{x}` → √x, accolades imbriquées comprises. */
function fractionsEtRacines(t: string): string {
  let sortie = "";
  let i = 0;
  while (i < t.length) {
    if (t.startsWith("\\frac", i) || t.startsWith("\\dfrac", i) || t.startsWith("\\tfrac", i)) {
      const debut = t.indexOf("frac", i) + 4;
      const [haut, apresHaut] = groupe(t, debut);
      const [bas, apresBas] = groupe(t, apresHaut);
      const entourer = (x: string) => {
        const simple = fractionsEtRacines(x);
        return /^[\w.,\\]+$|^\\[A-Za-z]+$/.test(simple) ? simple : `(${simple})`;
      };
      sortie += `${entourer(haut)}/${entourer(bas)}`;
      i = apresBas;
    } else if (t.startsWith("\\sqrt", i) && t[i + 5] === "{") {
      const [dedans, apres] = groupe(t, i + 5);
      const simple = fractionsEtRacines(dedans);
      sortie += /^[\w.,]+$/.test(simple) ? `√${simple}` : `√(${simple})`;
      i = apres;
    } else {
      sortie += t[i++];
    }
  }
  return sortie;
}

/** Formule LaTeX → bouts : `^` donne un exposant, `_` un indice ; le reste devient du texte lisible. */
export function formuleEnBouts(latex: string): Bout[] {
  const t = fractionsEtRacines(latex);
  const bouts: Bout[] = [];
  let normal = "";
  const vider = () => {
    if (normal) bouts.push({ texte: latexEnTexte(normal), position: "normal" });
    normal = "";
  };
  let i = 0;
  while (i < t.length) {
    const c = t[i];
    if ((c === "^" || c === "_") && i + 1 < t.length) {
      vider();
      const [contenu, apres] = groupe(t, i + 1);
      // Exposant d'exposant (rare au lycée) : aplati.
      bouts.push({ texte: latexEnTexte(contenu.replace(/[\^_]/g, "")), position: c === "^" ? "exposant" : "indice" });
      i = apres;
    } else {
      normal += c;
      i++;
    }
  }
  vider();
  return bouts;
}

/** Retire le Markdown que l'IA glisse parfois dans les points (**gras**, ==surligné==, `code`). */
export function sansMarkdown(t: string): string {
  return t.replace(/\*\*|__|==|`/g, "").replace(/\s+/g, " ").trim();
}

/** Texte d'un point de diapositive → bouts fusionnés. */
export function bouts(texte: string): Bout[] {
  const tous: Bout[] = [];
  for (const m of decouper(sansMarkdown(texte))) {
    if (m.type === "formule") tous.push(...formuleEnBouts(m.texte));
    else tous.push({ texte: m.texte, position: m.type === "texte" ? "normal" : m.type });
  }
  const fusion: Bout[] = [];
  for (const b of tous) {
    if (!b.texte) continue;
    const dernier = fusion.at(-1);
    if (dernier && dernier.position === b.position) dernier.texte += b.texte;
    else fusion.push({ ...b });
  }
  return fusion;
}

/**
 * Coupe les bouts en lignes de largeur ≤ `largeur`. On coupe aux espaces du texte normal ; un mot trop long
 * reste seul sur sa ligne. `mesurer` donne la largeur d'un bout (dépend de la police et de la position).
 */
export function couperEnLignes(liste: Bout[], largeur: number, mesurer: (b: Bout) => number): Bout[][] {
  // Mots : suites de bouts sans espace ; l'espace qui suit un mot est mémorisé pour la mesure.
  const mots: Bout[][] = [[]];
  for (const b of liste) {
    const morceaux = b.position === "normal" ? b.texte.split(/( )/) : [b.texte];
    for (const m of morceaux) {
      if (m === " ") mots.push([]);
      else if (m) mots.at(-1)!.push({ texte: m, position: b.position });
    }
  }
  const espace = mesurer({ texte: " ", position: "normal" });
  const largeurMot = (mot: Bout[]) => mot.reduce((s, b) => s + mesurer(b), 0);
  const lignes: Bout[][] = [];
  let ligne: Bout[] = [];
  let occupee = 0;
  for (const mot of mots.filter((m) => m.length > 0)) {
    const l = largeurMot(mot);
    if (ligne.length > 0 && occupee + espace + l > largeur) {
      lignes.push(ligne);
      ligne = [];
      occupee = 0;
    }
    if (ligne.length > 0) {
      ligne.push({ texte: " ", position: "normal" });
      occupee += espace;
    }
    ligne.push(...mot);
    occupee += l;
  }
  if (ligne.length > 0) lignes.push(ligne);
  return lignes;
}
