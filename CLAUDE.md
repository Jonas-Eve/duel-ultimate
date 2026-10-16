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
| `crates/engine` | Règles : un seul tour continu, une règle de temps unique, invocation (`Action::Play { card }`, la carte passe de la main au plateau), effets de héros (dont la pioche), attaque du héros adverse (`Action::Attack { attacker, target: Target::Hero }` : coût `attack_cost`, dégâts `force` ; PV à 0 → victoire `HeroDown` sans passation) ou d'une créature adverse (`target: Target::Creature(id)`, JSON `{"creature":7}` : elle perd `force` PV (`stats.health` est la santé *courante*), meurt à 0 et va dans la défausse de son propriétaire (`GameState::discard`, publique, dans l'ordre de mort ; `creature_health` pour le plateau) ; dégâts excédentaires perdus, jamais de `HeroDown` ; événements `creature_damaged` puis `died`, avant le changement de joueur actif ; toute cible qui n'est pas une créature du plateau adverse — créature propre, main, pioche, défausse, id inconnu — donne la même erreur `target_not_on_board`, après le contrôle de l'attaquant), **provocation** (tant que l'adversaire a une créature à `provocation` sur son plateau, une attaque ne peut viser que l'une d'elles, le héros n'est pas ciblable ; refus `target_forbidden_by_provocation`, distinct de `target_not_on_board`, évalué après l'attaquant et après la présence de la cible sur le plateau adverse, donc sans fuite sur les zones cachées ; filtre unique `provocation_allows` appelé par `validate` seul, recalculé sur l'état courant, sans cache ; seule la provocation de l'adversaire contraint, une créature qui provoque attaque librement ; une créature morte ne provoque plus), **riposte simultanée** (une créature attaquée inflige sa `riposte` à l'attaquant **en même temps** que celui-ci lui inflige sa `force`, y compris si elle meurt de cette attaque ; les deux dégâts sont calculés sur l'état d'avant l'attaque par `plan_damage` (fonction pure de `combat.rs`) puis appliqués par `apply_damage`, étape isolée qu'un futur effet d'ordre de dégâts modifiera seule ; le héros n'a pas de riposte, une attaque sur le héros n'en déclenche jamais ; riposte 0 : aucun événement ni effet ; excédent perdu, soustractions saturées ; événement additif `riposted { player, creature, amount }`, `player` = propriétaire de l'attaquant, `creature` = l'attaquant, `amount` = riposte nominale de la cible ; journal d'une attaque : `attacked`, `damaged` (héros) ou `creature_damaged` puis `riposted`?, `died`* (tous les morts des deux camps, ordre d'identifiant d'instance croissant, quel que soit le camp ou le rôle, l'attaquant mort comme la cible), `game_won` (`HeroDown`, seulement sur le héros : alors ni riposte ni passation) ou `token_moved`, `active_player_changed`?, `game_won` (`Exhaustion`)? ; le coût d'attaque est payé même si l'attaquant meurt de la riposte ; l'épuisement est évalué après chaque attaque, y compris quand la riposte tue la dernière créature d'un joueur qui reste actif), `Concede`, épuisement (une créature **vivante** peut toujours attaquer — un provocateur adverse s'il y en a, sinon le héros adverse —, donc garde son joueur en vie ; une créature morte a quitté le plateau et ne compte plus), `legal_actions` (ordre : cartes en main, activations d'effets, puis une attaque par couple (attaquant, cible) : par attaquant dans l'ordre d'invocation, le héros adverse d'abord, puis chaque créature adverse dans l'ordre du plateau, jamais une créature propre ni une créature morte, au plus plateau × (1 + plateau adverse) ; face à un provocateur, seuls les provocateurs adverses sont listés, dans l'ordre du plateau, sans le héros ; enfin `Concede` ; `apply` accepte exactement ces actions et `Concede`) ; terminaison : le potentiel Φ (PV des héros + par carte PV courants, +2 en pioche, +1 en main) décroît strictement à chaque action (la provocation ne fait que restreindre les actions légales ; la riposte ne retire que des PV finis ≥ 0 de plus à une créature en jeu, sans jamais rien ajouter), preuve dans `combat.rs` vérifiée par un test sur 250 parties au hasard ; les mécaniques des decks seront des briques composables, qu'un deck active ou non. `GameStarted` (premier événement de `new_game`) porte `data_versions`, copie opaque de `GameSetup.data_versions` (clé obligatoire du JSON de setup, jamais interprétée ni triée ; dernière clé de l'événement, absente du JSON quand la liste est vide, donc les journaux sans versions restent identiques octet pour octet) ; Dépend de `duel-cards` (revalide ses entrées) | Rust 2024, `rand_chacha` (graine) |
| `crates/cards` | Cartes, decks et extensions en **données JSON** versionnées, avec validation | Rust, `serde`/`serde_json` |
| `crates/simulator` | Parties IA contre IA pour mesurer les taux de victoire entre decks (équilibrage) ; **bibliothèque `duel_simulator`** (`src/batch.rs` : `run_batch(contenders, config, policy) -> Report` et `run_batch_observed`, `play_game` (une partie, plafond d'actions, parties interrompues jamais cachées), `contenders_from(cards, decks)` et `setup_from(cards, decks, data_versions)` purs, sans E/S, qui renvoient `Err(String)` au lieu de paniquer ; elle ne lit jamais le disque) et binaire fin (`src/main.rs`) ; jeux de données **embarqués à la compilation par le binaire seul** (`include_str!`, liste fermée `fixture` et `starter`, jamais de chemin ni de lecture disque) choisis par `--set <nom>[@<version>]` (défaut `starter`, dernière version) ; **matchups** : chaque paire croisée `i < j` puis chaque miroir (avec les 2 decks de départ : `rush vs bulwark`, `rush (mirror)`, `bulwark (mirror)`), `N` parties **par matchup**, sièges alternés dans un matchup croisé (le deck A au siège One aux indices pairs), victoires comptées par deck ; **graines** : une graine de lot (`--seed`, défaut 0) dérivée par `derive_seed` (finaliseur SplitMix64) en graine de partie, le moteur la reçoit telle quelle, la politique reçoit un RNG distinct par siège, aucun hasard hors de la graine de lot (même lot, même rapport) ; **politique** : `Policy = fn(&[Action], &mut ChaCha8Rng) -> usize` ne voit que les actions légales hors `Concede` (au hasard par défaut, attaques sur héros et sur créatures) ; sortie : exactement `Report::render()` (voir « Format du rapport du simulateur »), argument invalide : message sur stderr et code 2 ; aucun objectif d'équilibre sur `starter` (T4) | Rust |
| `crates/server` | Parties en ligne (HTTP + WebSocket) | Rust, `axum` 0.8, `tokio` |
| `crates/engine-wasm` | Binding WASM du moteur pour le client (mêmes règles que le serveur) : constructeur depuis un JSON de setup, `legal_actions()` et `apply(action)` en JSON, erreurs `{"code","message"}`, entrée limitée à 1 MiB, accesseur `start_events()` des seuls événements de démarrage (`game_started`, éventuellement `game_won` par épuisement ; publics, identiques à chaque appel), aucun accesseur d'état complet (la défausse se reconstruit côté client à partir des événements `died`) | Rust, `wasm-bindgen` |
| `apps/web` | Client **mobile-first et responsive** (PWA ensuite, appli native Capacitor plus tard) | React 19, TypeScript, Vite |

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
- **CI** (`.github/workflows/ci.yml`, sur `pull_request` et `push` vers `main`) : `fmt`, `clippy`,
  tests Rust, build WASM, contrôle de non-entropie, puis `npm ci`, `npm run build` et `npm test`
  dans `apps/web`. Permissions `contents: read`, actions épinglées par SHA, aucun secret ; sa
  politique est gardée par `apps/web/tests/ci-policy.test.ts`.
- Une nouvelle mécanique ajoute une brique au moteur sans modifier le comportement des decks
  qui ne l'utilisent pas (à vérifier par le simulateur).
- Format des données : un set = un dossier versionné `data/<set>/<version>/` (`cards.json`, `decks.json`). Une carte a un discriminant `kind` ; un deck embarque son héros et compte exactement 30 cartes ; toute validation passe par `duel-cards` (`parse_cards`, `parse_decks`, `resolve_deck`), stats bornées à `MAX_STAT` = 10000. Exemple : `data/fixture/1.0.0/`. Une créature a aussi `provocation` (booléen, défaut faux) et `riposte` (u32 ≥ 0, défaut 0, ≤ `MAX_STAT`, 0 valide sans minimum 1), `#[serde(default)]` sur `Card::Creature` et `CreatureStats` (toujours émis à la sérialisation ; la compatibilité exigée est en lecture : un JSON à quatre clés reste accepté), validés par `validate_creature` (erreur `StatTooHigh { field: "riposte" }`). Le moteur lit `provocation` (S2b-T2, filtre de cibles) et `riposte` (S2b-T3, dégâts rendus à l'attaquant) ; la fixture ne change pas (`S2A_GOLDEN`, dans `crates/engine/tests/engine.rs`, est le journal de référence que les decks sans riposte ni provocation doivent laisser identique). Jeu de départ `data/starter/1.0.0/` (version enregistrée `starter@1.0.0`) : deux decks de 30 cartes, `rush` (One) puis `bulwark` (Two), héros d'initiatives différentes (Ruée commence toujours) ; premier jeu qui utilise `provocation` (Rempart seul) et `riposte` (Ruée seul). **Valeurs provisoires, non équilibrées** ; propriétés pinnées par `crates/cards/tests/starter_set.rs`. La fixture reste l'exemple de format sans ces mécaniques.
- **Héros** : `max_time`, `starting_hand`, `initiative`, `start_budget`, `health` et une liste `effects` (au plus `MAX_HERO_EFFECTS` = 8) de `{ name, cost, effect }` ; `effect` a un discriminant `kind`, une seule variante pour l'instant, `draw { count }`. Piocher n'est pas une action du moteur : c'est un effet de héros, activé par `Action::Activate { effect }` (indice dans la liste, non fiable, validé par le moteur). `CreatureStats` est aussi (dé)sérialisable : c'est le format du setup JSON d'`engine-wasm`.
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
