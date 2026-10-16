# apps/web

Client web (React + TypeScript + Vite), **mobile-first et responsive** : zones tactiles larges,
aucune interaction qui dépende du survol. Il appelle le moteur de règles compilé en WASM
(`crates/engine-wasm`) ; le serveur reste l'autorité sur la partie.

- `npm run dev` — serveur de développement (pas de CSP : Vite y injecte un script en ligne)
- `npm run build` — vérification des types puis build (la CSP est injectée dans `index.html`)
- `npm run preview` — sert le build ; `npm run build && npm run preview -- --host` pour l'essayer sur un
  téléphone du réseau local (un build ne s'ouvre pas en `file://` : modules et `.wasm` ; hors réseau
  local, voir #54)
- `npm test` — tests (Vitest + Testing Library + `user-event`), dont un test de fumée contre le vrai `.wasm`

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
| `src/game/` | logique du client sans React ni WASM : la graine (`seed.ts`), les enregistrements de journal (`records.ts`), la politique de l'ordinateur (`computer.ts`), le contrôleur de la partie locale (`controller.ts`), le démarrage sans exception (`start.ts`) et le modèle d'écran (`screen-model.ts`) |
| `src/ui/` | composants et textes français : écrans (`Home`, `GameScreen`, `EndScreen`, `FatalScreen`), éléments (`HeroBar`, `TimeBand`, `Board`, `CardTile`, `Hand`, `ActionBar`, `ThinkingIndicator`, `ErrorBanner`), dialogues (`Dialog`, `ConfirmDialog`), textes (`fr.ts`, `errors.fr.ts`) et `consequence.ts` |
| `src/App.tsx` | assemble les écrans ; le seul à lire `data/starter` et `engine/load` |
| `src/data/starter.ts` | données `starter` importées en `?raw` |
| `vite/csp-plugin.ts` | plugin Vite de la CSP de build |

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

## Partie locale

Tout est dans `src/game/`, sans React ni `wasm/` ; le contrôleur ne connaît que l'interface
`GameSession`. Deux modes, tous deux complets et testés ici (les écrans ne font qu'afficher) :
`"computer"` (l'humain est le siège `One`, l'ordinateur joue `Two`) et `"two-camps"` (l'humain joue
les deux sièges, aucun pilote).

- **Contrôleur** (`GameController`) : `create(config, deps)` (crée la session, lit `startEvents` et les
  vues, lève `EngineError` si la création échoue), `start()`, `getSnapshot()` (immuable, nouvelle
  identité à chaque publication), `subscribe(fn)` (ces deux-là sont des propriétés fléchées, utilisables
  détachées : `useSyncExternalStore(controller.subscribe, controller.getSnapshot)`), `submit(aperçu)`, `concede()`, `clearError()`,
  `dispose()`. **L'appelant (`App`) le crée dans un gestionnaire d'événement, jamais dans un
  `useEffect`** : StrictMode rejoue les effets et une libération dans un cleanup tuerait l'instance du
  second montage. `dispose()` incrémente un jeton de génération (le pilote s'arrête à son prochain
  contrôle), vide les abonnés et libère la session une seule fois ; appelé depuis un abonné pendant `start()` ou
  `submit()`, il empêche le pilote de démarrer (aucun appel à la session après `dispose`). `startGame` (`start.ts`) valide la
  graine saisie, tire une graine avec `crypto` si elle est vide et ne lève jamais : sans
  `crypto.getRandomValues` il renvoie `entropy_unavailable` (l'écran propose alors la graine manuelle,
  jamais `Math.random`).
- **Phases** : `human-turn`, `computer-turn`, `over`, `fatal`. Elles se lisent dans la **vue** :
  `result !== null` = `over` (jamais déduit des événements), contre l'ordinateur `active !== "One"` =
  `computer-turn`. Le contrôleur ne calcule ni coût, ni cible, ni fin de partie : coût, passation de la
  main et fin de partie viennent de `legalActionsWithTime()`, appelé une seule fois par publication
  et seulement au tour d'un humain. L'humain peut ne pas ouvrir (miroir, égalité d'initiative tranchée
  par la graine) : aucun cas particulier.
- **Snapshot** : `phase`, `mode`, `humanSeat` (`null` en deux camps), `seed`, `aiSeed`, `dataVersion`,
  `players`, `views` (contre l'ordinateur : le siège `One` seul ; deux camps : les deux), `legal`
  (vide hors `human-turn`), `records`, `error`, `computerSteps`. `submit` n'accepte que **l'identité**
  d'un aperçu du snapshot courant : un aperçu périmé (double appui) donne `stale`, sans appel au moteur
  ni bandeau d'erreur ; hors tour, `not-your-turn`. Le moteur reste le juge : une erreur de règle
  (`refused`) laisse état, vues et `legal` inchangés et pose `snapshot.error`.
- **Ordinateur** (`computer.ts`) : `ComputerPolicy(actions, rng) => indice`, `randomPolicy` par défaut.
  Le pilote filtre `"concede"` avant la politique : **l'ordinateur ne reçoit jamais l'abandon** ; si
  seule `"concede"` reste, erreur `computer_stuck` (phase `fatal`, partie intacte), jamais un abandon
  silencieux. Au-delà de `MAX_COMPUTER_ACTIONS_PER_CHAIN` = 1000 actions de suite : `computer_chain_limit`,
  jamais une troncature. Le premier snapshot (`computer-turn`, 0 action) est publié avant la première
  action, suivi d'un `yield` ; ensuite un snapshot et un `yield` toutes les `COMPUTER_SLICE_MS` = 12 ms
  pour que le navigateur reste réactif (0,06 ms par action sous Node : garantie de non-gel, pas une
  nécessité). Un seul générateur `mulberry32(deriveAiSeed(seed))` pour toute la partie : même graine et
  mêmes actions humaines = même partie, ordinateur compris.
- **Enregistrements de journal** (`records.ts`, pur) : un `StepRecord` par pas (`seq` 0 = démarrage),
  avec les événements bruts, les noms et PV courants **d'après** l'action des cartes citées (lus dans
  les vues fournies) et les héros. Contre l'ordinateur le contrôleur ne demande que `view("One")` : un
  enregistrement ne peut donc pas contenir le nom d'une carte de la main de l'ordinateur, et le
  formateur du journal écrira « pioche une carte » **parce que le nom manque**, non parce qu'un `if`
  le décide. **Ce n'est pas une garantie de sécurité** : `legalActions()` pendant le tour de
  l'ordinateur et les événements `drew` exposent les identifiants de sa main, et la vue n'authentifie
  pas le siège. Le consommateur d'événements est un `switch` exhaustif avec une branche `never` qui lève :
  un événement inconnu est une erreur franche, jamais ignorée.
- **Session morte** : une erreur fatale (throw non JSON, une `RuntimeError` WASM par exemple) marque la
  session comme morte ; tous les appels suivants lèvent `internal_error` sans atteindre le WASM, et un
  `free()` qui lève ne masque jamais l'erreur d'origine. Le contrôleur passe en phase `fatal`.
- **Modèle d'écran** (`screen-model.ts`, pur) : `toScreenModel(snapshot)` range le haut et le bas.
  Contre l'ordinateur : l'humain en bas en permanence (main visible), l'ordinateur en haut (`hand:
  null`, seulement `handCount`), libellés « Toi — deck » / « Ordinateur — deck ». Deux camps : le camp
  actif en bas, mains des deux camps visibles, pioches en nombre seulement, libellés « Camp 1 — deck » /
  « Camp 2 — deck » (le numéro de camp distingue le miroir). `budget` et `active` viennent de la vue.
- **Tests** : `tests/support/policies.ts` porte `stallPolicy`, **de test seulement** : une politique
  d'usure (effet de héros, puis invocation, puis attaque d'une créature, en dernier attaque du héros ;
  première action après un tri stable, `concede` exclue). L'**épuisement n'arrive jamais au hasard**
  (240 parties aléatoires : toutes finissent par `hero_down`) ; avec `stallPolicy` jouée par les deux
  côtés, `rush` contre `bulwark`, graine 2, finit par `exhaustion` après 220 actions, en deux camps comme
  contre l'ordinateur (humain et ordinateur en `stallPolicy` rejouent exactement la même partie). Si les
  données `starter` changent, ce test doit échouer bruyamment : chercher alors une nouvelle graine dans
  une boucle bornée. Le décorateur de session (`tests/support/engine.ts`) compte les appels, injecte un
  aperçu illégal (filet de sécurité) ou force une vue.

## Interface

Contre l'ordinateur, jouable de bout en bout : accueil, partie, tour de l'ordinateur, fin de partie.
Il n'y a qu'un mode (pas de sélecteur) ; l'attaque (#66), le journal et les défausses (#65) et le mode
deux camps (#67) viennent ensuite.

- **`App`** (`src/App.tsx`) : charge le moteur pendant l'écran de chargement (« Lancer la partie » reste
  inactif ; échec : message français et « Recharger »), garde la `GameFactory`, puis assemble la session
  de façon **synchrone** (`createWasmSession` avec `CARDS_JSON`, `DECKS_JSON`, `DATA_VERSION`). Elle reçoit
  des dépendances par prop pour les tests (`App({ deps })` : `engine`, `crypto`, `policy`, `scheduler`,
  `now`), **sans mock du moteur** : les tests fournissent le vrai WASM, et une graine aléatoire se
  contrôle par un `crypto` factice (`null` simule l'absence d'entropie). `Home` reçoit les decks et la
  version par props : `ui/` n'importe ni `data/starter` ni `engine/load` (`tests/architecture.test.ts`).
- **Le contrôleur est créé dans le gestionnaire du clic**, jamais dans un `useEffect` (StrictMode
  rejoue les effets : un cleanup libérerait l'instance du second montage ; gardé par le test de montage StrictMode, qui ne couvre que le cleanup qui
  libère, et par le scan de source `tests/controller-creation-policy.test.ts` : aucun `startGame(` ni
  `GameController.create(` dans un `useEffect`/`useLayoutEffect` de `App` ou de `ui/`),
  par `startGame`, qui ne lève jamais. Sur succès, `App` garde le contrôleur et appelle `start()` ; elle
  appelle `dispose()` sur l'ancien avant d'en créer un autre, au retour à l'accueil et au démontage
  (N parties = N `free()`). « Rejouer » se reconstruit depuis le `Snapshot` : même graine
  (`String(snapshot.seed)`) ou nouvelle graine (texte vide, donc `crypto` rappelé).
- **Échecs de `startGame`** : `entropy_unavailable` (pas de `crypto.getRandomValues`) garde l'accueil, avec
  une alerte française (`fr.ts`), le focus sur le champ graine et aucune partie lancée (jamais `Math.random`
  ni graine 0) ; une graine saisie fonctionne sans entropie. `invalid_seed` : alerte liée au champ par
  `aria-describedby`. `engine` (données ou deck refusés) : retour à l'accueil avec le texte du code, jamais
  le message anglais.
- **Écran de partie** (`GameScreen`, reçoit le contrôleur en prop et lit `useSyncExternalStore(controller.subscribe,
  controller.getSnapshot)`) : de haut en bas, graine et « Infos de partie », héros adverse, plateau adverse,
  bandeau de temps et de jeton, plateau du joueur, héros du joueur, main, barre d'actions (effets de héros et
  « Abandonner » avec confirmation). Les boutons rendent au contrôleur **l'aperçu du snapshot courant**
  (jamais une action fabriquée) ; coût, « Passe la main », temps restant et fin de partie viennent de cet
  aperçu (`ui/consequence.ts`, pur : action simple, passation, attaque létale, passation par épuisement,
  abandon). Pendant le tour de l'ordinateur et après la fin, toutes les commandes sont inactives ; un seul
  résumé `aria-live` « L'ordinateur a joué N actions. À toi de jouer. » est annoncé à la **transition**
  `computer-turn` -> `human-turn` (`computerSteps` reste périmé ensuite), puis retiré dès que l'humain agit.
  La main de l'ordinateur n'existe pas dans la vue : seul son nombre est affiché.
- **Écrans spéciaux** : fin de partie (vainqueur, motif en français selon le perdant, graines, trois
  boutons, focus sur le titre) ; état fatal (phase `fatal` : message du code, « Rejouer avec la même graine »,
  « Retour à l'accueil », aucune commande) ; bandeau d'erreur `role="alert"` pour une erreur de règle
  (partie intacte, « Fermer »).
- **Accessibilité** : vrais `<button>` (zone tactile `--tap: 44px`), aucun `:hover`, provocation = pictogramme
  SVG `aria-hidden` **et** libellé « Provocation », riposte = « Riposte N », nom accessible complet des cartes
  (« Pillard, coût 2, force 3, coût d'attaque 1, PV 2, passe la main, provocation, riposte 1 »). `Dialog`
  (`role="dialog"`, `aria-modal`, piège à focus, Échap, focus rendu au déclencheur) n'utilise ni
  `<dialog>.showModal()` ni `inert` (absents de jsdom) ; il est générique et réutilisé par #65 et #66.
  Les noms de cartes, héros et decks sont rendus comme **texte React**, jamais comme HTML.
- **Aucun attribut `style=` ni `<style>`** : classes CSS seulement (`src/index.css`, mobile-first, un thème
  clair, polices système), vérifié par `tests/ui-policy.test.ts`. **CSP de build** (`vite/csp-plugin.ts`,
  `apply: "build"`) : `default-src 'none'; script-src 'self' 'wasm-unsafe-eval'; style-src 'self'; img-src
  'self' data:; connect-src 'self'; font-src 'self'; base-uri 'none'; form-action 'none'` ; `'wasm-unsafe-eval'`
  est requis pour instancier le WASM, jamais `'unsafe-eval'` ni `'unsafe-inline'`. Pas de CSP en dev. Les
  en-têtes HTTP (`frame-ancestors`, HSTS) relèvent du déploiement (#54). `tests/csp.test.ts` lance un vrai
  `build()` de Vite dans un dossier temporaire et lit l'`index.html` produit.
- **Tests de l'interface** : Testing Library + `@testing-library/user-event` 14.6.8 contre le vrai WASM
  (`tests/support/ui.tsx`, `tests/support/app.tsx`). Les fins de partie se testent en injectant des actions
  par le contrôleur ; `playUntil(controller, choose, predicate, maxSteps)` (`tests/support/policies.ts`,
  réutilisé par #65 à #67) cherche un scénario de façon bornée et échoue explicitement.

Évolution prévue : PWA, puis application native (Capacitor) sans réécriture.
