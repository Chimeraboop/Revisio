// Différences ligne à ligne entre deux versions d'une note (modifications proposées par l'IA).

export type LigneDiff = { genre: "egale" | "ajoutee" | "retiree"; texte: string };

/** Différences ligne à ligne (plus longue sous-suite commune). */
export function diffLignes(avant: string, apres: string): LigneDiff[] {
  const a = avant.replace(/\r\n/g, "\n").split("\n");
  const b = apres.replace(/\r\n/g, "\n").split("\n");
  // Les notes font quelques centaines de lignes au plus : un tableau complet suffit.
  const n = a.length;
  const m = b.length;
  const lcs: number[][] = Array.from({ length: n + 1 }, () => new Array<number>(m + 1).fill(0));
  for (let i = n - 1; i >= 0; i--)
    for (let j = m - 1; j >= 0; j--) lcs[i][j] = a[i] === b[j] ? lcs[i + 1][j + 1] + 1 : Math.max(lcs[i + 1][j], lcs[i][j + 1]);
  const resultat: LigneDiff[] = [];
  let i = 0;
  let j = 0;
  while (i < n && j < m) {
    if (a[i] === b[j]) {
      resultat.push({ genre: "egale", texte: a[i] });
      i++;
      j++;
    } else if (lcs[i + 1][j] >= lcs[i][j + 1]) {
      resultat.push({ genre: "retiree", texte: a[i++] });
    } else {
      resultat.push({ genre: "ajoutee", texte: b[j++] });
    }
  }
  while (i < n) resultat.push({ genre: "retiree", texte: a[i++] });
  while (j < m) resultat.push({ genre: "ajoutee", texte: b[j++] });
  return resultat;
}

/** Morceau affiché : des lignes, ou un saut de lignes identiques replié. */
export type MorceauDiff = LigneDiff | { genre: "replie"; nombre: number };

/** Replie les longues suites de lignes identiques, en gardant `contexte` lignes autour des changements. */
export function replier(lignes: LigneDiff[], contexte = 2): MorceauDiff[] {
  const garder = lignes.map((l) => l.genre !== "egale");
  lignes.forEach((l, k) => {
    if (l.genre === "egale") return;
    for (let d = 1; d <= contexte; d++) {
      if (k - d >= 0) garder[k - d] = true;
      if (k + d < lignes.length) garder[k + d] = true;
    }
  });
  const morceaux: MorceauDiff[] = [];
  let caches = 0;
  lignes.forEach((l, k) => {
    if (garder[k]) {
      if (caches) morceaux.push({ genre: "replie", nombre: caches });
      caches = 0;
      morceaux.push(l);
    } else caches++;
  });
  if (caches) morceaux.push({ genre: "replie", nombre: caches });
  return morceaux;
}
