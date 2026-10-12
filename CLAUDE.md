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
| `crates/engine` | Règles : un seul tour continu, une règle de temps unique, invocation (`Action::Play { card }`, la carte passe de la main au plateau), effets de héros (dont la pioche), attaque du héros adverse (`Action::Attack { attacker, target: Target::Hero }` : coût `attack_cost`, dégâts `force` ; PV à 0 → victoire `HeroDown` sans passation), `Concede`, épuisement (une créature en jeu peut attaquer, donc garde son joueur en vie), `legal_actions` (ordre : cartes en main, activations d'effets, une attaque par créature du plateau, `Concede`) ; les mécaniques des decks seront des briques composables, qu'un deck active ou non. Dépend de `duel-cards` (revalide ses entrées) | Rust 2024, `rand_chacha` (graine) |
| `crates/cards` | Cartes, decks et extensions en **données JSON** versionnées, avec validation | Rust, `serde`/`serde_json` |
| `crates/simulator` | Parties IA contre IA pour mesurer les taux de victoire entre decks (équilibrage) ; pour l'instant, decks de la fixture et politique au hasard | Rust |
| `crates/server` | Parties en ligne (HTTP + WebSocket) | Rust, `axum` 0.8, `tokio` |
| `crates/engine-wasm` | Binding WASM du moteur pour le client (mêmes règles que le serveur) : constructeur depuis un JSON de setup, `legal_actions()` et `apply(action)` en JSON, erreurs `{"code","message"}`, entrée limitée à 1 MiB, aucun accesseur d'état complet | Rust, `wasm-bindgen` |
| `apps/web` | Client **mobile-first et responsive** (PWA ensuite, appli native Capacitor plus tard) | React 19, TypeScript, Vite |

Règles propres à ce repo :
- **Une carte est une donnée, pas du code** ; une extension est un dossier versionné, et chaque
  partie enregistre les versions utilisées (parties rejouables, équilibrage traçable).
- **Aucun hasard hors de la graine du moteur** : toute règle qui tire au sort passe par
  `GameState::random_below`.
- Une nouvelle mécanique ajoute une brique au moteur sans modifier le comportement des decks
  qui ne l'utilisent pas (à vérifier par le simulateur).
- Format des données : un set = un dossier versionné `data/<set>/<version>/` (`cards.json`, `decks.json`). Une carte a un discriminant `kind` ; un deck embarque son héros et compte exactement 30 cartes ; toute validation passe par `duel-cards` (`parse_cards`, `parse_decks`, `resolve_deck`), stats bornées à `MAX_STAT` = 10000. Exemple : `data/fixture/1.0.0/`.
- **Héros** : `max_time`, `starting_hand`, `initiative`, `start_budget`, `health` et une liste `effects` (au plus `MAX_HERO_EFFECTS` = 8) de `{ name, cost, effect }` ; `effect` a un discriminant `kind`, une seule variante pour l'instant, `draw { count }`. Piocher n'est pas une action du moteur : c'est un effet de héros, activé par `Action::Activate { effect }` (indice dans la liste, non fiable, validé par le moteur). `CreatureStats` est aussi (dé)sérialisable : c'est le format du setup JSON d'`engine-wasm`.
- **Règle de temps** (`crates/engine/src/time.rs`, la seule) : budget `b`, coût `c` ; si `c < b` le joueur reste actif avec `b − c`, sinon la main passe et l'adversaire reçoit `min(c − b + 1, max_time adverse)`. Aucune pioche automatique, aucune action « passer ». Le hasard n'est consommé qu'au setup (mélange de One, mélange de Two, puis départage d'une égalité d'initiative) ; `apply` ne l'utilise jamais. `new_game` revalide toutes ses données via `duel-cards`.
- Pas de base de données ni de comptes joueurs tant qu'aucune story ne l'exige.
<!-- PILOT:ARCHITECTURE:END -->

---

## 2. COMMON COMMANDS
<!-- PILOT:COMMANDS:START -->
- **Rust (workspace) :** `cargo fmt --all`, `cargo clippy --workspace --all-targets -- -D warnings`,
  `cargo test --workspace`
- **Simulateur :** `cargo run -p duel-simulator -- 1000` (nombre de parties)
- **Serveur :** `cargo run -p duel-server` (`PORT`, 8080 par défaut ; `GET /health`)
- **Web :** `cd apps/web && npm ci && npm run dev` ; `npm run build` ; `npm test`
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
