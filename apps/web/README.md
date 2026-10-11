# apps/web

Client web (React + TypeScript + Vite), **mobile-first et responsive** : zones tactiles larges,
aucune interaction qui dépende du survol. Il appelle le moteur de règles compilé en WASM
(`crates/engine-wasm`) ; le serveur reste l'autorité sur la partie.

- `npm run dev` — serveur de développement
- `npm run build` — vérification des types puis build
- `npm test` — tests (Vitest + Testing Library)

Évolution prévue : PWA, puis application native (Capacitor) sans réécriture.
