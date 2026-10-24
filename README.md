# Révisio

**Tes notes Obsidian deviennent des révisions.** Révisio est une application pour Windows (et Mac, en version expérimentale) qui lit ton vault Obsidian, en tire des cartes de révision et te prépare chaque jour une séance sur mesure. Tes photos de cours peuvent même devenir des notes.

Site : https://chimeraboop.github.io/Revisio/

## Télécharger

Dernière version : [page des versions](https://github.com/Chimeraboop/Revisio/releases/latest). Il faut un vault Obsidian (un dossier qui contient un sous-dossier `.obsidian`).

**Windows** : `Revisio_<version>_x64-setup.exe`, pour Windows 10 ou 11 (64 bits), installation sans droits administrateur. Windows SmartScreen peut avertir que l'installeur n'est pas signé : « Informations complémentaires », puis « Exécuter quand même ».

**Mac (expérimental)** : `Revisio_<version>_macOS-experimental.dmg`, pour macOS 13 ou plus (puce Apple ou Intel). Cette version est compilée et testée automatiquement sur un Mac de GitHub, mais n'a pas encore été essayée par une personne : n'hésite pas à signaler ce qui ne marche pas. Ouvre le `.dmg` et glisse Révisio dans Applications. L'app n'est pas signée par Apple, donc macOS la bloque au premier lancement : ouvre Réglages Système, Confidentialité et sécurité, puis clique sur « Ouvrir quand même » (ou, dans le Terminal : `xattr -dr com.apple.quarantine /Applications/Revisio.app`).

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
- **Tes données restent sur ton ordinateur**, dans `%APPDATA%\Revisio` (Windows) ou `~/Library/Application Support/Revisio` (Mac). Aucun serveur, aucun compte.
- **La clé Gemini** est rangée dans le Gestionnaire d'identification de Windows ou le Trousseau de macOS, jamais dans un fichier.
- **Hors ligne d'abord** : seuls l'IA en ligne, la lecture des photos et l'espace web ont besoin d'Internet.

Avec le niveau gratuit de Gemini, Google peut utiliser les extraits envoyés pour améliorer ses produits. Seuls des extraits de cours (et les photos, si tu utilises cette fonction) sont envoyés.

## Compiler depuis les sources

Prérequis : Node.js 20 ou plus et Rust (stable). Sous Windows, Visual Studio Build Tools (C++) et WebView2 ; sur Mac, les outils de développement Xcode.

```bash
npm install
npm run tauri dev
```

- Tests : `npm test` (interface) et `cargo test` dans `src-tauri` (cœur Rust, dont les tests de lecture seule du vault).
- Installeur Windows : `npx tauri build` produit `src-tauri/target/release/bundle/nsis/Revisio_<version>_x64-setup.exe`.
- Version Mac : `npx tauri build --target universal-apple-darwin --bundles app,dmg` sur un Mac. Le processus GitHub Actions « Mac » (`.github/workflows/mac.yml`) lance les tests et fabrique le `.dmg` ; il l'ajoute à la page d'une version si on lui donne son étiquette.
- Le site est dans `site/` (HTML, CSS et JavaScript, sans compilation) et se publie sur GitHub Pages à chaque envoi sur `main`.

Architecture : Tauri 2, cœur en Rust (`src-tauri/`, base SQLite avec recherche FTS5), interface React + TypeScript (`src/`).

## Licence

Code sous licence [MIT](LICENSE). Les polices incluses (Inter, Unbounded) sont sous licence SIL Open Font License 1.1 ; les dépendances gardent leurs propres licences (notamment Mediabunny, MPL-2.0).
