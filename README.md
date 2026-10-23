# Révisio

**Tes notes Obsidian deviennent des révisions.** Révisio est une application Windows qui lit ton vault Obsidian, en tire des cartes de révision et te prépare chaque jour une séance sur mesure. Tes photos de cours peuvent même devenir des notes.

Site : https://chimeraboop.github.io/Revisio/

## Télécharger

Dernière version : [page des versions](https://github.com/Chimeraboop/Revisio/releases/latest), fichier `Revisio_<version>_x64-setup.exe`.

- Windows 10 ou 11, 64 bits
- Installation sans droits administrateur
- Un vault Obsidian (un dossier qui contient un sous-dossier `.obsidian`)

Au premier lancement, un guide t'aide à choisir ton vault, à ajouter (si tu veux) une clé Gemini et à activer les photos de cours.

## Ce que fait Révisio

- **Cartes tirées de tes notes** : vocabulaire, textes à trous (mots en gras, callouts « Définition »), schémas Excalidraw muets, étapes à remettre dans l'ordre.
- **Répétition espacée FSRS** : chaque carte revient au bon moment. Tu indiques ta confiance avant de répondre ; les erreurs vont dans un carnet avec l'extrait du cours.
- **Planning d'évaluations** : ajoute la date d'un contrôle, Révisio répartit les chapitres jusqu'au jour J.
- **IA facultative** (Gemini avec ta propre clé, ou Ollama sur ton ordinateur) : questions qui citent tes cours, correction des réponses ouvertes, méthode Feynman, exercices type bac et examens blancs, cartes mentales, podcasts, vidéos.
- **Photos de cours en notes** : importe des photos ou laisse Révisio surveiller un dossier Google Drive ; il range de petites notes dans le bon chapitre, rédigées comme les tiennes.
- **Tableau de bord** : série, maîtrise par matière, points faibles, export de ta progression.

## Confidentialité

- **Lecture seule par défaut** : Révisio ne modifie pas ton vault tant que tu ne l'as pas autorisé (Réglages). Ensuite, chaque écriture est sauvegardée et peut être annulée, et une note existante n'est jamais modifiée par l'IA sans ton accord.
- **Tes données restent sur ton ordinateur**, dans `%APPDATA%\Revisio`. Aucun serveur, aucun compte.
- **La clé Gemini** est rangée dans le Gestionnaire d'identification de Windows, jamais dans un fichier.
- **Hors ligne d'abord** : seuls l'IA en ligne, la lecture des photos et l'espace web ont besoin d'Internet.

Avec le niveau gratuit de Gemini, Google peut utiliser les extraits envoyés pour améliorer ses produits. Seuls des extraits de cours (et les photos, si tu utilises cette fonction) sont envoyés.

## Compiler depuis les sources

Prérequis : Node.js 20 ou plus, Rust (stable, MSVC), Visual Studio Build Tools (C++), WebView2.

```bash
npm install
npm run tauri dev
```

- Tests : `npm test` (interface) et `cargo test` dans `src-tauri` (cœur Rust, dont les tests de lecture seule du vault).
- Installeur : `npx tauri build` produit `src-tauri/target/release/bundle/nsis/Revisio_<version>_x64-setup.exe`.
- Le site est dans `site/` (HTML, CSS et JavaScript, sans compilation) et se publie sur GitHub Pages à chaque envoi sur `main`.

Architecture : Tauri 2, cœur en Rust (`src-tauri/`, base SQLite avec recherche FTS5), interface React + TypeScript (`src/`).

## Licence

Code sous licence [MIT](LICENSE). Les polices incluses (Inter, Unbounded) sont sous licence SIL Open Font License 1.1 ; les dépendances gardent leurs propres licences (notamment Mediabunny, MPL-2.0).
