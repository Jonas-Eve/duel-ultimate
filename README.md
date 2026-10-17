# duel-ultimate

LCG (Living Card Game) en ligne, évolutif, que l'on fera grandir pour le rendre compétitif : des duels 1 contre 1 au tour par tour avec des decks préconstruits et équilibrés, sans booster ni collection. Chaque deck a des mécaniques réellement différentes ; le gameplay se travaille au fur et à mesure. Le moteur de règles est générique et indépendant du thème ; cartes, decks et extensions sont des données versionnées, et l'équilibrage est mesuré par un simulateur IA contre IA.

## Functional scope
See [`PROJECT-FUNCTIONAL-SCOPE.md`](./PROJECT-FUNCTIONAL-SCOPE.md).

## Architecture
<!-- PILOT:ARCHITECTURE:START -->
## Architecture

Monorepo : un workspace Cargo (cœur en Rust) et une application web TypeScript. Le moteur est
**pur et déterministe** (`état + action → nouvel état + événements`, hasard issu d'une graine
portée par l'état) : il n'a ni I/O, ni horloge, ni dépendance au réseau. Le **serveur est
l'autorité** sur la partie et filtre l'information cachée envoyée à chaque joueur ; le client
ne décide jamais d'un résultat. Parties **anonymes** (par lien), en mémoire, joueur contre
joueur ou contre IA (l'IA réutilise le moteur côté serveur).

| Composant | Rôle | Stack |
|---|---|---|
| `crates/engine` | Règles : un seul tour continu, une règle de temps unique, invocation (`Action::Play { card }`, la carte passe de la main au plateau), effets de héros (dont la pioche), attaque du héros adverse (`Action::Attack { attacker, target: Target::Hero }` : coût `attack_cost`, dégâts `force` ; PV à 0 → victoire `HeroDown` sans passation) ou d'une créature adverse (`target: Target::Creature(id)`, JSON `{"creature":7}` : elle perd `force` PV (`stats.health` est la santé *courante*), meurt à 0 et va dans la défausse de son propriétaire (`GameState::discard`, publique, dans l'ordre de mort ; `creature_health` pour le plateau) ; dégâts excédentaires perdus, jamais de `HeroDown` ; événements `creature_damaged` puis `died`, avant le changement de joueur actif ; toute cible qui n'est pas une créature du plateau adverse — créature propre, main, pioche, défausse, id inconnu — donne la même erreur `target_not_on_board`, après le contrôle de l'attaquant), **provocation** (tant que l'adversaire a une créature à `provocation` sur son plateau, une attaque ne peut viser que l'une d'elles, le héros n'est pas ciblable ; refus `target_forbidden_by_provocation`, distinct de `target_not_on_board`, évalué après l'attaquant et après la présence de la cible sur le plateau adverse, donc sans fuite sur les zones cachées ; filtre unique `provocation_allows` appelé par `validate` seul, recalculé sur l'état courant, sans cache ; seule la provocation de l'adversaire contraint, une créature qui provoque attaque librement ; une créature morte ne provoque plus), **riposte simultanée** (une créature attaquée inflige sa `riposte` à l'attaquant **en même temps** que celui-ci lui inflige sa `force`, y compris si elle meurt de cette attaque ; les deux dégâts sont calculés sur l'état d'avant l'attaque par `plan_damage` (fonction pure de `combat.rs`) puis appliqués par `apply_damage`, étape isolée qu'un futur effet d'ordre de dégâts modifiera seule ; le héros n'a pas de riposte, une attaque sur le héros n'en déclenche jamais ; riposte 0 : aucun événement ni effet ; excédent perdu, soustractions saturées ; événement additif `riposted { player, creature, amount }`, `player` = propriétaire de l'attaquant, `creature` = l'attaquant, `amount` = riposte nominale de la cible ; journal d'une attaque : `attacked`, `damaged` (héros) ou `creature_damaged` puis `riposted`?, `died`* (tous les morts des deux camps, ordre d'identifiant d'instance croissant, quel que soit le camp ou le rôle, l'attaquant mort comme la cible), `game_won` (`HeroDown`, seulement sur le héros : alors ni riposte ni passation) ou `token_moved`, `active_player_changed`?, `game_won` (`Exhaustion`)? ; le coût d'attaque est payé même si l'attaquant meurt de la riposte ; l'épuisement est évalué après chaque attaque, y compris quand la riposte tue la dernière créature d'un joueur qui reste actif), `Concede`, épuisement (une créature **vivante** peut toujours attaquer — un provocateur adverse s'il y en a, sinon le héros adverse —, donc garde son joueur en vie ; une créature morte a quitté le plateau et ne compte plus), `legal_actions` (ordre : cartes en main, activations d'effets, puis une attaque par couple (attaquant, cible) : par attaquant dans l'ordre d'invocation, le héros adverse d'abord, puis chaque créature adverse dans l'ordre du plateau, jamais une créature propre ni une créature morte, au plus plateau × (1 + plateau adverse) ; face à un provocateur, seuls les provocateurs adverses sont listés, dans l'ordre du plateau, sans le héros ; enfin `Concede` ; `apply` accepte exactement ces actions et `Concede`) ; terminaison : le potentiel Φ (PV des héros + par carte PV courants, +2 en pioche, +1 en main) décroît strictement à chaque action (la provocation ne fait que restreindre les actions légales ; la riposte ne retire que des PV finis ≥ 0 de plus à une créature en jeu, sans jamais rien ajouter), preuve dans `combat.rs` vérifiée par un test sur 250 parties au hasard ; les mécaniques des decks seront des briques composables, qu'un deck active ou non. `GameStarted` (premier événement de `new_game`) porte `data_versions`, copie opaque de `GameSetup.data_versions` (clé obligatoire du JSON de setup, jamais interprétée ni triée ; dernière clé de l'événement, absente du JSON quand la liste est vide, donc les journaux sans versions restent identiques octet pour octet) ; **Noms de cartes** : `PlayerSetup.card_names` (optionnel, absent = pas de noms, `skip_serializing_if` vide) est **parallèle** à `cards`, vide ou de longueur exactement `cards.len()`, chaque nom d'au plus `MAX_NAME_LEN` = 64 caractères ; `build_side` l'associe à la carte (`CardInstance.name`, `pub(crate)`) avant le mélange, donc le nom suit la carte dans toutes les zones ; présentation seulement : aucune règle, formule Φ ni tirage ne le lit, journaux d'événements identiques octet pour octet avec ou sans noms (`validate_side` : forme du deck d'abord, puis noms, puis stats). **Construction depuis les données** (`src/data.rs`, la seule implémentation, sans E/S ni panique) : `player_setup(deck, cards)` (héros, stats et noms via `resolve_deck_named`) et `game_setup_from_data(cards, decks, deck_one, deck_two, data_versions)` (cartes puis decks analysés par `duel-cards`, decks trouvés par identifiant strict, miroir permis, `data_versions` opaque), `DataError` aux codes `invalid_json` / `invalid_setup` / `unknown_deck`. **Vue par siège** (`src/view.rs`, pure, sans générateur ni E/S) : `player_view(&GameState, seat) -> PlayerView` (`PlayerView`, `OwnSide`, `OpponentSide`, `HeroView`, `CardView`, `GameResult`, `Serialize` seulement) ; deux types distincts, `OwnSide` (main du siège) et `OpponentSide` (aucun champ capable de porter la main adverse, seulement `hand_count`) rendent la fuite impossible par construction, les deux n'exposent la pioche qu'en `deck_count`, la vue ne contient ni générateur, ni graine, ni `data_versions` ; forme JSON figée (contrat du client) : clés en snake_case, sièges et `active` en `"One"`/`"Two"` (`PlayerId` n'a pas de `rename_all`, incohérence cosmétique assumée), `result` `null` ou `{"winner","reason"}` (`hero_down`/`exhaustion`/`concede`), `health` = PV courants (0 en défausse), `name` `null` sans noms ; la vue n'authentifie pas le siège (le serveur l'appelle avec le siège de la connexion). Limite connue : `legal_actions` et les événements `drew` exposent les identifiants de la main du joueur actif (acceptable en local, à rédiger côté serveur). **Observation** (`src/observation.rs`, pure, #140) : `Observation::of(&GameState) -> Option<Observation>` (`None` si la partie est finie), la fenêtre du siège actif pour une IA ; champs privés, ni `Deref`, ni accesseur vers l'état, ni `Clone` : la signature est la garantie, comme `OwnSide`/`OpponentSide` ; `seat()`, `actions()` (`legal_actions` sans `Concede`, même ordre, calculée une fois à la construction, éventuellement vide) et `view()` (`player_view` du siège, calculée à la demande) ; paresseuse, n'authentifie rien ; **Aperçu du temps** (`src/preview.rs`, pur) : `preview_legal_actions(&GameState) -> Vec<ActionPreview>`, de même longueur et dans le même ordre que `legal_actions`, par **essai à blanc** : chaque action légale est rejouée par `apply` sur un clone de l'état (l'état réel et le générateur ne sont jamais touchés ; `legal_actions`, `apply`, `time.rs` et `rules.rs` inchangés, la règle de temps reste unique) et le résumé est lu dans les événements produits, par un `match` exhaustif sur `Event` sans bras `_` (une nouvelle variante force une revue de l'aperçu). Cinq champs seulement, jamais le contenu des `drew` : `action`, `cost` (`cost` des événements `Played`/`EffectActivated`/`Attacked`, `null` pour `Concede`), `hands_over` (vrai si et seulement si `ActivePlayerChanged`), `budget_after` (`budget` de `TokenMoved`, du joueur qui garde la main ou de celui qui la reçoit, plafonné par son `max_time` ; `null` sans `TokenMoved`), `ends_game` (`GameResult` de `GameWon`, sinon `null`). Sémantique figée : une **attaque létale** sur le héros n'avance aucun temps et ne passe pas la main (`cost` = coût d'attaque affiché, `hands_over` faux, `budget_after` `null`, `ends_game` `hero_down`) ; une **passation** peut finir la partie (`hands_over` vrai **et** `ends_game` `exhaustion`) ; `Concede` : `cost` `null`, `ends_game` `concede` ; le coût est payé même si l'attaquant meurt de la riposte ; une pioche de plusieurs cartes ne change que `cost` et la passation. Un clone par action légale : au pire 931 actions (30 contre 30 créatures), quelques ms en release ; Dépend de `duel-cards` (revalide ses entrées) | Rust 2024, `rand_chacha` (graine) |
| `crates/cards` | Cartes, decks et extensions en **données JSON** versionnées, avec validation ; chemin de résolution unique `resolve_deck_named(deck, cards) -> (stats, noms)` (`resolve_deck` en est un mince appel : mêmes erreurs), `Card::name()`, `MAX_NAME_LEN`, `validate_card_names` ; `parse_art` (validation pure d'un `art.json` optionnel, présentation seulement) ; `roxmltree` en dev-dependency seulement (tests des SVG) | Rust, `serde`/`serde_json` |
| `crates/simulator` | Parties IA contre IA pour mesurer les taux de victoire entre decks (équilibrage) ; **bibliothèque `duel_simulator`** (`src/batch.rs` : `run_batch(contenders, config, policy) -> Report` et `run_batch_observed`, `play_game` (une partie, plafond d'actions, parties interrompues jamais cachées), `contenders_from(cards, decks)` et `setup_from(cards, decks, data_versions)` purs, sans E/S, qui renvoient `Err(String)` au lieu de paniquer et appellent le noyau `player_setup` d'`engine` (aucune résolution de deck propre au simulateur) ; elle ne lit jamais le disque) et binaire fin (`src/main.rs`) ; jeux de données **embarqués à la compilation par le binaire seul** (`include_str!`, liste fermée `fixture` et `starter`, jamais de chemin ni de lecture disque) choisis par `--set <nom>[@<version>]` (défaut `starter`, dernière version) ; **matchups** : chaque paire croisée `i < j` puis chaque miroir (avec les 2 decks de départ : `rush vs bulwark`, `rush (mirror)`, `bulwark (mirror)`), `N` parties **par matchup**, sièges alternés dans un matchup croisé (le deck A au siège One aux indices pairs), victoires comptées par deck ; **graines** : une graine de lot (`--seed`, défaut 0) dérivée par `derive_seed` (finaliseur SplitMix64, défini dans `duel-ai` et ré-exporté par le simulateur avec `policy_rng` et les tags de politique) en graine de partie, le moteur la reçoit telle quelle, la politique reçoit un RNG distinct par siège, aucun hasard hors de la graine de lot (même lot, même rapport) ; **politique** : `Policy`, trait de `duel-ai` ré-exporté (`fn choose(&self, &Observation, &mut ChaCha8Rng) -> usize`, toute fonction ou fermeture de cette forme en est une, passée en `&dyn Policy` à `run_batch`, `run_batch_observed` et `play_game`), ne voit que l'`Observation` du siège actif (actions légales hors `Concede`, jamais l'état ; `play_game` en construit une par pas) ; `random_policy` (au hasard, attaques sur héros et sur créatures) est la politique par défaut ; sortie : exactement `Report::render()` (voir « Format du rapport du simulateur »), argument invalide : message sur stderr et code 2 ; aucun objectif d'équilibre sur `starter` (T4) | Rust |
| `crates/ai` | IA du duel (`duel-ai`, #140) : le trait `Policy` (`fn choose(&self, obs: &Observation, rng: &mut ChaCha8Rng) -> usize`, impl pour toute fonction ou fermeture de cette forme, générateur concret `ChaCha8Rng`, distinct de celui du moteur), `random_policy` (`src/policies/random.rs`, même appel `random_range(0..len)` que l'ancienne politique du simulateur), les graines `derive_seed`, `policy_rng`, `TAG_POLICY_ONE`/`TAG_POLICY_TWO` (déplacés du simulateur, valeurs inchangées, tests d'or) et `TAG_COMPUTER` (réservé au binding, #142), et le harnais `choose(&GameState, &dyn Policy, &mut ChaCha8Rng) -> Result<Action, AiError>` (`NoPlayableAction`, `IndexOutOfRange`, jamais de panique, jamais `Concede`) ; une politique ne reçoit jamais l'état, `tests/policies_isolation.rs` interdit `GameState`, `apply`, `legal_actions` et `preview_legal_actions` dans `src/policies/` ; aucune règle de jeu, aucune E/S, aucune horloge ; dépend de `duel-engine`, `rand` (sans features par défaut, jamais `getrandom`) et `rand_chacha`, `duel-engine` ne dépend jamais de lui ; hors de l'arbre WASM tant que le binding ne le branche pas | Rust |
| `crates/server` | Parties en ligne (HTTP + WebSocket) | Rust, `axum` 0.8, `tokio` |
| `crates/engine-wasm` | Binding WASM du moteur pour le client (mêmes règles que le serveur) : constructeur depuis un JSON de setup, construction depuis les données `Game.from_data(cards_json, decks_json, deck_one, deck_two, data_versions_json, seed)` (méthode statique ; decks choisis par identifiant ; chacune des cinq chaînes plafonnée à 1 MiB avant toute analyse ; `data_versions_json` est un tableau JSON de chaînes ; nouveau code d'erreur `unknown_deck`), `legal_actions()` et `apply(action)` en JSON, erreurs `{"code","message"}`, entrée limitée à 1 MiB, accesseur `start_events()` des seuls événements de démarrage (`game_started`, éventuellement `game_won` par épuisement ; publics, identiques à chaque appel), toujours aucun accesseur d'état complet, mais une **vue par siège** `Game.view(seat)` (JSON ; `seat` exactement `"One"` ou `"Two"`, sinon erreur `invalid_seat`, jamais de panique ni de normalisation de casse ; `seat` aussi plafonné à 1 MiB) : projection pure `player_view` du moteur, qui donne le contenu de la main de ce siège seulement, la main adverse et les deux pioches en nombres (`hand_count`, `deck_count`), les plateaux (PV courants, noms) et les défausses publiques dans l'ordre de mort, le héros (PV courants, `max_time`, effets), `active`, `budget` et `result` ; la défausse n'est donc plus à reconstruire à partir des événements `died`. **La vue n'authentifie pas le siège** : `view` est un adaptateur local de confiance (en mode ordinateur le client ne demande que la vue du siège humain, le binding ne l'impose pas) ; le futur serveur appellera `player_view` lui-même avec le siège de la connexion et n'exposera jamais `Game.view`. **Limite connue, non résolue** : `legal_actions()` et les événements `drew` exposent toujours les identifiants d'instance de la main du joueur actif (donc de l'ordinateur), qui suivent l'ordre du deck avant mélange ; acceptable en local seulement, le serveur devra rédiger événements et actions. **Aperçu du temps** `Game.legal_actions_with_time()` (JSON, sans paramètre ; `Duel::legal_actions_with_time_json`) : les mêmes actions légales que `legal_actions()`, dans le même ordre, chacune `{"action","cost","hands_over","budget_after","ends_game"}` (coût en temps, passation de la main `hands_over` **à lire avec** `ends_game` : une attaque létale finit la partie sans passer la main, une passation peut finir la partie par épuisement ; budget après l'action ; fin de partie `{"winner","reason"}` ou `null`), obtenus par `duel_engine::preview_legal_actions`, un essai à blanc sur clone qui ne révèle jamais ce qui est pioché ; tableau vide quand la partie est finie ; `action` se renvoie telle quelle à `apply` ; `legal_actions`/`apply` inchangés, aucun nouveau code d'erreur | Rust, `wasm-bindgen` |
| `apps/web` | Client **mobile-first et responsive**, jouable dans le navigateur (moteur en WASM) contre l'ordinateur ou en jouant les deux camps : accueil (mode, deck(s), graine, version), partie (héros, plateaux, temps et jeton, main, effets de héros, abandon avec confirmation), tour de l'ordinateur, bandeau d'erreur, journal lisible (dernière ligne en région vive, historique en tiroir, formateur pur gardé contre `enum Event` de `rules.rs`), défausses publiques des deux camps, écran de fin avec journal complet et « Rejouer » ; attaque (je touche une de mes créatures, ses cibles légales sont celles des aperçus du moteur, confirmation du coût avant de jouer : provocation et riposte testables à la main) ; mode deux camps (#67) : le camp qui a le jeton est en bas, les deux mains sont visibles, haut et bas s'échangent à la passation (annonce « Au tour de … », aucun écran de passage d'appareil), rien n'est masqué hors le contenu des pioches (nombres seulement), ni graine de l'ordinateur (PWA ensuite, appli native Capacitor plus tard) | React 19, TypeScript, Vite |

Règles propres à ce repo :
- **Une carte est une donnée, pas du code** ; une extension est un dossier versionné, et chaque
  partie enregistre les versions utilisées (parties rejouables, équilibrage traçable).
- **Aucun hasard hors de la graine du moteur** : toute règle qui tire au sort passe par
  `GameState::random_below`.
- **Pas d'entropie système, y compris dans le binding WASM** : `rand` est déclaré au niveau du
  workspace **sans features par défaut** (`default-features = false`) ; `rand::rng()`, `StdRng` et
  `OsRng` ne compilent donc pas, et `getrandom` n'est jamais tiré. Ne jamais activer
  `getrandom/wasm_js` : la CI échoue si `getrandom` apparaît dans l'arbre de `duel-engine-wasm`.
- **Paquet WASM du client** : généré, jamais versionné. `apps/web/scripts/build-wasm.mjs` (le seul
  script, appelé par les hooks npm `predev`, `prebuild` et `pretest`) compile `duel-engine-wasm` en
  `--release` puis lance `wasm-bindgen --target web` vers `apps/web/src/wasm/` (ignoré par git). La
  version attendue du CLI est celle de l'entrée `wasm-bindgen` de `Cargo.lock`, comparée par égalité
  stricte ; sinon le script échoue avec la commande d'installation exacte. `--print-version` imprime
  cette version (la CI s'en sert). Le contrat d'erreur du binding : une `Error` JS dont `message` est
  la chaîne JSON `{"code","message"}`.
- **Client web** : aucune règle de jeu en TypeScript ; la version de données est une constante
  unique (`starter@1.1.0`) épinglée par test contre le chemin importé ; le `base` de Vite vient de `VITE_BASE` (défaut `/`, forme `/<segment>/` validée strictement, `preview` à lancer avec la même variable que le build) et l'accueil affiche « Version : <id> » (7 premiers caractères de `VITE_BUILD_SHA`, `dev` sinon, `src/data/build.ts` lu par `App` seul) ; l'art optionnel d'un set (`art.json`, `art/`) est lu par `src/data/art.ts` (`App` seul, `ui/` n'importe rien de `data/`) en une recherche par nom d'affichage exact, la carte compacte, le détail et les défausses affichent l'image du set quand elle existe (décorative, `alt=""`), sinon le paysage de substitution ; la table d'erreurs françaises
  est gardée par un test contre les sources Rust (un nouveau code d'erreur Rust casse ce test tant que
  son texte français manque) ; contre l'ordinateur, la confidentialité de sa main n'est qu'un choix
  d'affichage local. Client : le contrôleur ne calcule ni coût, ni cible, ni fin de partie ; l'ordinateur ne reçoit jamais l'abandon ; le journal contre l'ordinateur est écrit d'après la seule vue du siège humain. Interface : `App` assemble (seul lecteur des données `starter` et du chargeur du moteur), crée le contrôleur dans le gestionnaire du clic, jamais dans un `useEffect` (gardé par un test de lecture de source), et libère l'ancien avant d'en créer un autre ; le coût, la passation de la main et la fin de partie affichés viennent de l'aperçu du moteur ; aucun attribut `style=` (classes CSS) ; CSP injectée au build seulement. Détails dans `apps/web/README.md`.
- **Tests de bout en bout** (`apps/web/e2e/*.e2e.ts`, Playwright, #68) : ils jouent le **build de
  production** (CSP, `.wasm` hachés) servi par `vite preview` sur `127.0.0.1:4173`, avec le vrai
  moteur et le vrai ordinateur, sans rien simuler ; ils lisent les libellés du DOM et ne recalculent
  jamais un coût ni une règle. **Dérogation à « dernière version stable » (§3), consignée et validée
  par le propriétaire** : `@playwright/test` est épinglé à **1.56.1** (avec `playwright-core` 1.56.1,
  pair de `@axe-core/playwright`), la dernière version exécutable avec le Chromium 141 préinstallé
  dans les environnements d'agents (`/opt/pw-browsers`, aucun téléchargement possible) : chaque version
  de Playwright exige une révision précise de Chromium. Dans le workflow manuel, `npx playwright install` à la version du
  lock installe ce même Chromium 141 : un e2e identique partout. **Remonter la version quand
  `/opt/pw-browsers` est rafraîchi** (ticket `type:tech` à ce moment-là).
- **CI** (`.github/workflows/ci.yml`, sur `pull_request` et `push` vers `main`) : `fmt`, `clippy`,
  tests Rust, build WASM, contrôle de non-entropie, puis `npm ci`, `npm run build` et `npm test`
  dans `apps/web` ; **elle ne lance plus les tests de bout en bout** (#78, décision du propriétaire,
  pour économiser le quota de CI). Ceux-ci tournent à la demande par `.github/workflows/e2e.yml`
  (« E2E (manual) », `workflow_dispatch` seul, jamais `pull_request` ni `push`), à lancer depuis
  l'onglet Actions : la CI automatique ne couvre donc plus le flux complet (Vitest, lui, reste
  automatique). Permissions `contents: read`, actions épinglées par SHA, aucun secret, aucun artefact ;
  ces deux workflows sont gardés par `apps/web/tests/ci-policy.test.ts`.
- **Déploiement sur GitHub Pages** (`.github/workflows/deploy-pages.yml`, « Deploy to Pages », #54/#81) : le
  client est publié sur <https://jonas-eve.github.io/duel-ultimate/>. Le déclenchement est **manuel**
  (onglet Actions > « Deploy to Pages » > Run workflow, sur `main`), pour économiser le quota de CI : à lancer
  **depuis un `main` vert**, car ce workflow ne lance ni tests, ni Clippy, ni `fmt`, ni e2e. Il construit
  `apps/web` avec `VITE_BASE=/duel-ultimate/` (voir `apps/web/README.md`) et le SHA court du commit. Pour
  vérifier la version en ligne, le pied de l'écran d'accueil affiche « Version : <SHA à 7 caractères> » (à
  comparer à `git rev-parse --short origin/main`) et « Données : starter@1.1.0 ». **Cache** : Pages envoie
  `Cache-Control: max-age=600` et on n'y règle pas les en-têtes ; pendant ~10 minutes après un redéploiement,
  un `index.html` périmé peut référencer des assets hachés disparus et afficher l'écran d'erreur
  `wasm_load_failed` avec « Recharger » : attendre ou recharger. Permissions minimales (le job `build` ne lit
  que `contents` et `pages`, seul le job `deploy` écrit Pages et émet le jeton OIDC), actions `actions/*`
  épinglées par SHA, aucun secret ; le tout est gardé par `apps/web/tests/ci-policy.test.ts`. Pour
  redéployer à chaque push sur `main`, ajouter `push: branches: [main]` sous `on:` du workflow **et**
  adapter le garde du déclencheur du test (qui échoue sinon, volontairement). Action unique du propriétaire :
  Settings > Pages > Source « GitHub Actions » (et, si un déploiement est refusé par une règle de branche,
  Settings > Environments > `github-pages` : autoriser `main`).
- Une nouvelle mécanique ajoute une brique au moteur sans modifier le comportement des decks
  qui ne l'utilisent pas (à vérifier par le simulateur).
- Format des données : un set = un dossier versionné `data/<set>/<version>/` (`cards.json`, `decks.json`). Une carte a un discriminant `kind` ; un deck embarque son héros et compte exactement 30 cartes ; toute validation passe par `duel-cards` (`parse_cards`, `parse_decks`, `resolve_deck_named`, `resolve_deck` en étant un mince appel), stats bornées à `MAX_STAT` = 10000. Exemple : `data/fixture/1.0.0/`. Une créature a aussi deux champs optionnels : `provocation` (booléen, faux par défaut) et `riposte` (entier ≥ 0, 0 par défaut, borné par `MAX_STAT`), repris tels quels dans `CreatureStats` ; un ancien JSON à quatre clés reste valide ; le moteur lit `provocation` (filtre de cibles) et `riposte` (dégâts rendus à l'attaquant). La fixture `data/fixture/1.0.0/` n'en utilise aucune. Le jeu de départ `data/starter/1.1.0/` (`starter@1.1.0`, mêmes règles que `1.0.0`, qui reste rejouable ; le simulateur prend `1.1.0` par défaut) fournit deux decks de 30 cartes, Ruée (`rush`) et Rempart (`bulwark`), aux héros et aux distributions de coût, force, PV et coût d'attaque nettement différents ; c'est le premier jeu qui utilise `provocation` (Rempart seul) et `riposte` (Ruée seul). Ses valeurs sont provisoires et non équilibrées ; les propriétés demandées sont vérifiées par `crates/cards/tests/starter_set.rs`. La fixture reste l'exemple de format sans ces mécaniques. **Art (optionnel)** : un set peut avoir un `art.json` (tableau `{id, file, author, license, source}`) et un dossier `art/` ; c'est de la présentation, le moteur ne le lit jamais. `parse_art` (`duel-cards`, pure) le valide : une entrée par carte existante, nom de fichier `[a-z0-9][a-z0-9_-]{0,63}.(webp|svg)`, licence `CC0-1.0` ou `CC-BY-4.0`, auteur ≤ 100 et source ≤ 300 caractères en texte libre (jamais un lien actif), document ≤ 64 Kio. Les fichiers (existence, aucun orphelin, signature WebP, ≤ 40 Kio par image, ≤ 400 Kio au total, SVG sans script ni lien externe) sont vérifiés par `crates/cards/tests/art_files.rs`. `parse_cards` refuse aussi deux cartes de même nom (`DuplicateCardName`).
- **Héros** : `max_time`, `starting_hand`, `initiative`, `start_budget`, `health` et une liste `effects` (au plus `MAX_HERO_EFFECTS` = 8) de `{ name, cost, effect }` ; `effect` a un discriminant `kind`, une seule variante pour l'instant, `draw { count }`. Piocher n'est pas une action du moteur : c'est un effet de héros, activé par `Action::Activate { effect }` (indice dans la liste, non fiable, validé par le moteur). `CreatureStats` est aussi (dé)sérialisable : c'est le format du setup JSON d'`engine-wasm`, où chaque côté peut porter en plus `card_names` (liste optionnelle de noms parallèle à `cards`) ; `CreatureStats` lui-même reste sans nom (le nom n'est pas une stat).
- **Règle de temps** (`crates/engine/src/time.rs`, la seule) : budget `b`, coût `c` ; si `c < b` le joueur reste actif avec `b − c`, sinon la main passe et l'adversaire reçoit `min(c − b + 1, max_time adverse)`. Aucune pioche automatique, aucune action « passer ». Le hasard n'est consommé qu'au setup (mélange de One, mélange de Two, puis départage d'une égalité d'initiative) ; `apply` ne l'utilise jamais. `new_game` revalide toutes ses données via `duel-cards`.
- Pas de base de données ni de comptes joueurs tant qu'aucune story ne l'exige.
<!-- PILOT:ARCHITECTURE:END -->

## Format du rapport du simulateur

La bibliothèque `duel_simulator` sait construire et afficher un rapport d'équilibrage (`Report`, `MatchupReport`, `Report::render`, `Report::check`), et `run_batch` le remplit : c'est la sortie du CLI `duel-simulator`. Les parties interrompues par le plafond d'actions (10 000 par défaut) sont comptées à part, exclues des victoires, des causes de fin et de la durée moyenne ; avec les decks de départ, trois matchups (`rush vs bulwark`, `rush (mirror)`, `bulwark (mirror)`), le premier joueur d'un miroir est tiré par la graine de la partie. Aucune valeur de taux n'est figée par les tests du CLI (structure et déterminisme seulement).

- Un rapport est fait d'entiers (aucun flottant stocké) : le set (`nom@version`), la graine, le nombre de parties par matchup, le plafond d'actions et, par matchup, parties lancées / interrompues, victoires A et B, victoires du premier joueur, fins par PV du héros / épuisement / abandon et total d'actions des parties terminées. Invariants : terminées = lancées - interrompues = victoires A + B = somme des trois fins.
- Un matchup est `Cross` (deck A contre deck B) ou `Mirror` (un deck contre lui-même ; ses victoires A / B sont celles des sièges One / Two, et le tableau affiche `-` à la place des victoires par deck : seule la part du premier joueur a un sens).
- Le rendu est du texte brut, déterministe (aucune date ni chemin, ordre des matchups conservé, lignes terminées par `\n`) : une ligne d'en-tête, puis un tableau à colonnes fixes. Chaque pourcentage est calculé sur les parties terminées et suivi de son intervalle de Wilson à 95 %, par exemple `51.2% [48.1-54.3]` ; `n/a` quand aucune partie n'est terminée.
- Les calculs n'utilisent que `+ - * /` et `sqrt` sur des f64 (aucun libm) afin que le texte soit identique sur toute plateforme.

## Getting started
<!-- PILOT:COMMANDS:START -->
- **Rust (workspace) :** `cargo fmt --all`, `cargo clippy --workspace --all-targets -- -D warnings`,
  `cargo test --workspace`
- **Simulateur :** `cargo run -p duel-simulator -- 1000` (parties **par matchup**) ; `[parties] [--set <nom>[@<version>]] [--seed <u64>]`, ordre libre, défauts `1000`, `--set starter`, `--seed 0` ; `-- 200 --set fixture --seed 7` joue le jeu `fixture` ; imprime le rapport (en-tête, une ligne par matchup avec intervalles de Wilson), voir « Format du rapport du simulateur » ; `N = 0`, `--seed` illisible ou option donnée deux fois : code 2
- **Serveur :** `cargo run -p duel-server` (`PORT`, 8080 par défaut ; `GET /health`)
- **Outils requis pour le web :** Node de `apps/web/.node-version` (24), Rust avec la cible
  (`rustup target add wasm32-unknown-unknown`) et `wasm-bindgen-cli` **à la version exacte de
  l'entrée `wasm-bindgen` de `Cargo.lock`** : `cargo install wasm-bindgen-cli --version <version> --locked`
  (`node apps/web/scripts/build-wasm.mjs --print-version` l'affiche). Sans eux, `predev`, `prebuild` et
  `pretest` échouent avec un message qui donne la commande à lancer.
- **Web :** `cd apps/web && npm ci && npm run dev` ; `npm run build` ; `npm test` (chacun régénère
  d'abord `apps/web/src/wasm/` via `scripts/build-wasm.mjs`)
- **E2E web :** `cd apps/web && npm run e2e` (construit d'abord le build de production, puis lance
  Chromium sur `vite preview`, environ 1 à 2 minutes dont ~40 s pour le cas « épuisement »). Chromium
  requis : en local `npx playwright install chromium` ; dans l'environnement des agents le Chromium 141
  est préinstallé (`PLAYWRIGHT_BROWSERS_PATH=/opt/pw-browsers`, ne jamais lancer `playwright install`
  là) et la version de Playwright est épinglée à 1.56.1 pour cette raison. Ils ne tournent **jamais**
  dans la CI automatique : à lancer à la main (onglet Actions, workflow « E2E (manual) », ou
  `npm run e2e` en local), de préférence avant toute fusion qui touche le flux de jeu.
- **Tester à la main :** `cd apps/web && npm run dev`. Un build ne s'ouvre pas en `file://` (modules et
  `.wasm`) : `npm run build && npm run preview -- --host` le sert sur le réseau local (le build porte la
  CSP, pas le serveur de dev). Sur un téléphone hors réseau local : voir #54.
- **Rust, comme la CI :** `cargo fmt --all -- --check`, `cargo clippy --workspace --all-targets --locked -- -D warnings`,
  `cargo test --workspace --locked`, `cargo build --locked --target wasm32-unknown-unknown -p duel-engine-wasm --release`
<!-- PILOT:COMMANDS:END -->

<!-- PILOT:INTRO:START -->
## 🛫 PILOT — Ticket Pipeline

Feature and technical work moves through **PILOT**, a lightweight multi-agent ticket
pipeline: specialized agent personas take a ticket from a raw idea to a merged PR, at a
fraction of the usual token cost, because each phase runs in its own short, focused
context instead of one big context carrying the whole history. PILOT's own
skills/agents/docs are copied in from
[`Jonas-Eve/pilot`](https://github.com/Jonas-Eve/pilot), not written here from scratch —
run `/pilot-update` to pull in that repo's process-doc/label/skill updates.

Five phases, each with its own Claude Code skill and agent persona:

| Phase | Skill | Agent | Produces |
|---|---|---|---|
| 1. Discovery | `/pilot-discovery` | PM + Architect, in dialogue | One or more `type:feature` stories (and, when the same effort needs one, a `type:tech` companion story) under a new/reused Epic, architecture already anticipated. `type:feature` only — a standalone technical need or a bug report never comes through here |
| 2. Spec | `/pilot-spec` | Architect + Tech Lead, in dialogue | Splits an already-formalized story into dev-sized tasks (mandatory for feature, judgment call for tech) and writes each resulting ticket's technical spec, in one continuous pass — or, given no ticket at all, first originates a standalone technical need or a bug report (the one shared bug-creation mechanism) |
| 3. Dev | `/pilot-dev` | Senior Dev, or the e2e persona for the end-to-end-test task | Code + an open pull request |
| 4. Review | `/pilot-review` | Tech Lead + PM by default (feature/e2e) or Tech Lead alone (tech/bug) — Architect optional in either set | A submitted GitHub PR review (approve/request changes) — merges too, only with `--merge` |
| 5. Human QA (feature stories only) | `/pilot-qa` | QA persona, paired with a human | A human-confirmed done, once every task has merged |

A `type:feature` idea always goes through Discovery first; a purely
technical need (CI, security, deployment, migrations, optimization) with no product
framing, or a bug report, skips straight to `/pilot-spec` instead (`--tech`/`--bug`,
explicit or auto-detected) — there's no separate ticket-creation path for either. There's
no single command that runs all five phases end to end — each phase is driven on its own,
one at a time — and each also runs bare, with no argument at all, which is what lets a
scheduled cron Routine drive a phase on a timer instead of a human typing it each time.
`/pilot-discovery` and `/pilot-qa` always run paired with a human (no Routine for either,
though `--resume` picks a paused session back up); `/pilot-spec`
and `/pilot-dev` default to that same pair mode too, so their Routine's
prompt adds `--auto` to run unattended.
A GitHub label state machine (`status:draft` → `status:backlog` → `in-spec` → `dev-ready`
→ `review-ready` → `in-review` → `approved` → `done`, plus `priority:`, a `type:` axis for
the nature of the work
(`feature`/`tech`/`bug`/`e2e`) and a `level:` one for its depth
(`epic`/`story`/`task`, with `status:split` tracking a story broken into tasks) — a bug
is always a `level:task`, created via `/pilot-spec`'s own no-ticket entry and never split)
tracks where a ticket is and stops multiple agents from picking up the same one —
including a `status:changes-requested`
loop back to the dev phase when review finds something that needs an actual code fix, as
opposed to a pure judgment call. A `type:feature` story never reaches `done` straight from
its tasks, either: it passes through `status:qa` → `status:in-qa` first, phase 5's
human confirmation, before `/pilot-qa` itself sets `done`. Two flags sit orthogonal to
that state machine, each
cleared by a human once resolved: `needs-human` is what a phase adds when it hits
something only a person can decide right now; `on-hold` instead pauses a ticket for
something bigger in flight elsewhere (a restructuring, a dependency on other work) with
no immediate decision needed. Agents can propose a merge is ready (`status:approved`); a
human performs the actual merge by default, unless `/pilot-review` was run with `--merge`,
in which case phase 4 merges the PR itself once every reviewer approves.

Each phase is a Claude Code slash command (`/pilot-discovery "..."`, `/pilot-spec 42`,
`/pilot-dev 42`, ...) — see [.pilot/pilot-process.md](.pilot/pilot-process.md) §0 for the
required GitHub labels (created once, not auto-created by any skill) and a full command
quickstart, and the rest of that doc for the state machine and claim protocol in detail.
<!-- PILOT:INTRO:END -->
