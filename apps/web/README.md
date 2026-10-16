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

Évolution prévue : PWA, puis application native (Capacitor) sans réécriture.
