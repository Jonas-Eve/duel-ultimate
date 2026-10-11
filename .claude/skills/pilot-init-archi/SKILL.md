---
name: pilot-init-archi
description: "One-time monorepo architecture setup, run after /pilot-init: asks how many apps/services the project has and each one's technology (or accepts defaults — Clean Architecture, a Node.js/TypeScript backend, a React/TypeScript frontend, Expo for any mobile app), generates each app's docs, fills in the root CLAUDE.md/README.md architecture and commands sections, and scaffolds each app's skeleton pinned to the latest compatible dependency versions. Refuses to run twice — if already initialized, stops with an explicit message instead of touching anything. Use when a human wants to define or scaffold the monorepo's apps. Requires /pilot-init to have already run."
argument-hint: "[free-text description of the apps/services and their tech]"
disable-model-invocation: true
---

# PILOT bootstrap — `pilot-init-archi`

Not one of the five PILOT phases — the second half of one-time setup, after
`/pilot-init`. It reads and writes `.pilot/state.json` at the project root (schema in
`.claude/skills/pilot-init/SKILL.md`).

## Steps

1. **Check preconditions.** Read `.pilot/state.json`.
   - Missing, or `pilotInitialized` is not `true` → stop and tell the human to run
     `/pilot-init` first. Do nothing else.
   - `archInitialized` is already `true` → **stop here** and report, verbatim in
     substance:
     > This project's architecture is already initialized (on <archInitializedAt>):
     > <comma-separated app names>. Nothing to do — `pilot-init-archi` only runs once. To add
     > a new app or change an existing one's stack, edit its docs and scaffold by hand;
     > this command doesn't support re-running against an existing architecture.

2. **Gather the app list.** From the argument if given, otherwise by asking the human,
   determine: how many apps/services this monorepo has, and for each — its name, its
   one-line purpose, and its technology choice. If the human doesn't know or says
   "default"/"peu importe", apply:
   - **Architecture pattern:** Clean Architecture (Domain → Use Cases → Adapters →
     Infrastructure), matching the layering already described in `CLAUDE.md` §3.
   - **Backend:** Node.js + TypeScript, a minimal HTTP framework (Express unless the
     human names another).
   - **Frontend (web):** React + TypeScript, Vite as the build tool.
   - **Frontend (mobile requested):** React Native via **Expo** + TypeScript. If both
     web and mobile are requested, ask whether they want one Expo app targeting both
     (Expo's web target) or two separate apps (Expo mobile + Vite web) — don't assume.
   - **Everything is TypeScript** unless the human explicitly asks for another language
     for a given app.

3. **Resolve dependency versions.** For every framework/library about to be introduced
   (Express, Vite, React, Expo, TypeScript, etc.), check its latest stable compatible
   version via the registry (`npm view <pkg> version`) rather than from memory — same
   rule as `CLAUDE.md` §3 "New Framework/Dependency → Latest Compatible Version". Pin
   what you find in each generated `package.json`.

4. **Scaffold each app** under `apps/<name>/`, minimal but real (installable,
   runnable, not just empty folders):
   - **Node/TypeScript backend:** `package.json`, `tsconfig.json`,
     `src/domain/`, `src/application/` (use cases), `src/adapters/` (controllers,
     repository interfaces), `src/infrastructure/` (the HTTP framework wiring, DB/external
     clients), `src/main.ts` as the composition root, a `tests/` folder, and a
     per-app `README.md` describing its purpose and dev commands.
   - **React/Vite frontend:** `package.json`, `tsconfig.json`, `index.html`,
     `src/main.tsx`, `src/App.tsx`, a `README.md`. Keep Clean Architecture layering
     lighter here (a `src/domain/` or `src/features/` split is enough) — a frontend
     doesn't need the full four layers a backend does.
   - **Expo app:** `package.json`, `app.json`, `tsconfig.json`, `App.tsx`, `src/`,
     `README.md`.
   Each app's `package.json` gets minimal working scripts (`dev`, `build`, `test` at
   least) so `npm run dev`/`test` actually does something once dependencies are
   installed.

5. **Fill in the root docs.** In the project's `CLAUDE.md` and `README.md`, replace the
   content between the `<!-- PILOT:ARCHITECTURE:START -->`/`END` markers with: the list
   of apps, each one's purpose and tech stack, and how they relate (Clean Architecture
   layering, which app calls which). Replace the content between the
   `<!-- PILOT:COMMANDS:START -->`/`END` markers with the actual per-app dev/test/build
   commands. Keep the marker comments themselves in place so a future `pilot-init-archi`-adjacent
   tool can still locate the block — only replace what's between them.

6. **Update the state marker.** Set in `.pilot/state.json`: `archInitialized: true`,
   `archInitializedAt` to now, and an `apps` array of `{name, purpose, stack}` for what
   was just scaffolded.

7. **Report.** Summarize the apps created, their stacks and pinned versions, and where
   their docs/commands ended up. Remind the human this is a one-time scaffold — adding a
   new app later is a manual, ordinary code change, not a re-run of this skill.
