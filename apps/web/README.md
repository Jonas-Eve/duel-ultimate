# apps/web

Client web (React + TypeScript + Vite), **mobile-first et responsive** : zones tactiles larges,
aucune interaction qui dépende du survol. Il appelle le moteur de règles compilé en WASM
(`crates/engine-wasm`) ; le serveur reste l'autorité sur la partie.

- `npm run dev` — serveur de développement (pas de CSP : Vite y injecte un script en ligne)
- `npm run build` — vérification des types puis build (la CSP est injectée dans `index.html`)
- `npm run preview` — sert le build ; `npm run build && npm run preview -- --host` pour l'essayer sur un
  téléphone du réseau local (un build ne s'ouvre pas en `file://` : modules et `.wasm` ; hors réseau
  local, voir #54)
- **Sous-chemin (#80)** : `VITE_BASE` fixe le `base` de Vite au build (`vite/base.ts`, `resolveBase`).
  Absente ou vide : `/` (rien ne change : `dev`, `preview` et les e2e restent à la racine). Sinon la
  forme est stricte, `/` ou `/<segment>(/<segment>)*/` (`/duel-ultimate/`), segments `[A-Za-z0-9._-]+`
  hors `.` et `..` ; tout le reste (base relative `./`, URL absolue, `//`, espace, `?`, `#`, sans `/`
  initial ou final) **fait échouer** la config avec une erreur qui nomme `VITE_BASE`. Une base relative est
  refusée : le `.wasm` est résolu par `new URL(..., import.meta.url)`, que Vite émet sous la base
  absolue. La variable est lue par le processus Node de la config : pour essayer le build sous le
  sous-chemin, la donner aussi à `preview` :
  `VITE_BASE=/duel-ultimate/ npm run build && VITE_BASE=/duel-ultimate/ npm run preview`.
- **Identifiant de build (#80)** : `VITE_BUILD_SHA` (un SHA de commit, 7 à 40 caractères hexadécimaux
  minuscules) est affiché en 7 caractères sur l'accueil, sous « Données : starter@1.0.0 » :
  « Version : abc1234 ». Absent ou invalide (dev, tests, build local) : `dev`. `src/data/build.ts`
  (`buildIdFrom`, `BUILD_ID`) n'est lu que par `App`, qui le passe à l'accueil par la prop `buildId`.
  Seules les variables `VITE_` entrent dans le bundle ; aucun secret. `index.html` porte
  `<link rel="icon" href="data:," />` (pas de requête `/favicon.ico`, la CSP autorise `data:` en images).
- **Déploiement GitHub Pages (#54, #81)** : `.github/workflows/deploy-pages.yml` (« Deploy to Pages »)
  construit ce paquet avec `VITE_BASE=/duel-ultimate/` et `VITE_BUILD_SHA` (7 premiers caractères de
  `GITHUB_SHA`, dérivés en shell) puis publie `dist/` sur `https://jonas-eve.github.io/duel-ultimate/`.
  **Manuel** (`workflow_dispatch` seul : Actions > « Deploy to Pages » > Run workflow, sur `main`, depuis
  un `main` vert : il ne lance ni tests, ni Clippy, ni `fmt`, ni e2e). Pour vérifier la version en ligne,
  lire sur l'accueil « Version : <SHA à 7 caractères> » (à comparer à `git rev-parse --short origin/main`)
  et « Données : starter@1.0.0 ». **Cache** : Pages envoie `Cache-Control: max-age=600` (en-têtes non
  réglables) ; pendant ~10 minutes après un redéploiement, un `index.html` périmé peut référencer des assets
  hachés disparus et afficher l'écran d'erreur `wasm_load_failed` avec « Recharger » : attendre ou recharger.
  Le garde est le bloc « deploy-pages.yml » de `tests/ci-policy.test.ts` (déclencheur, permissions,
  épinglage des actions, aucun `${{ }}` dans un `run`) ; pour réajouter le déclencheur `push`, ajouter
  `push: branches: [main]` sous `on:` du workflow **et** adapter le garde du déclencheur. Action unique du
  propriétaire : Settings > Pages > Source « GitHub Actions » (règle de branche de l'environnement
  `github-pages` : autoriser `main`). Le sous-chemin n'est pas rejoué par les e2e : le test de sortie
  construite de #80 le couvre.
- `npm test` — tests (Vitest + Testing Library + `user-event`), dont un test de fumée contre le vrai `.wasm`
- `npm run e2e` — tests de bout en bout (Playwright) contre le build de production : voir « Tests de bout en bout »

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
| `src/ui/` | composants et textes français : écrans (`Home`, `GameScreen`, `EndScreen`, `FatalScreen`), éléments (`HeroBar`, `TimeBand`, `Board`, `CardTile`, `Hand`, `ActionBar`, `ThinkingIndicator`, `ErrorBanner`, `LogBar`, `LogHistory`), dialogues (`Dialog`, `ConfirmDialog`, `AttackSheet`, `LogDrawer`, `DiscardDialog`), textes (`fr.ts`, `errors.fr.ts`), `consequence.ts`, l'état du flux d'attaque `attack-state.ts` et le formateur du journal `log-format.ts` |
| `src/App.tsx` | assemble les écrans ; le seul à lire `data/starter`, `data/build` et `engine/load` |
| `src/data/starter.ts` | données `starter` importées en `?raw` |
| `src/data/build.ts` | `buildIdFrom`, `BUILD_ID` : identifiant de build (`VITE_BUILD_SHA`), lu par `App` seul |
| `vite/csp-plugin.ts` | plugin Vite de la CSP de build |
| `vite/base.ts` | `resolveBase` : le `base` de Vite depuis `VITE_BASE` (#80) |

- **Seuls `engine/wasm-session.ts` (types) et `engine/load.ts` importent `src/wasm/`.** Le client ne
  fabrique jamais une action : il la retrouve dans `legalActionsWithTime()` et la renvoie telle quelle
  à `apply`. `dispose()` libère la partie une seule fois (après `free()` le binding lève « null pointer
  passed to rust ») ; toute autre méthode lève alors `SessionDisposedError` sans toucher au WASM.
- **Données** : `cards.json` et `decks.json` sont lus depuis `data/starter/1.0.0/` (jamais copiés).
  L'import hors de `apps/web` exige `server.fs.allow` dans `vite.config.ts` (sans lui, Vitest répond
  « Denied ID ») ; `allow` remplace la liste par défaut de Vite, donc elle contient `apps/web` et
  `data/`, jamais la racine du dépôt (`data/build.ts`, lui, ne lit que `import.meta.env`). `DATA_VERSION` (`starter@1.0.0`) est la seule constante de
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

Jouable de bout en bout dans deux modes, choisis à l'accueil (groupe « Mode ») : contre l'ordinateur
(par défaut) et « Je joue les deux camps » (#67). Le journal et les défausses sont là (#65), l'attaque
aussi (#66).

### Mode deux camps

Pour reproduire à la main une situation précise que le hasard d'une IA ne produirait pas : une seule
personne joue les deux decks (« Deck du camp 1 » et « Deck du camp 2 » ; par défaut l'autre deck pour le
camp 2, le miroir est accepté ; le siège `One` est toujours le camp 1). Le mode ne change qu'une chose :
c'est l'humain qui choisit l'action des deux sièges (le contrôleur n'a pas de pilote). L'écran est le
même, alimenté par `toScreenModel` :

- le **camp qui a le jeton** est en bas (zone « joueur », mains et actions jouables) ; l'autre camp est en
  haut avec **sa main visible** (tuiles non jouables, légende « Main de … (non jouable) ») ; à la
  passation du jeton haut et bas s'échangent aussitôt, **sans écran de passage d'appareil** ; une
  annonce `aria-live` dit « Au tour de {camp} » et le focus passe à la région de partie (la tuile
  pressée a pu quitter la main) ;
- les camps sont nommés « Camp 1 — Ruée » / « Camp 2 — Rempart » (jamais « joueur 1/2 » seul) : bandeau
  central (« Au tour de … »), régions (« Camp 2 — Rempart : main »), dialogue d'abandon (« Le camp N
  abandonne ? »), écran de fin (« Camp 2 — Rempart a gagné. »), journal (tout nommé, y compris les
  pioches des deux camps, blocs titrés par camp) ; le miroir reste lisible par le numéro de camp ;
- **rien n'est masqué hors le contenu des pioches** (le moteur ne fournit que `deck_count`) : ne pas
  croire qu'un camp est « caché » à l'autre, ce n'est pas un mode sécurisé ; **aucune graine de
  l'ordinateur** (il n'y en a pas) : seule la graine de partie est affichée, pendant la partie et à la
  fin ; « Rejouer » (même graine ou nouvelle) rejoue le même mode et les mêmes decks (depuis le
  snapshot).

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
  SVG `aria-hidden` **et** libellé « Provocation », riposte = la deuxième des trois valeurs de la carte, nom
  accessible complet des cartes (« Pillard, coût 2, attaque 3, riposte 0, vie 2, provocation, passe la main »,
  voir « Carte compacte » ci-dessous). `Dialog`
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

## Journal et défausses

- **Formateur** (`ui/log-format.ts`, pur, sans React, textes dans `fr.ts`) : `formatRecords(records, ctx)`
  donne une `LogLine` française par événement (les `drew` consécutifs d'un joueur et `token_moved` +
  `active_player_changed` fusionnent) ; `formatBlocks` les groupe en blocs titrés (« Début de partie »,
  « Ton tour », « Tour de l'ordinateur », « Camp 1 — Ruée »). Le contexte vient du snapshot
  (`logContext(snapshot)` : mode, `youSeat = snapshot.humanSeat`, libellés tirés des noms de deck, jamais
  `SideModel.label`, qui dépend de la position haut/bas). 2e personne pour le siège humain (« Tu
  invoques Pillard »), 3e sinon. Clé de regroupement : `by` pour le démarrage et l'ordinateur,
  (`by`, `actor`) pour l'humain, car en deux camps `by` vaut toujours `human`. Un nom absent donne
  « une carte », jamais un identifiant.
- **Exhaustivité** : `switch` sur `eventKind` avec une branche `never` qui lève ; le test
  `tests/log-format.test.ts` lit `enum Event` de `crates/engine/src/rules.rs` et exige un `case` par
  variante, donc une variante Rust ajoutée casse la compilation puis le test.
- **Confidentialité = choix d'affichage** : contre l'ordinateur les enregistrements sont écrits d'après
  la seule vue du siège humain ; sa pioche s'affiche « L'ordinateur pioche 2 cartes. » **parce que le nom
  manque**. Pas de table globale id -> nom : elle nommerait une pioche après coup (l'écran reformate tout
  à chaque rendu, le test le rejoue deux fois). Ce n'est pas une garantie de sécurité. Les tests de
  parties complètes (80 parties, vrai WASM) lisent la vraie main de l'ordinateur sur la session interne
  par le décorateur de test, jamais par le contrôleur.
- **Régions vives** : `LogBar` (dernière ligne, toujours visible, tiroir ouvert ou non) est un
  `role="log"` `aria-live="polite"` `aria-atomic="false"`, mais `aria-live="off"` pendant le tour de
  l'ordinateur **et** tant que le résumé « L'ordinateur a joué N actions » est l'annonce en cours (une
  seule annonce de fin de chaîne, jamais deux) ; il redevient poli à l'action suivante. `LogDrawer` (bouton
  « Journal », `Dialog` titré « Journal ») et le journal complet de `EndScreen` sont des listes non vives.
- **Défausses** (`DiscardDialog`) : bouton « Défausse (N) » dans chaque `HeroBar`, les cartes dans
  l'ordre de mort (`view.discard`), « Détruite » au lieu de « PV 0 » (les PV initiaux ne sont pas dans la
  vue). Le journal et les défausses restent consultables pendant le tour de l'ordinateur.

## Flux d'attaque

- **État local, dérivé** : `GameScreen` ne stocke que l'identifiant de l'attaquant touché et la cible en
  attente de confirmation. `deriveAttackState(selected, snapshot.legal)` (`ui/attack-state.ts`, pur) en
  tire les aperçus d'attaque de cet attaquant (`isAttackBy` appliqué à `p.action`), `heroTargetable` et
  `creatureTargets`. Le contrôleur n'en sait rien.
- **Les cibles sont les aperçus du moteur** : l'interface ne construit aucune liste et **ne recalcule
  pas la provocation**. Face à un provocateur le moteur ne liste que les provocateurs, donc le héros et
  les autres créatures adverses n'ont aucun aperçu : leur tuile est `aria-disabled` avec la mention
  visible « Cible non disponible » (un appui ne fait rien, `apply` n'est pas appelé). L'interface ne dit
  pas pourquoi ; elle marque seulement les provocateurs (donnée de la carte).
- **Sélection** : un appui sur une de ses créatures qui a une attaque dans `legal` la sélectionne
  (`aria-pressed`, bordure épaisse et texte « Sélectionnée ») ; un nouvel appui, « Annuler la
  sélection » ou Échap la retire. Une créature sans attaque légale (tour de l'ordinateur) est
  `aria-disabled`. Les cibles possibles sont des `<button>` avec bordure et texte « Cible possible » ;
  le héros adverse porte un bouton cible dans son bandeau.
- **Carte compacte (#92)** : `CardTile` est une carte de jeu de taille fixe. L'élément externe (`.card
  .card-compact` : `<button>` ou `div role="group"`) contient `.card-face` (exactement `--card-w` = 62 px sur
  112 px, quel que soit le contenu : nom tronqué par `text-overflow: ellipsis`, marques sans effet sur la
  taille) puis `.card-tags` (sous la carte, dans l'élément interactif). Cadre commun à tous les decks par
  jetons `:root` (parchemin, bordure brune, filet doré décoratif). La face (`CardFace`, exporté, purement
  visuel) montre le nom, la pastille de coût **en main seulement** (sablier + chiffre, une seule fois, jamais
  sur un plateau), une illustration de substitution en SVG `aria-hidden`, et trois valeurs à chiffres de
  10 px au moins : attaque (étoile, `force`), riposte (flèche de retour, affichée même à 0), vie (cœur,
  PV courants). Sous la carte : « Provocation », la légende de main « Reste N » ou « Passe la main » (lue sur
  l'aperçu du moteur, jamais calculée ; le détail complet viendra avec #90) et la marque « Sélectionnée » /
  « Cible possible » / « Cible non disponible ». Nom accessible : « Nom, coût N (main seulement), attaque N,
  riposte N, vie N[, provocation][, passe la main | il te restera N de temps][, marque] » ; le coût d'attaque
  n'est plus sur la carte (il reste dans `AttackSheet`). Cinq cartes tiennent en main à 360 px (rangées :
  `gap: 2px`, `padding: 4px`). `.card` reste partagé avec la défausse (`DiscardDialog`, repris dans la tâche suivante de #87) : la taille fixe
  est sur le modificateur `.card-compact`. Pas de vie maximale dans `CardView` : aucun style « blessé » pour
  l'instant. Le helper de test `tests/support/card-label.ts` écrit le libellé attendu en toutes lettres.
- **Sélection périmée** : si plus aucun aperçu ne soutient l'attaquant (créature morte, tour de
  l'ordinateur) ou la cible en attente, l'état est remis à zéro **pendant le rendu** (jamais par un
  `useEffect`) ; elle ne ressuscite donc pas au tour suivant.
- **`AttackSheet`** (`Dialog` de #64) : « {attaquant} attaque {cible} » puis les lignes de
  `describeConsequence(aperçu)` (« Coût d'attaque : N », « Passe la main : l'adversaire aura N de temps »
  ou « Il te restera N de temps », « Met fin à la partie : ... »). Aucun dégât n'est annoncé : le client
  ne le connaît pas. « Annuler » (qui tient le focus, la réponse sûre) ferme la feuille et garde la
  sélection ; « Confirmer » appelle `controller.submit` avec **l'aperçu de cette cible**, efface la
  sélection et rend le focus à la région « Partie », quel que soit le `SubmitResult` (`stale` et
  `not-your-turn` sont ignorés sans bandeau ; `refused` arrive par `snapshot.error`).
- **Tests** : `attack-state.test.ts` (pur), `attack-flow.test.tsx` (vrai WASM, scénarios trouvés par
  recherche bornée : provocateur adverse, riposte du Loup hérissé, attaque létale, deux camps ; la
  session jumelle rejoue `probe.applied` pour comparer coût et passation). Vérifié aussi dans Chromium
  à 360 px (pas de défilement horizontal de la page, tous les boutons >= 44 px).

## Tests de bout en bout

`npm run e2e` (#68) : `pree2e` refait le build de production, puis Playwright lance Chromium sur
`vite preview --host 127.0.0.1 --port 4173 --strictPort` (jamais `0.0.0.0`, jamais le serveur de dev :
c'est le build qui porte la CSP et les `.wasm` hachés). Un worker, un seul projet `chromium`, 360 x 740
tactile par défaut (`apps/web/playwright.config.ts`), interactions par `tap()`. Les fichiers sont
`e2e/*.e2e.ts` (le suffixe évite que Vitest les ramasse) ; `e2e/support.ts` en tient les aides.

- **Rien n'est simulé** : vrai moteur WASM, vrai ordinateur, vrai build. Les tests lisent les libellés
  que le joueur voit (rôles, noms accessibles, textes français) et ne recalculent jamais un coût, une
  cible ou une règle : ils comparent une annonce lue avant une action à ce qu'on lit après.
- **Fichiers** : `computer.e2e.ts` (cas 1 à 3 : duel entier contre l'ordinateur, humain Rempart, temps
  annoncé et payé), `two-camps.e2e.ts` (cas 4 à 6 : provocation, riposte, deux camps, les trois fins),
  `layout.e2e.ts` (cas 7 et 8 : 360 px, CSP du build, 1280 px).
- **Graine fixe** : 42 (`SEED`, saisie dans le champ d'accueil), notée dans les tests. Une situation
  voulue (provocation, riposte) est **cherchée** par une boucle bornée (`playUntil`, au plus 40 à 60
  actions) plutôt que supposée ; si les données `starter` changent et que la graine ne la produit plus,
  le test échoue bruyamment en nommant la situation et la borne.
- **Robot d'usure** (cas « exhaustion ») : l'épuisement n'arrive jamais au hasard, donc un robot piloté
  par le DOM seul (aucun accès au moteur) joue les deux camps : effet de héros, puis première carte de
  la main, puis attaque d'une créature adverse (attaquant suivant si aucune cible créature), en dernier
  recours le héros ; jamais « Abandonner ». Ruée contre Rempart, graine 2 : 220 actions, environ 40 s
  (`timeout` de 180 s pour ce seul test, au plus 700 pas).
- **Sans bruit** : une fixture automatique (`guard`) échoue tout test dont la page a produit une
  violation de CSP (`securitypolicyviolation`), une erreur de console, une `pageerror` ou une requête
  hors de l'origine (S1 et S3 de #64). Chaque écran mobile passe `expectMobileSound` : pas de défilement
  horizontal de la page, toute cible tactile >= 44 x 44 px (un bouton radio est jugé par son libellé,
  le rond de 22 px n'est que le dessin), axe (`wcag2a`, `wcag2aa`, `wcag22aa`) sans violation.
- **Diagnostic sans artefact** : le workflow manuel n'archive rien. Sur échec, la fixture imprime
  `ariaSnapshot()` de `<main>` et la liste des messages de console et violations : tout est dans le
  journal du run.
- **Non atteignable par l'interface, donc absent ici** : le filet de sécurité « action refusée »
  (couvert en #63 et #64 par injection) et `entropy_unavailable` (Chromium a `crypto.getRandomValues`
  même hors contexte sécurisé ; couvert en #64 par un `crypto` factice). `eval` ne peut pas être sondé
  non plus (le code lancé par le protocole DevTools n'est pas soumis à sa restriction) : la politique de
  la balise `<meta>` est lue en toutes lettres.
- **Chromium** : en local `npx playwright install chromium` ; dans l'environnement des agents le
  Chromium 141 est préinstallé (`PLAYWRIGHT_BROWSERS_PATH=/opt/pw-browsers`), ne jamais y lancer
  `playwright install`. `@playwright/test` est donc épinglé à **1.56.1** (dérogation à « dernière version
  stable » du `CLAUDE.md` racine, validée par le propriétaire), `playwright-core` aussi (pair de
  `@axe-core/playwright` 4.14.0, qui sinon en installerait une seconde copie).
- **CI** : jamais dans la CI automatique (#78, décision du propriétaire, pour économiser le quota). Le
  workflow `.github/workflows/e2e.yml` (« E2E (manual) », `workflow_dispatch` seul) installe le WASM puis
  `npx playwright install --with-deps chromium` (CLI local, version du lock) et lance `npm run e2e` ; à
  démarrer depuis l'onglet Actions, de préférence avant toute fusion qui touche le flux de jeu.
- **Limite assumée** : le flux complet n'est plus couvert automatiquement, une régression peut passer
  jusqu'au prochain lancement manuel ; les tests Vitest (composants, contrôleur, moteur sur vrai WASM)
  restent automatiques.
