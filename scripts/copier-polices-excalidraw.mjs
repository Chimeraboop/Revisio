// Copie les polices d'Excalidraw dans public/ pour afficher les schémas hors ligne
// (sinon la bibliothèque les télécharge sur Internet). Lancé automatiquement avant
// `npm run dev` et `npm run build`.
// La police chinoise Xiaolai (12 Mo) est ignorée : inutile pour des notes en français.
import { cpSync, existsSync, mkdirSync, readdirSync, rmSync } from "node:fs";
import { join } from "node:path";

const source = join("node_modules", "@excalidraw", "excalidraw", "dist", "prod", "fonts");
const cible = join("public", "excalidraw", "fonts");
const IGNOREES = new Set(["Xiaolai"]);

if (!existsSync(source)) {
  console.error(`Polices Excalidraw introuvables (${source}) : lancer « npm install ».`);
  process.exit(1);
}
rmSync(cible, { recursive: true, force: true });
mkdirSync(cible, { recursive: true });
let n = 0;
for (const famille of readdirSync(source)) {
  if (IGNOREES.has(famille)) continue;
  cpSync(join(source, famille), join(cible, famille), { recursive: true });
  n++;
}
console.log(`Polices Excalidraw copiées (${n} familles) dans ${cible}`);
