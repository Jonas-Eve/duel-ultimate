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
- Format des données : un set = un dossier versionné `data/<set>/<version>/` (`cards.json`, `decks.json`). Une carte a un discriminant `kind` ; un deck embarque son héros et compte exactement 30 cartes ; toute validation passe par `duel-cards` (`parse_cards`, `parse_decks`, `resolve_deck`), stats bornées à `MAX_STAT` = 10000. Exemple : `data/fixture/1.0.0/`. Une créature a aussi deux champs optionnels : `provocation` (booléen, faux par défaut) et `riposte` (entier ≥ 0, 0 par défaut, borné par `MAX_STAT`), repris tels quels dans `CreatureStats` ; un ancien JSON à quatre clés reste valide ; le moteur lit `provocation` (filtre de cibles) et `riposte` (dégâts rendus à l'attaquant). La fixture `data/fixture/1.0.0/` n'en utilise aucune. Le jeu de départ `data/starter/1.0.0/` (`starter@1.0.0`) fournit deux decks de 30 cartes, Ruée (`rush`) et Rempart (`bulwark`), aux héros et aux distributions de coût, force, PV et coût d'attaque nettement différents ; c'est le premier jeu qui utilise `provocation` (Rempart seul) et `riposte` (Ruée seul). Ses valeurs sont provisoires et non équilibrées ; les propriétés demandées sont vérifiées par `crates/cards/tests/starter_set.rs`. La fixture reste l'exemple de format sans ces mécaniques.
- **Héros** : `max_time`, `starting_hand`, `initiative`, `start_budget`, `health` et une liste `effects` (au plus `MAX_HERO_EFFECTS` = 8) de `{ name, cost, effect }` ; `effect` a un discriminant `kind`, une seule variante pour l'instant, `draw { count }`. Piocher n'est pas une action du moteur : c'est un effet de héros, activé par `Action::Activate { effect }` (indice dans la liste, non fiable, validé par le moteur). `CreatureStats` est aussi (dé)sérialisable : c'est le format du setup JSON d'`engine-wasm`.
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
