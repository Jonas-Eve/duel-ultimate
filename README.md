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
| `crates/engine` | Règles de base minimales ; les mécaniques des decks seront des briques composables, qu'un deck active ou non | Rust 2024, `rand_chacha` (graine) |
| `crates/cards` | Cartes, decks et extensions en **données JSON** versionnées, avec validation | Rust, `serde`/`serde_json` |
| `crates/simulator` | Parties IA contre IA pour mesurer les taux de victoire entre decks (équilibrage) | Rust |
| `crates/server` | Parties en ligne (HTTP + WebSocket) | Rust, `axum` 0.8, `tokio` |
| `crates/engine-wasm` | Binding WASM du moteur pour le client (mêmes règles que le serveur) | Rust, `wasm-bindgen` |
| `apps/web` | Client **mobile-first et responsive** (PWA ensuite, appli native Capacitor plus tard) | React 19, TypeScript, Vite |

Règles propres à ce repo :
- **Une carte est une donnée, pas du code** ; une extension est un dossier versionné, et chaque
  partie enregistre les versions utilisées (parties rejouables, équilibrage traçable).
- **Aucun hasard hors de la graine du moteur** : toute règle qui tire au sort passe par
  `GameState::random_below`.
- Une nouvelle mécanique ajoute une brique au moteur sans modifier le comportement des decks
  qui ne l'utilisent pas (à vérifier par le simulateur).
- Pas de base de données ni de comptes joueurs tant qu'aucune story ne l'exige.
<!-- PILOT:ARCHITECTURE:END -->

## Getting started
<!-- PILOT:COMMANDS:START -->
- **Rust (workspace) :** `cargo fmt --all`, `cargo clippy --workspace --all-targets -- -D warnings`,
  `cargo test --workspace`
- **Simulateur :** `cargo run -p duel-simulator -- 1000` (nombre de parties)
- **Serveur :** `cargo run -p duel-server` (`PORT`, 8080 par défaut ; `GET /health`)
- **Web :** `cd apps/web && npm ci && npm run dev` ; `npm run build` ; `npm test`
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
