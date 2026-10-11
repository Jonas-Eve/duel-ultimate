---
name: pilot-init
description: "One-time bootstrap for a new project adopting PILOT: asks for the project name and a short functional vision, copies PILOT's skills/agents and canonical docs into the project from github.com/Jonas-Eve/pilot, generates the project's own CLAUDE.md, README.md, and PROJECT-FUNCTIONAL-SCOPE.md from templates, scaffolds a default `.github/pull_request_template.md` if none exists, and provisions the GitHub labels the state machine depends on. Refuses to run twice — if already initialized, stops with an explicit message instead of touching anything. Use when a human wants to start using PILOT in a project that hasn't run it before. Not for describing the monorepo's apps/technologies — that's /pilot-init-archi, which requires this to have run first."
argument-hint: "[project name] [functional vision]"
disable-model-invocation: true
---

# PILOT bootstrap — `pilot-init`

This is not one of the five PILOT phases — it's the one-time setup that makes the rest
of PILOT usable in a project. For bootstrapping into a brand-new project where this
skill isn't available yet, see `README.md`'s "Installing in a project" section.

## Get a fresh copy of the source repo

Every step below that copies or reads PILOT's own content does so from a fresh clone of
`https://github.com/Jonas-Eve/pilot`, not a fixed installed location (PILOT has no
plugin/installer). Clone it once at the start into a scratch directory (e.g. `mktemp -d`,
or this session's own scratch directory if it has one) — call that path `$PILOT_SRC` —
and reuse the same clone for every step; don't re-clone per step.

## State marker

This skill's idempotency is tracked in `.pilot/state.json` at the project root:

```json
{
  "pilotInitialized": true,
  "pilotInitializedAt": "<ISO8601>",
  "projectName": "<name>",
  "archInitialized": false,
  "archInitializedAt": null,
  "lastProcessDocSyncAt": "<ISO8601>",
  "lastLabelsSyncAt": "<ISO8601>",
  "lastIntroSyncAt": "<ISO8601>",
  "lastSkillsSyncAt": "<ISO8601>"
}
```

`lastIntroSyncAt` covers every marked block (`PILOT:INTRO`, `PILOT:MAINTENANCE`, and any
later one) — the name predates `PILOT:MAINTENANCE`, but the field wasn't split since both
are always diffed and stamped together. `lastSkillsSyncAt` is when
`.claude/skills/`/`.claude/agents/` were last copied in from `$PILOT_SRC` —
see `.claude/skills/pilot-update/SKILL.md`.

## Steps

1. **Check for a prior run.** Read `.pilot/state.json`. If it exists and
   `pilotInitialized` is `true`, **stop here** and report, verbatim in substance:
   > PILOT is already initialized for this project ("<projectName>", on
   > <pilotInitializedAt>). Nothing to do — `pilot-init` only runs once. To pick up
   > PILOT's own file updates, use `/pilot-update`. To change the functional vision,
   > edit `PROJECT-FUNCTIONAL-SCOPE.md` directly.
   Do not write, overwrite, or re-run anything else in that case.

2. **Gather inputs.** Take the project name and functional vision from the arguments if
   given; otherwise ask the human. The functional vision should be a short paragraph
   (2-5 sentences) — enough to seed `PROJECT-FUNCTIONAL-SCOPE.md`'s "Vision" section and
   the one-line summary atop `CLAUDE.md`/`README.md`, not a full spec (the human fills in
   "In scope"/"Out of scope" themselves later, as the product takes shape).

3. **Check for pre-existing root docs.** If `CLAUDE.md`, `README.md`, or
   `PROJECT-FUNCTIONAL-SCOPE.md` already exist at the project root (retrofitting PILOT
   onto existing work, not a brand-new empty repo), do **not** silently overwrite them.
   Show the human a diff/summary of what the generated version would contain and ask
   whether to overwrite, or skip that file and let them merge the PILOT-relevant sections
   in by hand. Never guess silently either way.

4. **Copy the skills and agents.** Copy every directory under `$PILOT_SRC/.claude/skills/` to
   `.claude/skills/` at the project root, and every file under `$PILOT_SRC/.claude/agents/` to
   `.claude/agents/`, verbatim. All of it — including `pilot-init`, `pilot-init-archi`, and
   `pilot-update` themselves — is PILOT-owned from here on: never hand-edit it in the
   project; `/pilot-update` re-copies all of it from a fresh `$PILOT_SRC` clone.

5. **Render templates.** For each of
   `$PILOT_SRC/assets/templates/CLAUDE.md.tmpl`,
   `$PILOT_SRC/assets/templates/README.md.tmpl`, and
   `$PILOT_SRC/assets/templates/PROJECT-FUNCTIONAL-SCOPE.md.tmpl`, substitute
   `{{PROJECT_NAME}}` and `{{FUNCTIONAL_VISION}}` with the gathered values,
   `{{PILOT_INTRO_CLAUDE}}` / `{{PILOT_INTRO_README}}` with the verbatim contents of
   `$PILOT_SRC/assets/templates/pilot-intro-claude.md.tmpl` /
   `pilot-intro-readme.md.tmpl`, and `{{PILOT_MAINTENANCE_CLAUDE}}` with the verbatim
   contents of `$PILOT_SRC/assets/templates/pilot-maintenance-claude.md.tmpl` (each
   inside its own `<!-- PILOT:*:START -->`/`END` markers — keep those markers in the
   output; `/pilot-update` looks for them later). Write the result to the
   project root (`CLAUDE.md`, `README.md`, `PROJECT-FUNCTIONAL-SCOPE.md`), respecting
   step 3's per-file decision. The `<!-- PILOT:ARCHITECTURE:START -->` and
   `<!-- PILOT:COMMANDS:START -->` marker blocks in `CLAUDE.md`/`README.md` are left as
   their template placeholder text — `/pilot-init-archi` fills those in later.

6. **Copy the process docs.** Copy `$PILOT_SRC/assets/docs/pilot-process.md` to
   `.pilot/pilot-process.md`; every `$PILOT_SRC/assets/docs/pilot-process-*.md` companion
   (currently just `pilot-process-companion.md`) to the matching `.pilot/` path; every
   `$PILOT_SRC/assets/docs/pilot-link-*.md` to the matching `.pilot/` path; and every
   `$PILOT_SRC/assets/docs/pilot-task-*.md` to the matching `.pilot/` path — all at the
   project root, creating `.pilot/` if needed, kept separate from the project's own
   `docs/`. `pilot-process.md` is the generic operational
   spec every phase skill reads; each `pilot-process-*.md` companion is purely human-facing
   (a sequence diagram, currently) — no skill or agent needs it; each `pilot-link-*.md` is
   the opposite of a companion — operational like `pilot-process.md` itself, but scoped to
   the specific two-or-more skills/agents it names, read only by those, not by every phase;
   each `pilot-task-*.md` is what a phase skill reads and passes into an `Agent` call's
   prompt for one specific persona duty — the `.claude/agents/pilot-*.md` files carry only
   identity now, never a duty's mechanics. Copy all four kinds regardless of how many exist
   right now — all are PILOT-owned from here on, never hand-edit any of them;
   `/pilot-update` overwrites them from a fresh `$PILOT_SRC`.

7. **Copy the status-cascade workflow.** Copy
   `$PILOT_SRC/assets/github/workflows/pilot-status-on-merge.yml` to
   `.github/workflows/pilot-status-on-merge.yml` at the project root, creating
   `.github/workflows/` if needed. `.pilot/pilot-process.md` §3 "`status:done`" depends on
   this workflow — without it, merged PRs and closed issues never cascade to `status:done`.
   It's PILOT-owned like the process doc: never hand-edit it; `/pilot-update` overwrites it
   from a fresh `$PILOT_SRC`. If a workflow file of that name already exists and differs,
   treat it like step 3 — show the human what would change and ask before overwriting.

8. **Copy the PR template.** If none of `.github/pull_request_template.md`,
   `.github/PULL_REQUEST_TEMPLATE.md`, root `PULL_REQUEST_TEMPLATE.md`, or
   `docs/PULL_REQUEST_TEMPLATE.md` already exist in the project, copy
   `$PILOT_SRC/assets/github/pull_request_template.md` to
   `.github/pull_request_template.md`, creating `.github/` if needed — this is what
   `pilot-dev`'s "PILOT ticket" section (`.pilot/pilot-task-implement.md`,
   `.claude/skills/pilot-dev/SKILL.md`) fills in when opening a PR. If one of those paths
   already exists, treat it like step 3 — show the human what would be added and ask
   before overwriting, or leave it alone if they'd rather merge the PILOT ticket section
   in by hand. Unlike the process doc and workflow above, this file is a one-time
   scaffold, not PILOT-owned: once copied, it's the project's own to customize freely, and
   `/pilot-update` never touches it.

9. **Provision GitHub labels.** Determine the target repo (`gh repo view --json
   nameWithOwner --jq .nameWithOwner`) and run
   `$PILOT_SRC/scripts/setup-github-labels.sh <owner>/<repo>`. If `gh` isn't
   authenticated or the repo can't be resolved, tell the human and give them the exact
   command to run themselves later — don't block the rest of this skill on it.

10. **Write the state marker.** Create `.pilot/state.json` per the schema above, with
    `pilotInitialized: true`, the current timestamp, the project name, and
    `lastSkillsSyncAt`/`lastIntroSyncAt`/`lastProcessDocSyncAt`/`lastLabelsSyncAt` set to
    now (or left null for whichever of labels was skipped in step 9).

11. **Report.** Summarize what was created/updated/skipped, and suggest running
    `/pilot-init-archi` next to describe the monorepo's apps and generate their scaffolds.
