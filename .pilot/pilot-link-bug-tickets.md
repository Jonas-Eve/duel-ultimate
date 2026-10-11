# PILOT link — Bug ticket creation (one mechanism, several invokers)

Injected whole by `.claude/skills/pilot-spec/SKILL.md` (both when a human invokes its
no-ticket `--bug` entry directly, and for a prerequisite bug the architect discovers
mid-Spec), `pilot-dev/SKILL.md`, and `pilot-qa/SKILL.md` — the phase-specific delta
(whether the discovering ticket needs unclaiming, and how) lives in each duty's own task
doc instead, never here. See `.pilot/pilot-process.md` §2/§3/§4 for the generic ticket
types, labels, and claim protocol this builds on.

**One mechanism, four invokers.** A `type:bug` ticket is always created the same way,
regardless of who's invoking it — a human describing a defect directly to `/pilot-spec
--bug` (or auto-detected), the architect discovering a prerequisite defect mid-Spec, or
`pilot-dev`/`pilot-e2e` (phase 3) / `pilot-qa` (phase 5) noticing one mid-work. Never
created any other way, and never through Discovery — classifying a bug needs only the
architect, no product judgment a PM would add.

Distinct from a prerequisite *tech* ticket (`.pilot/pilot-process.md` §2 "Prerequisite
tech/bug tickets" — a new technical need, no defect implied): a concrete **defect** in
already-shipped code, most often surfacing while writing/running an e2e test or a phase-5
manual case, but not limited to those.

**Classify it before creating anything** (`.pilot/pilot-process.md` §2 intro): genuinely a
code defect against something already agreed on, or actually a new/different need in
disguise. If it's not a bug, don't create a `type:bug` ticket: treat it as a prerequisite
*tech* ticket if it's a new technical need (via Spec's own `--tech` entry, "Prerequisite
tech/bug tickets"), or leave it for a human via Discovery if it needs an actual
product/judgment decision. If it isn't a real, actionable defect at all (already fixed,
not reproducible, an exact duplicate of an open bug ticket, or working as intended), say
so and create nothing — a bug has no `status:wont-do` checkpoint to catch an invalid one
later.

**If it genuinely is a bug, create it directly**: `type:bug`, `level:task`, unassigned —
never `level:story` (`.pilot/pilot-process.md` §2 "Three levels" — a bug never splits),
never grouped under a `level:epic`. Write it directly (what's
broken, how to reproduce/observe it, best root-cause diagnosis and suggested fix location
if known, severity/impact), with its own `priority:` (`.pilot/pilot-process.md` §3 — the
architect's technical framing) set by whoever creates it. Which `status:` it lands on, and
whether it gets spec'd right away, is each invoker's own delta below — the shared
mechanism is only the classification and the ticket's own content.

**Linking, when discovered mid-another-ticket** (the prerequisite case, and the phase-3/5
inline-discovery case alike): same mechanics as a prerequisite tech ticket
(`.pilot/pilot-process.md` §2 "Prerequisite tech/bug tickets") — never a sub-issue of the
ticket being worked. Link the two directions: "Blocks #M" on the new bug ticket, "Depends
on #N" in the discovering ticket's own body — always a hard blocker, since the
discovering ticket cannot be finished until the bug is fixed.

Each invoker's own delta beyond this shared mechanism:

- **A human, via `/pilot-spec --bug`**: no discovering ticket to link back to — just
  create it standalone, `status:draft` while the human confirms the classification live
  (this entry always has a human right there), then this same run continues straight
  into speccing it — `status:draft` → `status:in-spec` directly, no `status:backlog` stop
  in between (`pilot-spec/SKILL.md` step 1a) — a bug is small enough that running
  `/pilot-spec --bug` is naturally a single "report and spec it" pass, not two separate
  ones.
- **The architect, mid-Spec** (a prerequisite): no live drafting session of its own — the
  human present, if any, is focused on the ticket actually being worked, not this one —
  so create it straight at `status:backlog`, for a later, separate `/pilot-spec` run to
  pick up. Finish splitting/speccing the ticket being worked normally afterward — it isn't
  claimed by Dev yet, so recording the dependency is enough, no unclaiming needed.
- **`pilot-dev`/`pilot-e2e`, mid-implementation**: same as the architect's prerequisite
  case — straight to `status:backlog`, left for a later `/pilot-spec` run, never spec'd in
  this pass. The discovering ticket is already claimed and mid-phase, so unclaim it
  instead of leaving it stuck — `.pilot/pilot-task-implement.md` covers the exact steps
  (push WIP to a branch, comment, clear assignee, back to `status:dev-ready`).
- **`pilot-qa`, mid-manual-test**: same idea, straight to `status:backlog`, no branch/commit
  involved — `.pilot/pilot-task-human-qa.md` covers the exact steps (comment naming the
  new ticket(s), clear assignee, back to `status:qa`).
