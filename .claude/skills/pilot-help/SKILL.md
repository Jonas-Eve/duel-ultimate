---
name: pilot-help
description: "Read-only discovery/help layer over every other PILOT skill and agent in this project's own .claude/skills/pilot-*/ and .claude/agents/pilot-*.md — never claims a ticket, never invokes another skill, and never hardcodes what another skill/agent does (always re-derives it from that skill's current SKILL.md/argument-hint or that agent's current persona file, plus .pilot/pilot-process.md and the relevant .pilot/pilot-link-*.md, so it can't drift when those change). No argument → list mode: every installed pilot-* command grouped as bootstrap/maintenance (pilot-init, pilot-init-archi, pilot-update), the five phases in order (pilot-discovery, pilot-spec, pilot-dev, pilot-review, pilot-qa), and dispatch/utility (pilot-auto, pilot-help itself), each with a one-line summary. A command name (with or without a leading `/` or `pilot-` prefix, e.g. `dev`, `pilot-dev`, `/pilot-dev`) → detail mode: that one command's full purpose, its modes (pair vs `--auto` where it has them), every flag it accepts with what each does, resume/reclaim behavior, and example invocations. An agent/persona name (same normalization, e.g. `architect`, `pilot-architect`; an optional leading `agent` token, e.g. `agent dev`, disambiguates from the two persona names — `dev`, `qa` — that collide with a command name, where the bare name means the command) → agent mode: that persona's stable identity/judgment (never a duty's mechanics, which live in its task docs instead) plus its duties one by one — each task doc, the phase it belongs to, and which skill actually invokes it. A flag token (e.g. `--multi`, `--again`, `--resume`, with or without the leading dashes) → flag mode: which commands accept it, what it does on each (flagging any cross-command difference or incompatibility, e.g. `--again`/`--next` never combining on `pilot-auto`), and where its full mechanics are documented. Anything else — a free-text description of a goal or situation (e.g. 'I want to keep retrying the same ticket until it's done', 'how do I make 3 tech leads agree on a spec') → recommend mode: work out which command(s)/flag(s) accomplish it and answer with the exact command line to run, grounded in that command's actual argument-hint/description rather than a guess — or, if the free text is actually asking who/what persona is responsible for some kind of decision rather than how to run anything (e.g. 'who checks security'), answer by naming the persona instead, grounded in its own agent file, not a command line nobody asked for. Either way, ask a clarifying question instead of guessing when genuinely ambiguous between two candidates. Use whenever a human (or a scheduled Routine's own operator) wants to know what PILOT commands/agents exist, what one specific command/flag/agent does, which command to run for a goal they can describe but can't yet name, or which persona is responsible for a kind of judgment — never for actually running a phase, which stays each phase's own skill."
argument-hint: "[<command name, with or without pilot-/leading slash>] | [agent] <persona name, with or without pilot-/leading slash> | [--<flag name, dashes optional>] | <free text describing what you're trying to do> — omit for the full command list"
---

# PILOT — Help

Purely informational: reads other PILOT skills' and agents' own files and explains them,
but never invokes the `Skill` or `Agent` tool against any of them, never touches a GitHub
issue/PR, and never claims or advances a ticket. If what's actually wanted is running a
phase, say which command to run and stop there — let the human (or the target skill
itself) take it from there.

Every answer is derived live from this project's own already-installed files, never from
a memorized description of what a command "usually" does — those files are the single
source of truth and can change under `/pilot-update`:

- `.claude/skills/pilot-*/SKILL.md` (every installed one, including this skill's own) —
  each one's frontmatter (`description`, `argument-hint`) for the summary/flag list, its
  body for anything more detailed (resume/reclaim mechanics, examples).
- `.claude/agents/pilot-*.md` (every installed persona) — its frontmatter/body for
  identity and judgment, and its own trailing task-doc list for which duties it serves.
- `.pilot/pilot-process.md` — the generic mechanics shared by every phase (pair vs
  `--auto`, the claim protocol, `--resume`, the `status:`/label state machine) that a
  single `SKILL.md` assumes rather than restates.
- `.pilot/pilot-link-*.md` — mechanics shared by *some* commands, not all (e.g.
  `pilot-link-multi-consensus.md` for `--multi`, read by every phase skill and
  `pilot-auto` alike but by none of the bootstrap/maintenance commands).
- `.pilot/pilot-process-companion.md` — a human-facing quickstart with plain example
  invocations of every command; reuse or adapt its examples in detail/recommend mode
  instead of inventing new ones from scratch where one already fits.

If `.claude/skills/pilot-*/` isn't present yet (PILOT not installed in this project),
say so and point at `/pilot-init` instead of guessing at what would be there.

## Determining the mode

**No argument → list mode.** Read every `.claude/skills/pilot-*/SKILL.md`'s frontmatter
and report them grouped into the three fixed categories below (this grouping is
structural — the five-phase order comes from `.pilot/pilot-process.md`'s state machine —
not something to re-derive per run):

1. Bootstrap/maintenance: `pilot-init`, `pilot-init-archi`, `pilot-update`.
2. The five phases, in phase order: `pilot-discovery`, `pilot-spec`,
   `pilot-dev`, `pilot-review`, `pilot-qa`.
3. Dispatch/utility: `pilot-auto`, `pilot-help`.

For each command actually present, one line: its name, a plain-language one-sentence
summary distilled from its `description` (not the full frontmatter dumped verbatim), and
a pointer to run `/pilot-help <name>` for the rest. Skip any of the ten that isn't
actually installed in this project rather than describing it from memory; mention any
`pilot-*` skill present that isn't one of the ten as project-local, not PILOT's. Close
with one line naming the six personas behind these commands (`pilot-pm`,
`pilot-architect`, `pilot-techlead`, `pilot-dev`, `pilot-e2e`, `pilot-qa`) and pointing at
`/pilot-help agent <name>` for one's identity — they're not commands themselves, so they
don't get their own list entry above.

**A command name → detail mode.** Normalize the argument (strip a leading `/`, and add
back a `pilot-` prefix if missing, e.g. `dev` and `pilot-dev` and `/pilot-dev` all mean
the same file) and match it against the installed `.claude/skills/pilot-*/` directories.
No match → say so and suggest bare `/pilot-help` for the full list, rather than guessing
which one was meant. On a match, read that skill's whole `SKILL.md` and answer with:
   - What it does and when to use it (phase number and name, if it's one of the five).
   - Pair vs `--auto`, if it has both — and what `--auto` requires (no live human, e.g. a
     scheduled Routine) sourced from `.pilot/pilot-process.md`, not restated per command.
   - Every flag/argument form it accepts, each with a one-line explanation — pull a
     shared one (`--multi`, `--resume`) from the doc that actually owns it (above) rather
     than re-explaining it differently per command.
   - Resume/reclaim behavior specific to that command, if any.
   - Two or three example invocations, adapted from
     `.pilot/pilot-process-companion.md`'s quickstart if it already has that command's
     examples, otherwise built from the command's own `argument-hint`.

**An agent/persona name → agent mode.** Normalize the same way (strip a leading `/`, add
back a `pilot-` prefix if missing) and match against the six installed
`.claude/agents/pilot-*.md` personas (`pilot-pm`, `pilot-architect`, `pilot-techlead`,
`pilot-dev`, `pilot-e2e`, `pilot-qa`). Two of those names — `dev`, `qa` — also match a
command; on a bare match like that, command detail mode above takes precedence (a
command is the thing actually run) — an explicit leading `agent` token (`agent dev`,
`agent pilot-qa`) means the persona instead. The other four (`pm`, `architect`,
`techlead`, `e2e`) never collide, so the bare name alone is enough — but the leading
`agent` token is always accepted too, on any of the six, not just the two that need it,
so a human unsure which apply can always use it. No match on either →
say so and suggest bare `/pilot-help` for the full list. On a match, read that persona's
whole file and answer with:
   - Its stable identity: the judgment it brings (architectural soundness, product
     coverage, code quality, ...) — never a duty's mechanics, which live in its task docs
     instead, per this repo's own persona/task-doc split (`CLAUDE.md` §3).
   - Its duties, one line each: for every task doc in its own trailing list, name the
     duty, the phase it belongs to, and which skill actually reads/injects that
     `pilot-task-<duty>.md` — grepped across `.claude/skills/pilot-*/SKILL.md`, never
     guessed from the persona's name alone (e.g. the architect's task docs span phases 1,
     2, and 5, each a genuinely different duty, not one repeated three times).
   - A pointer to `/pilot-help <skill>` for how a specific duty actually runs (claim
     protocol, pair/`--auto`, flags) — that's the owning command's job to explain, not
     this persona file's.

**A flag → flag mode.** Normalize (dashes optional, so `multi`, `-multi`, `--multi` all
mean the same lookup) and search every installed `.claude/skills/pilot-*/SKILL.md` for
which ones actually accept it, plus whichever `.pilot/pilot-link-*.md` or
`.pilot/pilot-process.md` section documents its shared mechanics. Answer with: which
commands accept it, what it does on each (calling out any real difference between
commands, not just restating the same sentence per command), and any documented
incompatibility with another flag on the same command (e.g. `pilot-auto`'s `--again` and
`--next` never combine — `.claude/skills/pilot-auto/SKILL.md`) rather than silently
picking one. A flag that matches nothing installed → say it isn't a recognized PILOT
flag rather than guessing at what it might mean.

**Anything else → recommend mode.** A free-text description of a goal, a situation, or a
question phrased as prose rather than an exact command/agent/flag name. Two shapes of
ask land here, and the answer must match which one it actually is rather than defaulting
to the first:
   - "What do I run for X" (a goal or situation to accomplish) → read the installed
     commands' descriptions/argument-hints (and `.pilot/pilot-process.md`'s state
     machine, when knowing which `status:` a ticket is likely in matters) to work out
     which command(s) accomplish it, then answer with the exact command line to run —
     real flags, placeholder issue numbers clearly marked as placeholders — and a short
     "why" tying it back to that command's actual documented behavior, not an invented
     one.
   - "Who/what judges X" (asking which persona is responsible for some kind of decision,
     not how to run anything — e.g. "who checks security", "which persona validates
     acceptance criteria") → read `.claude/agents/pilot-*.md` the same way and answer by
     naming the persona(s), pointing at `/pilot-help agent <name>` for the full identity,
     rather than forcing a command-line answer nobody asked for.
   Genuinely ambiguous between two materially different commands, personas, or flag
   combinations (not just phrasing) → ask which one is meant instead of picking
   arbitrarily; a request that's simply underspecified in a way any single answer already
   covers (e.g. no issue number given, when the target command's own bare/no-argument
   behavior already handles that) doesn't need a question.

## Report

State which mode was used only if it's not obvious from the answer's shape. Never invoke
`Skill` against the command being explained or recommended, and never invoke `Agent`
against the persona being explained or named — the answer is the command line (and why)
or the persona's identity, not the act of running either.
