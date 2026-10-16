# apps/web

Client web (React + TypeScript + Vite), **mobile-first et responsive** : zones tactiles larges,
aucune interaction qui dépende du survol. Il appelle le moteur de règles compilé en WASM
(`crates/engine-wasm`) ; le serveur reste l'autorité sur la partie.

- `npm run dev` — serveur de développement
- `npm run build` — vérification des types puis build
- `npm test` — tests (Vitest + Testing Library), dont un test de fumée contre le vrai `.wasm`

## Moteur WASM

Le paquet JS/WASM est **généré, jamais versionné** : `src/wasm/` est ignoré par git. Les hooks npm
`predev`, `prebuild` et `pretest` lancent tous `node scripts/build-wasm.mjs`, l'unique script de
build : il lit la version de `wasm-bindgen` dans `Cargo.lock`, vérifie que `wasm-bindgen --version`
est **exactement** celle-là, compile `duel-engine-wasm` en `--release` puis génère `src/wasm/` avec
`wasm-bindgen --target web`. `tsc --noEmit` échoue sans ce dossier, d'où des hooks obligatoires.

Outils requis (en plus de `npm ci`) :

- Node de `.node-version` (24) ;
- Rust et la cible : `rustup target add wasm32-unknown-unknown` ;
- `wasm-bindgen-cli` à la version du lock : `cargo install wasm-bindgen-cli --version <version> --locked`
  (`node scripts/build-wasm.mjs --print-version` imprime `<version>` ; en cas d'écart le script échoue
  en affichant la commande exacte).

Dans l'application, le `.wasm` se charge de façon asynchrone (`import init, { Game } from "./wasm/duel_engine_wasm"`
puis `await init()`) ; dans les tests, de façon synchrone (`initSync({ module: octets })`), avec des
chemins en chaînes (`tests/support/paths.ts`) car `fs` n'accepte pas une URL sous jsdom. Une erreur du
binding est une `Error` dont `message` est la chaîne JSON `{"code","message"}`.

La CI (`.github/workflows/ci.yml`) rejoue cette chaîne sur chaque PR et chaque push vers `main`.

## Architecture du client

Trois couches, les dépendances ne vont que vers le bas (`tests/architecture.test.ts` lit `src/` comme
du texte et couvre aussi les fichiers futurs) : `engine/` <- `game/` <- `ui/`.

| Dossier | Contenu |
|---|---|
| `src/engine/` | contrat JSON typé (`types.ts`, écrit depuis les sources Rust), `GameSession` (`session.ts`, le seul accès au moteur), adaptateur `createWasmSession` (`wasm-session.ts`), chargeur mémoïsé `loadEngine` (`load.ts`), erreurs (`errors.ts`) |
| `src/game/` | logique du client sans React ni WASM : la graine (`seed.ts`) ; contrôleur et politique de l'ordinateur viendront ensuite |
| `src/ui/` | composants et textes français (`errors.fr.ts`) |
| `src/data/starter.ts` | données `starter` importées en `?raw` |

- **Seuls `engine/wasm-session.ts` (types) et `engine/load.ts` importent `src/wasm/`.** Le client ne
  fabrique jamais une action : il la retrouve dans `legalActionsWithTime()` et la renvoie telle quelle
  à `apply`. `dispose()` libère la partie une seule fois (après `free()` le binding lève « null pointer
  passed to rust ») ; toute autre méthode lève alors `SessionDisposedError` sans toucher au WASM.
- **Données** : `cards.json` et `decks.json` sont lus depuis `data/starter/1.0.0/` (jamais copiés).
  L'import hors de `apps/web` exige `server.fs.allow` dans `vite.config.ts` (sans lui, Vitest répond
  « Denied ID ») ; `allow` remplace la liste par défaut de Vite, donc elle contient `apps/web` et
  `data/`, jamais la racine du dépôt. `DATA_VERSION` (`starter@1.0.0`) est la seule constante de
  version ; `tests/data-version.test.ts` la compare au chemin importé.
- **Erreurs** : toute erreur est une `EngineError { code, message, fatal }` (`parseEngineError`) ; le
  `message` anglais n'est jamais affiché, la table `ui/errors.fr.ts` fait foi. **Une nouvelle erreur
  Rust casse `tests/error-codes.test.ts` : ajouter son texte français** (et son code dans
  `ENGINE_CODES`). Les erreurs de règle laissent l'état intact ; les codes `internal_error`,
  `serialization_failed`, `computer_stuck` et `computer_chain_limit` sont fatals.
- **Contrat JSON** : `EVENT_KINDS` doit égaler les variantes de `enum Event`
  (`crates/engine/src/rules.rs`) ; `tests/wasm.contract.test.ts` vérifie les clés exactes de chaque
  événement, action et aperçu sur 80 parties réelles. Ajouter une variante côté Rust casse ce test.
- **Graine** (`game/seed.ts`) : `parseSeed` (chiffres ASCII, 0 à 4294967295, le binding réduirait
  silencieusement une valeur hors `u32`), `randomSeed` (`crypto.getRandomValues`, seule entropie du
  client, jamais `Math.random`), `deriveAiSeed` (graine de l'ordinateur dérivée de celle de la partie) et
  `mulberry32`, figés par des valeurs d'or dans `tests/seed.test.ts`.
- **Sécurité** (`tests/security-policy.test.ts`) : aucun réseau, stockage ni `console` hors
  `console.error`, aucun HTML interprété (`innerHTML`, `eval`...) dans `src/` hors `src/wasm/`.
  `legalActions()`, `view(seat)` n'authentifient aucun siège : masquer la main de l'ordinateur est un
  choix d'affichage, jamais une garantie (ne jamais écrire « sécurisé » dans l'interface).

Évolution prévue : PWA, puis application native (Capacitor) sans réécriture.
