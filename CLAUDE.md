# CLAUDE CODE GUIDE & DIRECTIVES (MONOREPO ROOT)

## 1. OVERVIEW & SPECIFICATIONS REFERENCE
duel-ultimate — LCG (Living Card Game) en ligne, évolutif, que l'on fera grandir pour le rendre compétitif : des duels 1 contre 1 au tour par tour avec des decks préconstruits et équilibrés, sans booster ni collection. Chaque deck a des mécaniques réellement différentes ; le gameplay se travaille au fur et à mesure. Le moteur de règles est générique et indépendant du thème ; cartes, decks et extensions sont des données versionnées, et l'équilibrage est mesuré par un simulateur IA contre IA.

The functional scope is specified in `PROJECT-FUNCTIONAL-SCOPE.md`. Read it before
designing anything that touches product behavior.

<!-- PILOT:INTRO:START -->
Ticket work (functional or purely technical) goes through **PILOT**, a lightweight agent
process — `discovery` → `spec` → `dev` → `review` — each its
own isolated, low-token agent context instead of one accumulating one, plus a mandatory
fifth phase, human QA, for every `type:feature` story once every task (including its
automated end-to-end test) has merged. Its skills
(`.claude/skills/pilot-*`) and agent personas (`.claude/agents/pilot-*.md`) are copied
into this repo from the `pilot` distribution repo (github.com/Jonas-Eve/pilot) rather
than written here from scratch: the phase skills are invoked as `/pilot-discovery`,
`/pilot-spec`, `/pilot-dev`, `/pilot-review`, `/pilot-qa` — driven one
phase at a time, there's no single command that runs the whole pipeline end to end.
`.pilot/pilot-process.md` in this repo is a verbatim copy of that repo's canonical spec —
the state machine, labels, and claim protocol — kept in sync by `/pilot-update`, never
hand-edited here. Read it before creating or moving a ticket through any phase.
<!-- PILOT:INTRO:END -->

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
| `crates/engine` | Règles : un seul tour continu, une règle de temps unique, invocation (`Action::Play { card }`, la carte passe de la main au plateau), effets de héros (dont la pioche), attaque du héros adverse (`Action::Attack { attacker, target: Target::Hero }` : coût `attack_cost`, dégâts `force` ; PV à 0 → victoire `HeroDown` sans passation) ou d'une créature adverse (`target: Target::Creature(id)`, JSON `{"creature":7}` : elle perd `force` PV (`stats.health` est la santé *courante*), meurt à 0 et va dans la défausse de son propriétaire (`GameState::discard`, publique, dans l'ordre de mort ; `creature_health` pour le plateau) ; dégâts excédentaires perdus, jamais de `HeroDown` ; événements `creature_damaged` puis `died`, avant le changement de joueur actif ; toute cible qui n'est pas une créature du plateau adverse — créature propre, main, pioche, défausse, id inconnu — donne la même erreur `target_not_on_board`, après le contrôle de l'attaquant), **provocation** (tant que l'adversaire a une créature à `provocation` sur son plateau, une attaque ne peut viser que l'une d'elles, le héros n'est pas ciblable ; refus `target_forbidden_by_provocation`, distinct de `target_not_on_board`, évalué après l'attaquant et après la présence de la cible sur le plateau adverse, donc sans fuite sur les zones cachées ; filtre unique `provocation_allows` appelé par `validate` seul, recalculé sur l'état courant, sans cache ; seule la provocation de l'adversaire contraint, une créature qui provoque attaque librement ; une créature morte ne provoque plus), **riposte simultanée** (une créature attaquée inflige sa `riposte` à l'attaquant **en même temps** que celui-ci lui inflige sa `force`, y compris si elle meurt de cette attaque ; les deux dégâts sont calculés sur l'état d'avant l'attaque par `plan_damage` (fonction pure de `combat.rs`) puis appliqués par `apply_damage`, étape isolée qu'un futur effet d'ordre de dégâts modifiera seule ; le héros n'a pas de riposte, une attaque sur le héros n'en déclenche jamais ; riposte 0 : aucun événement ni effet ; excédent perdu, soustractions saturées ; événement additif `riposted { player, creature, amount }`, `player` = propriétaire de l'attaquant, `creature` = l'attaquant, `amount` = riposte nominale de la cible ; journal d'une attaque : `attacked`, `damaged` (héros) ou `creature_damaged` puis `riposted`?, `died`* (tous les morts des deux camps, ordre d'identifiant d'instance croissant, quel que soit le camp ou le rôle, l'attaquant mort comme la cible), `game_won` (`HeroDown`, seulement sur le héros : alors ni riposte ni passation) ou `token_moved`, `active_player_changed`?, `game_won` (`Exhaustion`)? ; le coût d'attaque est payé même si l'attaquant meurt de la riposte ; l'épuisement est évalué après chaque attaque, y compris quand la riposte tue la dernière créature d'un joueur qui reste actif), `Concede`, épuisement (une créature **vivante** peut toujours attaquer — un provocateur adverse s'il y en a, sinon le héros adverse —, donc garde son joueur en vie ; une créature morte a quitté le plateau et ne compte plus), `legal_actions` (ordre : cartes en main, activations d'effets, puis une attaque par couple (attaquant, cible) : par attaquant dans l'ordre d'invocation, le héros adverse d'abord, puis chaque créature adverse dans l'ordre du plateau, jamais une créature propre ni une créature morte, au plus plateau × (1 + plateau adverse) ; face à un provocateur, seuls les provocateurs adverses sont listés, dans l'ordre du plateau, sans le héros ; enfin `Concede` ; `apply` accepte exactement ces actions et `Concede`) ; terminaison : le potentiel Φ (PV des héros + par carte PV courants, +2 en pioche, +1 en main) décroît strictement à chaque action (la provocation ne fait que restreindre les actions légales ; la riposte ne retire que des PV finis ≥ 0 de plus à une créature en jeu, sans jamais rien ajouter), preuve dans `combat.rs` vérifiée par un test sur 250 parties au hasard ; les mécaniques des decks seront des briques composables, qu'un deck active ou non. `GameStarted` (premier événement de `new_game`) porte `data_versions`, copie opaque de `GameSetup.data_versions` (clé obligatoire du JSON de setup, jamais interprétée ni triée ; dernière clé de l'événement, absente du JSON quand la liste est vide, donc les journaux sans versions restent identiques octet pour octet) ; **Noms de cartes** : `PlayerSetup.card_names` (optionnel, absent = pas de noms, `skip_serializing_if` vide) est **parallèle** à `cards`, vide ou de longueur exactement `cards.len()`, chaque nom d'au plus `MAX_NAME_LEN` = 64 caractères ; `build_side` l'associe à la carte (`CardInstance.name`, `pub(crate)`) avant le mélange, donc le nom suit la carte dans toutes les zones ; présentation seulement : aucune règle, formule Φ ni tirage ne le lit, journaux d'événements identiques octet pour octet avec ou sans noms (`validate_side` : forme du deck d'abord, puis noms, puis stats). **Construction depuis les données** (`src/data.rs`, la seule implémentation, sans E/S ni panique) : `player_setup(deck, cards)` (héros, stats et noms via `resolve_deck_named`) et `game_setup_from_data(cards, decks, deck_one, deck_two, data_versions)` (cartes puis decks analysés par `duel-cards`, decks trouvés par identifiant strict, miroir permis, `data_versions` opaque), `DataError` aux codes `invalid_json` / `invalid_setup` / `unknown_deck`. **Vue par siège** (`src/view.rs`, pure, sans générateur ni E/S) : `player_view(&GameState, seat) -> PlayerView` (`PlayerView`, `OwnSide`, `OpponentSide`, `HeroView`, `CardView`, `GameResult`, `Serialize` seulement) ; deux types distincts, `OwnSide` (main du siège) et `OpponentSide` (aucun champ capable de porter la main adverse, seulement `hand_count`) rendent la fuite impossible par construction, les deux n'exposent la pioche qu'en `deck_count`, la vue ne contient ni générateur, ni graine, ni `data_versions` ; forme JSON figée (contrat du client) : clés en snake_case, sièges et `active` en `"One"`/`"Two"` (`PlayerId` n'a pas de `rename_all`, incohérence cosmétique assumée), `result` `null` ou `{"winner","reason"}` (`hero_down`/`exhaustion`/`concede`), `health` = PV courants (0 en défausse), `name` `null` sans noms ; la vue n'authentifie pas le siège (le serveur l'appelle avec le siège de la connexion). Limite connue : `legal_actions` et les événements `drew` exposent les identifiants de la main du joueur actif (acceptable en local, à rédiger côté serveur). **Aperçu du temps** (`src/preview.rs`, pur) : `preview_legal_actions(&GameState) -> Vec<ActionPreview>`, de même longueur et dans le même ordre que `legal_actions`, par **essai à blanc** : chaque action légale est rejouée par `apply` sur un clone de l'état (l'état réel et le générateur ne sont jamais touchés ; `legal_actions`, `apply`, `time.rs` et `rules.rs` inchangés, la règle de temps reste unique) et le résumé est lu dans les événements produits, par un `match` exhaustif sur `Event` sans bras `_` (une nouvelle variante force une revue de l'aperçu). Cinq champs seulement, jamais le contenu des `drew` : `action`, `cost` (`cost` des événements `Played`/`EffectActivated`/`Attacked`, `null` pour `Concede`), `hands_over` (vrai si et seulement si `ActivePlayerChanged`), `budget_after` (`budget` de `TokenMoved`, du joueur qui garde la main ou de celui qui la reçoit, plafonné par son `max_time` ; `null` sans `TokenMoved`), `ends_game` (`GameResult` de `GameWon`, sinon `null`). Sémantique figée : une **attaque létale** sur le héros n'avance aucun temps et ne passe pas la main (`cost` = coût d'attaque affiché, `hands_over` faux, `budget_after` `null`, `ends_game` `hero_down`) ; une **passation** peut finir la partie (`hands_over` vrai **et** `ends_game` `exhaustion`) ; `Concede` : `cost` `null`, `ends_game` `concede` ; le coût est payé même si l'attaquant meurt de la riposte ; une pioche de plusieurs cartes ne change que `cost` et la passation. Un clone par action légale : au pire 931 actions (30 contre 30 créatures), quelques ms en release ; Dépend de `duel-cards` (revalide ses entrées) | Rust 2024, `rand_chacha` (graine) |
| `crates/cards` | Cartes, decks et extensions en **données JSON** versionnées, avec validation ; chemin de résolution unique `resolve_deck_named(deck, cards) -> (stats, noms)` (`resolve_deck` en est un mince appel : mêmes erreurs), `Card::name()`, `MAX_NAME_LEN`, `validate_card_names` ; `parse_cards` refuse aussi deux cartes de même nom (`CardsError::DuplicateCardName`, comparaison exacte, après `DuplicateCard`) ; **art** (`src/art.rs`, pur, présentation seulement, jamais lu par le moteur) : `parse_art(json, cards) -> Result<Vec<ArtEntry>, ArtError>` valide un `art.json` optionnel (voir « Format des données ») ; `roxmltree` n'est qu'une dev-dependency de ce crate (tests des SVG), hors de l'arbre d'`engine-wasm` | Rust, `serde`/`serde_json` |
| `crates/simulator` | Parties IA contre IA pour mesurer les taux de victoire entre decks (équilibrage) ; **bibliothèque `duel_simulator`** (`src/batch.rs` : `run_batch(contenders, config, policy) -> Report` et `run_batch_observed`, `play_game` (une partie, plafond d'actions, parties interrompues jamais cachées), `contenders_from(cards, decks)` et `setup_from(cards, decks, data_versions)` purs, sans E/S, qui renvoient `Err(String)` au lieu de paniquer et appellent le noyau `player_setup` d'`engine` (aucune résolution de deck propre au simulateur) ; elle ne lit jamais le disque) et binaire fin (`src/main.rs`) ; jeux de données **embarqués à la compilation par le binaire seul** (`include_str!`, liste fermée `fixture` et `starter`, jamais de chemin ni de lecture disque) choisis par `--set <nom>[@<version>]` (défaut `starter`, dernière version) ; **matchups** : chaque paire croisée `i < j` puis chaque miroir (avec les 2 decks de départ : `rush vs bulwark`, `rush (mirror)`, `bulwark (mirror)`), `N` parties **par matchup**, sièges alternés dans un matchup croisé (le deck A au siège One aux indices pairs), victoires comptées par deck ; **graines** : une graine de lot (`--seed`, défaut 0) dérivée par `derive_seed` (finaliseur SplitMix64) en graine de partie, le moteur la reçoit telle quelle, la politique reçoit un RNG distinct par siège, aucun hasard hors de la graine de lot (même lot, même rapport) ; **politique** : `Policy = fn(&[Action], &mut ChaCha8Rng) -> usize` ne voit que les actions légales hors `Concede` (au hasard par défaut, attaques sur héros et sur créatures) ; sortie : exactement `Report::render()` (voir « Format du rapport du simulateur »), argument invalide : message sur stderr et code 2 ; aucun objectif d'équilibre sur `starter` (T4) | Rust |
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
- **Client web** : aucune règle de jeu en TypeScript. La version de données est une constante
  unique (`DATA_VERSION = "starter@1.0.0"`, `apps/web/src/data/starter.ts`), épinglée par test contre
  le chemin importé (`data/starter/1.0.0/`, import `?raw`, `server.fs.allow` dans `vite.config.ts`).
  Le seul accès au moteur est l'interface `GameSession` (`src/engine/`, couches `engine/` <- `game/`
  <- `ui/`, gardées par `tests/architecture.test.ts` et `tests/security-policy.test.ts`). La table
  d'erreurs françaises (`src/ui/errors.fr.ts`) est gardée par `tests/error-codes.test.ts` contre les
  sources Rust : un nouveau code d'erreur Rust casse ce test tant qu'on n'ajoute pas son texte ; le
  contrat JSON (`src/engine/types.ts`, `EVENT_KINDS`) est gardé par `tests/wasm.contract.test.ts`.
  Le `base` de Vite vient de la variable d'environnement `VITE_BASE` (`vite/base.ts`, `resolveBase`, lue dans le processus Node de la config ; défaut `/`, seule la forme `/` ou `/<segment>(/<segment>)*/` est acceptée, le reste lève une erreur nommant `VITE_BASE`, une base relative est refusée à cause du `.wasm`) et `BUILD_ID` (`src/data/build.ts`, `buildIdFrom` : 7 premiers caractères de `VITE_BUILD_SHA` s'il est hexadécimal minuscule de 7 à 40 caractères, sinon `dev`) est lu par `App` seul (garde de `tests/architecture.test.ts`) et affiché « Version : <id> » sur l'accueil ; `preview` doit recevoir le même `VITE_BASE` que le build qu'il sert. Contre l'ordinateur, la confidentialité de sa main n'est qu'un choix d'affichage local.
  Partie locale (`apps/web/src/game/`, #63) : le contrôleur ne calcule ni coût, ni cible, ni fin de partie ; l'ordinateur ne reçoit jamais l'abandon ; le journal contre l'ordinateur est écrit d'après la seule vue du siège humain. Le contrôleur (`GameController`) est créé par l'appelant dans un gestionnaire d'événement, jamais dans un `useEffect` ; `dispose()` annule le pilote et libère la session une fois ; une session qui a levé une erreur fatale (throw non JSON) est morte, plus aucun appel n'atteint le WASM. Interface (`apps/web/src/ui/`, #64) : `App` est le seul lecteur de `data/starter` et de `engine/load` (`ui/` n'importe ni l'un ni l'autre, gardé par `tests/architecture.test.ts`) ; il charge le moteur pendant l'écran de chargement, assemble la session de façon synchrone (`createWasmSession`) puis crée le contrôleur par `startGame(...)` **dans le gestionnaire du clic** « Lancer la partie » / « Rejouer » (jamais dans un `useEffect`, gardé par le scan de source `tests/controller-creation-policy.test.ts` ; le test de montage StrictMode garde le cleanup qui libère), appelle `controller.start()`, et libère l'ancien contrôleur avant d'en créer un autre, au retour à l'accueil et au démontage. `startGame` ne lève jamais : `entropy_unavailable` (graine manuelle proposée, jamais `Math.random`), `invalid_seed` et `engine` ont chacun leur affichage français (`ui/fr.ts`, `ui/errors.fr.ts`). Coût, « passe la main » et fin de partie affichés viennent de l'aperçu du moteur (`ui/consequence.ts`, aucun calcul côté client) ; les contrôles sont de vrais `<button>` de 44 px, inactifs pendant le tour de l'ordinateur et après la fin ; `Dialog` (piège à focus, Échap, focus rendu) est générique et réutilisé par #65 et #66. Aucun attribut `style=` ni `<style>` (classes CSS seulement) ; la CSP est injectée au **build seulement** par `vite/csp-plugin.ts` (`'wasm-unsafe-eval'`, ni `'unsafe-inline'` ni `'unsafe-eval'`), gardée par `tests/csp.test.ts`. Test de l'interface : `App({ deps })` reçoit le vrai moteur par prop (jamais de mock), `tests/support/{ui,app}.tsx`, `playUntil` dans `tests/support/policies.ts`. Journal et défausses (#65) : `ui/log-format.ts` (pur) formate les `StepRecord` ; `switch` exhaustif sur `eventKind`, un test lit `enum Event` de `crates/engine/src/rules.rs` (une variante ajoutée casse la compilation puis le test) ; contre l'ordinateur la pioche adverse n'a pas de nom parce que l'enregistrement vient de la seule vue du siège humain (jamais de table globale id -> nom ; choix d'affichage, pas une garantie) ; `LogBar` est un `role="log"` poli sauf pendant le tour de l'ordinateur (`off`, une seule annonce de fin de chaîne), le tiroir et le journal de l'écran de fin ne sont pas vifs ; défausses publiques des deux camps, en cartes compactes (`CardTile destroyed`, #93 : attaque et riposte, icône « détruite » et légende « Détruite » à la place de la vie, jamais « PV 0 »), le journal et les défausses restent consultables pendant le tour de l'ordinateur. Flux d'attaque (#66) : `selectedAttacker` est un état local de `GameScreen`, **dérivé** par `ui/attack-state.ts` (pur) des aperçus `snapshot.legal` (cibles = les aperçus `isAttackBy`, jamais une liste construite par le client ; la provocation n'est pas recalculée : un héros ou une créature absents des aperçus sont `aria-disabled` « Cible non disponible ») ; une sélection ou une feuille que plus aucun aperçu ne soutient est abandonnée pendant le rendu (jamais par un `useEffect`) ; `AttackSheet` (confirmation, coût « Coût d'attaque », passation, fin de partie, via `describeConsequence`, aucun dégât annoncé) appelle `controller.submit(aperçu)` ; états signalés par bordure et texte. Pastille de deck (#94) : type fermé `DeckEmblem = "bolt" | "battlements"`, `DeckRef.emblem?` (`game/controller.ts`) et `DeckInfo.emblem?` avec la table provisoire `DECK_EMBLEMS` (`data/starter.ts`, un seul export, remplacée par #88) ; `App` passe les `DECKS` au contrôleur, l'emblème arrive par `snapshot.players` à `SideModel.emblem` (`toScreenModel`), `ui/` et `game/` n'importent jamais `data/` ; `CardTile`/`CardFace` reçoivent `emblem` et dessinent un SVG `aria-hidden` `.card-emblem-bolt|battlements` en haut à droite de l'illustration, selon le siège de chaque carte (plateaux, mains, défausses), jamais le joueur courant ; absent = aucune pastille ; nom accessible inchangé ; contraste 3:1 gardé par `tests/emblem-contrast.test.ts`. Carte compacte (#92) : `CardTile` pose un élément externe `.card.card-compact` (bouton ou `div role=group`) qui contient `.card-face` (**exactement** `--card-w` = 62 px × 112 px, `box-sizing: border-box`, nom tronqué, marques sans effet sur la taille) puis `.card-tags` (sous la carte, dans l'élément interactif) ; `CardFace` (exporté, purement visuel) montre le nom, la pastille de coût (sablier + chiffre) **en main seulement** (jamais sur un plateau, jamais deux fois), une illustration SVG `aria-hidden` de substitution et **trois valeurs** à chiffres ≥ 10 px en px (attaque = `force`, riposte même à 0, vie = PV courants) ; cadre commun par jetons `:root` (`--card-*`) ; sous la carte « Provocation », la légende de main « Reste N » / « Passe la main » (lue sur l'aperçu, jamais calculée) et la marque. Libellé : « Nom, coût N (main seulement), attaque N, riposte N, vie N[, provocation][, passe la main | il te restera N de temps][, marque] » (le coût d'attaque n'est plus sur la carte, il reste dans `AttackSheet`) ; `tests/support/card-label.ts` en écrit la forme attendue littéralement. `.card` reste partagé avec la défausse (`DiscardDialog`) : la taille fixe est sur `.card-compact` ; les rangées main et plateaux ont `gap: 2px` et `padding: 4px`. Pas de vie maximale dans `CardView` : aucun style « blessé ».
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
- **Déploiement** (`.github/workflows/deploy-pages.yml`, « Deploy to Pages », #54/#81) : publie le client
  (`apps/web`, `VITE_BASE=/duel-ultimate/` et `VITE_BUILD_SHA` dérivé de `GITHUB_SHA` en shell, jamais d'un
  secret ni d'une valeur libre) sur `https://jonas-eve.github.io/duel-ultimate/`. **Manuel seulement**
  (`workflow_dispatch`, sans `inputs`, décision du propriétaire pour économiser le quota) : onglet Actions >
  « Deploy to Pages » > Run workflow, sur `main`, **depuis un `main` vert** : il ne lance ni tests, ni Clippy,
  ni `fmt`, ni e2e. Deux jobs : `build` (`contents: read` et `pages: read`, car `configure-pages` interroge
  l'API Pages, sans `enablement` : échec rapide avec un message si Pages est désactivé) et `deploy`
  (`pages: write` et `id-token: write`, environnement `github-pages`, une seule étape `deploy-pages`, ni
  checkout ni `run:`) ; actions `actions/*` épinglées par SHA. Vérifier la version déployée : le pied de
  l'accueil affiche « Version : <SHA à 7 caractères> » (à comparer à `git rev-parse --short origin/main`) et
  « Données : starter@1.0.0 ». **Cache** : Pages envoie `Cache-Control: max-age=600` et les en-têtes ne se
  règlent pas ici ; pendant ~10 minutes après un redéploiement, un `index.html` périmé peut référencer des
  assets hachés disparus et afficher l'écran d'erreur `wasm_load_failed` avec « Recharger » : attendre ou
  recharger. `VITE_BASE` : voir `apps/web/README.md` (#80). Gardé par `apps/web/tests/ci-policy.test.ts` (bloc
  dédié, hors du `describe.each` commun). Pour redéployer à chaque push sur `main`, ajouter
  `push: branches: [main]` sous `on:` **et** adapter le garde du déclencheur (sinon le test échoue, à dessein).
  Action unique du propriétaire : Settings > Pages > Source « GitHub Actions » ; si un déploiement est refusé
  par une règle de branche, Settings > Environments > `github-pages` : autoriser `main`. Aucun secret,
  aucun service worker, aucune analytique.
- Une nouvelle mécanique ajoute une brique au moteur sans modifier le comportement des decks
  qui ne l'utilisent pas (à vérifier par le simulateur).
- Format des données : un set = un dossier versionné `data/<set>/<version>/` (`cards.json`, `decks.json`). Une carte a un discriminant `kind` ; un deck embarque son héros et compte exactement 30 cartes ; toute validation passe par `duel-cards` (`parse_cards`, `parse_decks`, `resolve_deck_named`, `resolve_deck` en étant un mince appel), stats bornées à `MAX_STAT` = 10000. Exemple : `data/fixture/1.0.0/`. Une créature a aussi `provocation` (booléen, défaut faux) et `riposte` (u32 ≥ 0, défaut 0, ≤ `MAX_STAT`, 0 valide sans minimum 1), `#[serde(default)]` sur `Card::Creature` et `CreatureStats` (toujours émis à la sérialisation ; la compatibilité exigée est en lecture : un JSON à quatre clés reste accepté), validés par `validate_creature` (erreur `StatTooHigh { field: "riposte" }`). Le moteur lit `provocation` (S2b-T2, filtre de cibles) et `riposte` (S2b-T3, dégâts rendus à l'attaquant) ; la fixture ne change pas (`S2A_GOLDEN`, dans `crates/engine/tests/engine.rs`, est le journal de référence que les decks sans riposte ni provocation doivent laisser identique). Jeu de départ `data/starter/1.0.0/` (version enregistrée `starter@1.0.0`) : deux decks de 30 cartes, `rush` (One) puis `bulwark` (Two), héros d'initiatives différentes (Ruée commence toujours) ; premier jeu qui utilise `provocation` (Rempart seul) et `riposte` (Ruée seul). **Valeurs provisoires, non équilibrées** ; propriétés pinnées par `crates/cards/tests/starter_set.rs`. La fixture reste l'exemple de format sans ces mécaniques. **Art (optionnel, #100)** : un `data/<set>/<version>/art.json` et un dossier `art/` d'images, de la présentation séparée des règles ; `art.json` est un **tableau** d'entrées `{id, file, author, license, source}` (`deny_unknown_fields`), validé par `parse_art` (pure, document ≤ `MAX_ART_DOC_BYTES` = 64 Kio testé avant l'analyse ; `id` = carte existante, une seule entrée par carte ; `file` = `^[a-z0-9][a-z0-9_-]{0,63}\.(webp|svg)$`, unique ; `license` dans la liste fermée `ART_LICENSES` = `CC0-1.0`, `CC-BY-4.0` ; `author` 1 à 100 et `source` 1 à 300 caractères, sans caractère de contrôle, texte libre jamais un lien actif ; erreurs `ArtError`, distinctes de `CardsError`). Les fichiers sont contrôlés par `crates/cards/tests/art_files.rs` (existence, ni lien ni sous-dossier, aucun orphelin, signature WebP, ≤ 40 Kio par image et ≤ 400 Kio au total, SVG en liste blanche d'éléments sans script, `on*`, lien externe, DOCTYPE ni `@import`, défense en profondeur : un SVG est toujours chargé par `<img>`). Aucun set actuel n'a d'`art.json`. Le moteur, `CardView`, le simulateur et le binding ne lisent jamais l'art.
- **Héros** : `max_time`, `starting_hand`, `initiative`, `start_budget`, `health` et une liste `effects` (au plus `MAX_HERO_EFFECTS` = 8) de `{ name, cost, effect }` ; `effect` a un discriminant `kind`, une seule variante pour l'instant, `draw { count }`. Piocher n'est pas une action du moteur : c'est un effet de héros, activé par `Action::Activate { effect }` (indice dans la liste, non fiable, validé par le moteur). `CreatureStats` est aussi (dé)sérialisable : c'est le format du setup JSON d'`engine-wasm`, où chaque côté peut porter en plus `card_names` (liste optionnelle de noms parallèle à `cards`) ; `CreatureStats` lui-même reste sans nom (le nom n'est pas une stat).
- **Règle de temps** (`crates/engine/src/time.rs`, la seule) : budget `b`, coût `c` ; si `c < b` le joueur reste actif avec `b − c`, sinon la main passe et l'adversaire reçoit `min(c − b + 1, max_time adverse)`. Aucune pioche automatique, aucune action « passer ». Le hasard n'est consommé qu'au setup (mélange de One, mélange de Two, puis départage d'une égalité d'initiative) ; `apply` ne l'utilise jamais. `new_game` revalide toutes ses données via `duel-cards`.
- **Rapport du simulateur** (`crates/simulator/src/{stats,report}.rs`, pur, sans E/S) : intervalle de Wilson à 95 % (`z = 1,96`) ; déterminisme numérique : f64 avec `+ - * /` et `sqrt` seulement, **aucun libm** (`powi`, `ln`, `exp`) ; compteurs entiers (`Report`, `MatchupReport`), rendu texte `Report::render` figé par un test d'or, invariants vérifiés par `Report::check`. Câblé au CLI par `run_batch` (`src/batch.rs`), qui remplit le `Report` ; la ligne historique `N games: player one wins x%` n'existe plus et aucun taux de simulation n'est épinglé : les tests du CLI vérifient la structure et le déterminisme.
- Pas de base de données ni de comptes joueurs tant qu'aucune story ne l'exige.
<!-- PILOT:ARCHITECTURE:END -->

---

## 2. COMMON COMMANDS
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

---

## 3. CODING STANDARDS & GLOBAL RULES
- **New Framework/Dependency → Latest Compatible Version:** when introducing a new
  framework or library (not bumping an existing one), pin the latest stable release
  compatible with the app's runtime — check the registry (`npm view <pkg> version`,
  `pip index versions <pkg>`) rather than guessing or reusing an older version from
  memory. Existing dependencies keep their current upgrade cadence; this rule is about
  the version picked at introduction time.
- **Clean Architecture:** Strict dependency isolation between Domain, Use Cases,
  Adapters, and Infrastructure across all services.
- **Security:** Never log secrets or tokens. Inter-service calls that cross a trust
  boundary require an explicit shared secret or equivalent, never an implicit trust
  assumption.

---

## 4. DOCUMENTATION MAINTENANCE RULE
At the end of any task that modifies architecture, endpoints, folder structures, or API contracts:
1. Check if files in `docs/` or subfolder `CLAUDE.md` files are outdated.
2. Automatically update the corresponding `.md` files before finalizing the task.
3. **Keep `CLAUDE.md` and `README.md` (root and per-app) synchronized in substance, not
   verbatim.** They target different audiences — `CLAUDE.md` instructs the LLM, `README.md`
   informs human developers — so the same facts (workflow, architecture, conventions)
   should hold true in both, but phrasing, depth, and emphasis can differ per audience.
   A change that touches one MUST be checked against the other before finalizing the task.
<!-- PILOT:MAINTENANCE:START -->
4. **PILOT's file family is tightly cross-referenced** — `.pilot/pilot-process.md`, every
   `.pilot/pilot-link-*.md`, `.pilot/pilot-process-*.md`, and `.pilot/pilot-task-*.md`, every
   `.claude/skills/pilot-*/SKILL.md`, every `.claude/agents/pilot-*.md`, the
   `PILOT:INTRO` blocks in this file and `README.md`, and the rest of the PILOT mentions
   in both. All of it is PILOT-owned, copied in from
   [github.com/Jonas-Eve/pilot](https://github.com/Jonas-Eve/pilot): never hand-edit any
   of it, run `/pilot-update` instead. A renamed label, moved section heading, or
   changed mechanism upstream can leave stale references in the rest of this
   file/`README.md` outside the marked blocks (e.g. a command name in prose) — after
   running `/pilot-update`, check cheaply: `grep` the exact term(s) that changed across
   both files and fix what turns up.
5. **A synced mechanism change can also leave a gap a `grep` won't find** — this file's
   or `README.md`'s own prose about PILOT (a workflow description, a walkthrough) can go
   stale in substance even when no single term was renamed, if what it describes no
   longer matches the freshly-synced canonical behavior. After `/pilot-update`, skim that
   prose against what actually changed, not just the renamed-term grep above.
<!-- PILOT:MAINTENANCE:END -->

---

## 5. GIT WORKFLOW
- **Commit Freely On A Work Branch, Never On `main`:** on a short-lived work branch,
  committing and pushing completed work is authorized by default. On `main`, never commit
  or push without the user explicitly asking in the current task.
- **Amend, Don't Stack, Within The Same Not-Yet-Merged Context:** commits on a work
  branch exist to separate distinct contexts. If a new commit only fixes, corrects, or
  completes the change the immediately preceding commit made (not yet merged into `main`),
  amend that commit instead of adding a separate commit for the same context. Once a
  commit has merged into `main` (or a review has already landed on it), it's shared
  history — never amend or force-push it. A branch is also "shared" the moment another
  branch is created from it, even before either merges into `main`.
- **Trunk-Based Development:** `main` is the single trunk. Work happens on short-lived
  branches created from `main` and merged back quickly — no long-lived feature or release
  branches.
- **Conventional Commits:** Every commit message follows `type(scope): description`
  (e.g. `fix(backend): resolve null pointer in session lookup`). Common types: `feat`,
  `fix`, `docs`, `refactor`, `test`, `style`, `chore`. `scope` is typically the affected
  app, or omitted for repo-wide changes.
- **Squash Merge By Default, Rebase Merge For Deliberately Separate Commits:** squash a PR
  into `main` as one commit unless it carries multiple commits that are each independently
  meaningful on purpose (not fixups of an earlier commit in the same PR — those should have
  been amended per the rule above) — rebase-merge those instead, to keep them distinct in
  `main`'s history. Never a merge commit either way, and never force-push `main`.
