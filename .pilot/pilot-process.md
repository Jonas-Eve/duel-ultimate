# PILOT — Ticket Pipeline (Simplified Multi-Agent Process)

PILOT is this repo's lightweight, in-house multi-agent ticket process: specialized agent
personas take a piece of work from a raw idea to a merged PR, each phase running in its
own isolated agent context (§5) rather than accumulating the whole ticket's history.

Five phases: Discovery → Spec → Dev → Review → Human QA (§7), the last mandatory for
`type:feature` only — a manual acceptance check once every task is merged, before the
story reaches `status:done`. `type:tech`/`type:bug` tickets never go through it.

This document is the source of truth: the state machine, GitHub label conventions, claim
protocol, and how the phases fit together. The agent personas
(`.claude/agents/pilot-*.md`) and skills (`.claude/skills/*/SKILL.md`) reference this file
rather than repeating it — keep this document accurate first, then bring those in line.

---

## 0. Prerequisites & Quickstart

**These GitHub labels must exist before any PILOT skill runs** — no skill creates them on
the fly, and applying a label that doesn't exist fails the API call outright:

`type:feature`, `type:tech`, `type:bug`, `type:e2e`, `level:epic`, `level:story`,
`level:task`, `priority:P0`, `priority:P1`, `priority:P2`,
`status:draft`, `status:backlog`, `status:in-spec`,
`status:dev-ready`, `status:in-dev`, `status:review-ready`, `status:in-review`,
`status:changes-requested`,
`status:approved`, `status:wont-do`, `status:split`, `status:qa`, `status:in-qa`,
`status:done`, `needs-human`, `on-hold`, `can-resume`.

Each phase is a Claude Code slash command, run from a session with read/write access to
this repo's issues and PRs. `/pilot-discovery` and `/pilot-qa` never run except because
something sent their exact command literally, human or scheduled Routine (§4 "Scheduled
sweeps") — the other three phases can also be invoked without one, but every phase, whichever
way it's invoked, only ever acts on a ticket that's actually, currently in its own pre-claim
`status:` (§4 "Claim Protocol"); nothing here infers which phase a ticket belongs to or
invents work. Bare, no-argument commands are what make the scheduled case useful —
see `.pilot/pilot-process-companion.md` for example invocations of each command; there's no
single command that runs all five phases end to end, drive the pipeline one phase at a
time, per ticket.

Every phase skill also runs bare, with **no argument at all** — it then works its normal
pool of fresh candidates (its own pre-claim `status:`) *and* its own in-progress tickets a
human has marked `can-resume` (§4 "Picking the next ticket..." and "Scheduled sweeps"). This is what a cron Routine calls on a timer — with `--auto` added for
`/pilot-spec`, `/pilot-dev`, and `/pilot-review`, all three of which default
to pair and need that flag to run unattended (§4 "Interaction modes").
`/pilot-discovery` and `/pilot-qa` are pair-only and are never Routine-driven at all.

---

## 1. The Five Phases

| # | Phase | Skill | Agent | Produces |
|---|-------|-------|-------|----------|
| 1 | Discovery | `/pilot-discovery` | `pilot-pm` + `pilot-architect`, in dialogue (§4 "Agent dialogue") | One or more `level:story` — `type:feature`, and/or `type:tech` when the same effort genuinely needs a technical enabler alongside the product work — grouped under a new/reused `level:epic` (`type:feature` only) if it doesn't fit in one, with dependencies between them and early architecture decisions (interfaces, infra shape) already anticipated. Never splits into tasks — that's phase 2's job. `status:draft` until human approval, then `status:backlog`. Never for `type:tech` standalone or `type:bug` (§2 "Three levels") |
| 2 | Spec | `/pilot-spec` | `pilot-architect` + `pilot-techlead`, in dialogue — plus `pilot-pm` when a `type:feature` story is split | Splits an already-formalized story into dev-sized tasks (mandatory for `type:feature`, a judgment call for `type:tech`) **and** writes each resulting ticket's technical spec, in one continuous pass — or, given no ticket at all (`--tech`/`--bug`, explicit or auto-detected), originates the ticket itself first, standalone, then splits/specs it the same way. `status:dev-ready` per resulting ticket (the tracker parent, if split, lands on `status:split` instead) |
| 3 | Dev | `/pilot-dev` | `pilot-dev`, or `pilot-e2e` instead for a `type:e2e` task | An implementation + pull request (following `.github/pull_request_template.md`) |
| 4 | Review | `/pilot-review` | `pilot-techlead` + `pilot-pm` (feature/e2e) or `pilot-techlead` alone (tech/bug) — `pilot-architect` optional in either set | A submitted GitHub PR review (Approve/Request changes/Comment): `status:approved`, or blocking points (`needs-human`, plus `status:changes-requested` if code-level) — merges only with `--merge`, otherwise a human merges by hand |
| 5 | Human QA (`type:feature` only) | `/pilot-qa` | `pilot-qa` | A human-confirmed `status:done` once every task has merged, or a `needs-human` flag with what failed (§7) |

Each phase skill is invoked on its own (`/pilot-spec 123`, etc.), as its own isolated
agent call (see §5), one phase at a time.

## 2. Ticket Types And Levels

Two independent axes classify every ticket, and neither is inherited from a parent to a
child — every ticket carries its own of each, assigned for what that specific ticket
actually is, never copied down a tree:

- **`type:`** (this section) — the *nature* of the work: `feature`, `tech`, `bug`, or
  `e2e`. Decides which agent(s) originate it and the phase-4 reviewer
  set (`.pilot/pilot-link-review-consensus.md`) for that ticket specifically.
- **`level:`** ("Three levels" below) — *depth* in the tree: `epic`, `story`, or `task`,
  capped at exactly these three, never deeper.

**Where a ticket originates depends on its `type:`, not a single shared entry point:**

- **`type:feature`** — the only type that goes through Discovery. A human describes an
  idea in free text to `/pilot-discovery`; PM and architect turn it into one or more user
  stories together, in dialogue (§4 "Agent dialogue") — the architect anticipating
  architecture (interfaces, infra shape) from the start rather than challenging a
  finished artifact later. Phase 4 review involves PM + tech lead by default (architect
  optional — its architecture decisions are made and cross-examined by the tech lead
  already, at spec time).
- **`type:tech`** — originates in one of two ways: **grouped** with a `type:feature`
  effort — Discovery's own PM+architect dialogue recognizes the idea needs a technical
  enabler alongside the product work and formalizes it as its own `type:tech` story under
  the same `level:epic` — or **standalone** — a human describes a purely technical need
  (infra, CI, security hardening, migration, no product framing) directly to
  `/pilot-spec`, with no ticket and no Discovery pass at all (`--tech`, or auto-detected):
  the architect formalizes it and, in the same continuous pass, splits/specs it alongside
  the tech lead (§4 "Agent dialogue") — skipping Discovery entirely, since a purely
  technical need has no product judgment for a PM to weigh in on. Phase 4 review is tech
  lead alone (architect optional); the PM is never involved.
- **`type:bug`** — a defect in already-shipped code. **One single creation mechanism for
  the whole process**: `/pilot-spec --bug` (explicit or auto-detected), given no ticket —
  invoked directly by a human, or reused identically by `pilot-dev`/`pilot-e2e` (phase 3)
  or `pilot-qa` (phase 5) the instant they notice one mid-work
  (`.pilot/pilot-link-bug-tickets.md`). The architect alone classifies it: genuinely a
  code defect (created directly as `level:task`, `status:backlog` — dev-sized by nature,
  it never splits, and its spec still gets written by a later, ordinary `/pilot-spec`
  pass rather than in the same breath, so a dev/e2e/qa discovering one mid-work isn't
  blocked writing it); actually a different/new need in disguise (redirect to the
  ordinary `type:feature`/`type:tech` flow instead); or not actionable at all (create
  nothing — a bug has no later `status:wont-do` checkpoint to catch an invalid one).
  Never goes through Discovery. Phase 4 review is tech lead alone (architect optional),
  same as `type:tech` — the PM is never involved, even when found via an e2e test or
  phase-5 QA on a `type:feature` flow: a regression fix isn't a new product call.
- **`type:e2e`** — never created via Discovery (there's no raw "e2e need" a human reports
  from scratch) — always originated by the architect+tech lead's Spec dialogue, as the one
  mandatory end-to-end-test task every `type:feature` split produces
  (`.pilot/pilot-link-e2e-tasks.md`). Phase 4 review is PM + tech lead,
  the same set as `type:feature` — deliberately: reading the test is how the PM confirms
  the split, taken as a whole, actually validates the story's real end-to-end flow, not
  just its individual pieces.

A human never opens a raw, unformalized GitHub issue directly for any of these kinds of
ticket — every ticket is created by an agent, either Discovery (`type:feature`, and any
`type:tech` grouped with one) or Spec (a standalone `type:tech` need, a `type:bug` report,
or the split-off `type:e2e` task). The exceptions: a prerequisite tech or bug ticket the
architect originates itself, inline, mid-Spec pass (using Spec's own no-ticket entry —
"Prerequisite tech/bug tickets" below and `.pilot/pilot-link-bug-tickets.md`); and the PM's own
involvement when a `type:feature` story gets split — see "Three levels" below.

**`type:` is never inherited.** Each task the architect+tech lead create when splitting a
story gets its own `type:`, assigned for what that specific task actually is — not copied
from the story above it. A `type:feature` story's split is typically a mix: one or more
`type:feature` tasks doing the user-facing work, sometimes a `type:tech` task alongside
them for a technical enabler the feature tasks depend on (a migration, a shared piece of
infra) — free to block the feature tasks the ordinary way ("Dependencies between tasks of
the same split" below) — and always exactly one `type:e2e` task
(`.pilot/pilot-link-e2e-tasks.md`). A purely technical enabler task shouldn't get a
PM review it has nothing useful to say about
(`pilot-review/SKILL.md` step 3), and a task's own `type:` is
what routes that review — not which story it happens to sit under.

### Three levels: `level:epic` → `level:story` → `level:task`

Every ticket also carries a `level:` label — depth in the tree, capped at exactly three,
never deeper: a `level:task` is never itself split into further tasks. If phase 2 (Spec)
discovers one is still too big while writing its spec, that's an estimation problem to fix
by re-splitting the *story* differently — never a reason for a fourth level. `level:epic`
is not the same thing as a story split into tasks — the two are different levels and easy
to conflate.

- **`level:epic`** — always `type:feature`; there is no `type:tech` epic. Groups several
  **`level:story`** tickets under a shared theme, for categorization — a `type:feature`
  story, or a `type:tech` story genuinely tied to that same feature effort (above), can
  both sit under it. Never itself scoped, spec'd, or built — no `status:` label. Created
  by Discovery's PM+architect dialogue, at the same moment a story would otherwise be
  created, the instant it's clear the idea can't be delivered as a single story. A
  standalone `type:tech` need originated directly via Spec's own entry point is never
  grouped under an epic — there's no Discovery pass to decide one's needed.
  **Auto-closes once every one of its current stories is closed**
  (`status:done` or `status:wont-do`) — same
  cascading-completion mechanism as a split story's tasks (§3), one level up, driven by
  GitHub's own sub-issue open/closed state rather than a `status:` label (it carries
  none). Closing it this way is terminal, exactly like a split story reaching
  `status:done`: an Epic's theme is defined by the stories ever linked under it, and once
  every one of them is done or wont-do, that theme is realized. Further work in the same
  area becomes a new Epic — never a story added back into a closed one.
- **`level:story`** (`type:feature`/`type:tech`, `status:backlog` at creation — never
  `type:bug`, which is always `level:task` directly, below) — one functional or technical
  unit. It always goes through Spec on its own, where the architect+tech lead decide
  whether it needs splitting into tasks.
- **`level:task`** — normally Spec splits a story into dev-sized tasks; the story
  that got split then gets `status:split` instead (§3) — it is still just a `level:story`,
  one level below an Epic, not an Epic itself. Split along vertical slices (each task
  delivers a coherent, ideally independently shippable/testable unit) rather than by
  technical layer — a front-end-only or back-end-only task is rarely reviewable or
  testable on its own, unless the two are genuinely decoupled (e.g. a backend API meant to
  be consumed later, independently).
  - **`type:tech` story**: splitting is still a judgment call — "it's fine for it not to"
    (§2 "Scoping an already-created story" in `pilot-architect.md`), a well-scoped story
    can carry itself through phase 3 (Dev) as a single ticket, `level:story` the whole way,
    never split into `level:task`s.
  - **`type:bug` ticket: always `level:task` directly, never split, no parent.** The one
    exception to "Spec splits a story into tasks" above: dev-sized by nature, so
    it's created directly as `level:task`, `status:backlog` — the
    one place `level:task` exists standalone. Classified once, at the point it's reported
    or discovered (§2 above; `.pilot/pilot-link-bug-tickets.md`), never in a later pass.
  - **`type:feature` story: splitting is never optional.** A feature story always ends up
    `status:split` with **at least two** tasks — one or more tasks doing the dev work
    (typically `type:feature`, sometimes mixed with a `type:tech` enabler, above) plus
    exactly one `type:e2e` task — never a single unsplit ticket carried through Dev
    directly. See `.pilot/pilot-link-e2e-tasks.md` for why, and §7 "Phase 5 — Human QA" for
    what this makes possible once every task is done. The architect's real judgment for a
    feature story is how many tasks it needs and what each one's own `type:` should be —
    never *whether* to split at all.
  Either way, **for a `type:feature` story**, the PM is also invoked to check the proposed
  `type:feature` tasks still collectively cover the original story's acceptance criteria
  before it's finalized (excluding any `type:tech` or `type:e2e` task from that check —
  neither implements a criterion) — a split that quietly drops part of what the story
  promised would otherwise only surface in phase 4, after everything's already been built.

**Creating or reusing an Epic** (Discovery): before creating a new Epic,
check open `level:epic` issues (always `type:feature`) for one that already fits the
idea/need by theme — reuse it (attach the new story/stories as sub-issues of it) rather
than creating a duplicate. Only create a new Epic when no existing one fits and the idea
genuinely can't be delivered as one story.

**Spec still claims and works exactly one ticket per invocation** (§4 "Claim
Protocol"). But nothing stops the architect/tech lead from *reading* related tickets for
context while working that one ticket: the parent Epic (if any), sibling stories under it,
or anything already linked via "Blocks #M"/"Depends on #N" or a sub-issue relationship —
this is read-only context-gathering, not a second claim.

### Prerequisite tech/bug tickets (distinct from a split's tasks)

A task (above) is part of the *same* ticket's tree — its own `level:task` and `type:`
(§2 intro), never a separate root. A **prerequisite** is the opposite case: while
splitting/speccing a `type:feature` (or `type:tech`) ticket, the architect discovers it
depends on technical work that isn't part of it at all — infra, CI, a shared library, a
migration — exactly the kind of thing that would normally come in as a standalone
`type:tech` need (above), just discovered from inside another ticket's Spec pass instead
of being reported directly. Spec is the one place this still originates a brand-new
ticket itself — the architect already has full context loaded for this pass, so it
creates the prerequisite inline, writing it the same way `.pilot/pilot-task-formalize-tech-need.md`
describes for a standalone need — never the live human drafting loop that doc's own
no-ticket entry adds on top (`pilot-spec/SKILL.md` step 1a): a human live in this pass, if
any, is there for the ticket actually being worked, not to co-draft this one. Land it
directly at `status:backlog` for a later, separate `/pilot-spec` run to pick up, same as
any other prerequisite.

The same applies to a prerequisite **bug** discovered mid-Spec: classify it first
(`.pilot/pilot-link-bug-tickets.md`), and if genuine, create it the same way a standalone
bug is created via Spec's own `--bug` entry.

The architect creates the tech prerequisite — always exactly one `level:story`, standalone,
never grouped under an Epic (there is no `type:tech` epic; if it turns out too big for one
story, that's its own future Spec pass's split judgment call, not a reason to create
several here) — **never** as a sub-issue of the ticket being worked: a
sub-issue would make it `level:task` and tie its lifecycle to the parent's split-tracking,
which is wrong for a ticket that is its own root and goes through Spec/Dev/Review
independently. Instead, link the two directions with a plain issue reference: a comment on
the new ticket ("Blocks #M") pointing back, and a line in the body of the ticket being
worked naming it. A prerequisite bug ticket is `level:task` directly (never `level:story`)
and likewise never a sub-issue. Never `sub_issue_write` for either relationship.

If the ticket being worked itself belongs to a `level:epic`, also comment on that Epic
naming the blocking relationship (e.g. "Story #<worked> is blocked by prerequisite #<new>")
— the sub-issue list only shows the epic's own stories, never an external prerequisite, so
without this an epic's own view never reveals that one of its stories is waiting on
something outside it. Skip this when the worked ticket has no parent Epic.

Whether the prerequisite is a **hard blocker** — the ticket genuinely can't be built until
it lands — decides the exact wording of that line, because §4 "Blocked-by
dependencies" below mechanically gates a ticket on the literal phrase, not on a label:
- Hard blocker → write "Depends on #N". Nothing needs to be lifted by hand: §4's gate
  excludes the ticket from every phase's candidate pool for as long as #N stays open, and
  it becomes a normal candidate again on its own, the next time that phase runs, the
  instant #N closes. A prerequisite bug is always a hard blocker.
- Not a hard blocker (the ticket can proceed in parallel, or the prerequisite is a
  nice-to-have) → write a plain, non-gating reference instead — e.g. "Related
  prerequisite: #N" — informational only, nothing reads or excludes on this wording.

Don't reach for `on-hold` (§3) for this either way: its manual-lift requirement is the
wrong tool for a wait that resolves itself in a specific, mechanically checkable way.
Reserve `on-hold` for a pause with no single ticket to point at.

The prerequisite ticket itself is nothing special once created: a normal `type:tech`
story, `status:backlog`, moving through Spec/Dev/Review like any other (or, for a
prerequisite bug, `level:task`/`status:backlog` straight into a later Spec pass). The only
place its existence matters downstream is the gate check on whatever ticket names it (§4
below).

### Dependencies between tasks of the same split

Distinct from a prerequisite (above, a separate root ticket): when Spec splits a story,
two of its own tasks can still depend on each other — e.g. a front-end task that
consumes an API a back-end task in the same split creates, or a `type:tech` enabler task
that a `type:feature` task needs first (§2 intro). Unlike a prerequisite, this is *within*
the same tree, both tasks `level:task` under the same `level:story` parent regardless of
each one's own independent `type:`. Record it with the same "Depends on #N" phrase (§4
"Blocked-by dependencies") on the dependent task, at split time — the mechanism already
gates on the literal phrase regardless of whether the referenced ticket is a separate root
or a sibling.

### Re-scoping a `type:feature` story after its split is done

Spec only ever splits a `level:story` — never a `level:task` (a task is the *output* of
splitting, not something split again itself; if a task turns out to need further
breakdown, that's re-splitting the *story*, not the task). Within that, `/pilot-spec`
handles an already-once-split `level:story` differently depending on exactly how finished
it is:

- **Closed (`status:done` or `status:wont-do`)** — terminal, full stop. `/pilot-spec`
  refuses to claim it: report that it's already closed and stop, no exception — nothing
  else in PILOT ever moves a ticket backward out of a terminal state. The new work gets
  its own story instead (a plain, non-gating "Extends #N" reference in its body is enough
  to keep the link visible — never a sub-issue, same reasoning as a prerequisite ticket,
  §2 above).
- **`status:qa` or `status:in-qa`** — still reversible, because nothing terminal has
  happened yet: the cascade only just mechanically set `status:qa` the moment the e2e task
  closed, and no human has confirmed anything (`status:in-qa` is one step further — a
  `/pilot-qa` session may even be live — but still short of a verdict). Claiming an issue
  number in either status is a valid, explicit `/pilot-spec` entry point (never part of a
  bare pool) — an existing `status:in-qa` assignee doesn't block this claim, the
  architect+tech lead's claim simply overwrites it. Post a comment explaining why, then
  claim it into `status:in-spec` — the one deliberate, phase-2-only backward transition in
  the whole state machine. From here it's an ordinary Spec pass, just with extra context
  (which tasks, including the original e2e one, are already `status:done` from the
  earlier round):
  1. Propose the new task(s) the same way as any split (§2 "Three levels"), and write
     each one's spec in the same pass.
  2. Propose exactly one new `type:e2e` task for *this round* — never reopen or amend the
     original, already-`status:done` e2e task. Title it distinctly (e.g. "E2E: <story
     summary> (round 2)") and depend only on this round's new task(s). Its spec should
     say explicitly that its job is to *extend* the existing e2e test asset the
     original task wrote, not duplicate coverage from scratch.
  3. Run the PM coverage check (§2 above) against this round's new `type:feature` tasks
     only — the original tasks were already checked at split time.
  4. Finalize exactly like any other split (§4 "Claim Protocol"): the story lands back on
     `status:split`, now tracking this round's new tasks instead.
  Once this round's new e2e task also reaches `status:done`, the story cascades to
  `status:qa` again exactly as before (§3 "Cascading completion") — the new behavior gets
  its own human QA pass in phase 5, same as the original.
- **`status:split`, original e2e task not yet done** — no special handling needed: the
  story hasn't finished its first round yet, so this is an ordinary re-scope. Add the new
  task(s) to the existing split and extend the *existing* (still-open) e2e task's
  dependencies to also cover them, rather than spinning up a second one.

This applies the same way to a `type:tech` story that did split (not every one does, §2
"Three levels") — closed is just as terminal, `status:split` just as
reversible while it's still mid-flight — minus the e2e-task mechanics, which only ever
apply to `type:feature`.

---

## 3. Labels

### `type:` — the nature of the work, set independently on every ticket (§2)
- `type:feature`
- `type:tech`
- `type:bug` — a defect in already-shipped code (§2); classified and originated by the
  architect alone via Spec's own no-ticket entry (or reused identically by
  dev/e2e/qa when they discover one inline), reviewed by tech lead alone (architect
  optional), same as `type:tech`. Always `level:task` directly, never `level:story` —
  it never splits (§2 "Three levels").
- `type:e2e` — a `level:task`'s own type, never a `level:story`'s or `level:epic`'s;
  always exactly one per `type:feature` split (`.pilot/pilot-link-e2e-tasks.md`), reviewed by
  PM + tech lead, same set as `type:feature`. It changes which agent
  `/pilot-dev` calls in phase 3 (`pilot-e2e` instead of `pilot-dev`).

Never inherited from a parent to a child (§2 intro) — a story's tasks each get whichever of
the four fits that specific task, assigned by Spec at split time.

### `level:` — depth in the tree, set once at creation, never recomputed
- `level:epic` — always `type:feature`; groups several `level:story` tickets, `type:feature`
  or `type:tech` alike (§2 "Three levels"). Carries no
  `status:` label and never moves through the pipeline itself — its stories are linked to
  it as native GitHub sub-issues (`mcp__github__sub_issue_write`), not a hand-written
  checklist. It **auto-closes once every one of its stories is closed** (see "Cascading
  completion" below) — same mechanism as a split story's tasks, one level up, and just as
  terminal: further work in the same area becomes a new Epic, not a story reopening this
  one.
- `level:story` — the root unit Discovery produces for `type:feature` (and any grouped
  `type:tech`), or Spec's own no-ticket entry produces for a standalone `type:tech` need
  (§2 "Three levels").
- `level:task` — a dev-sized unit of a `status:split` story, or a standalone `type:bug`
  ticket with no parent (§2 "Three levels"). Never itself split further — depth is capped
  at these three levels.

### `status:` — the pipeline state machine, one label at a time
```
draft → backlog → in-spec → dev-ready → in-dev → review-ready →
  in-review → approved → done
              ↓         ↓
              +---- wont-do (before dev starts) ----+

in-spec → split (tracker for tasks, reaches done only by cascade — see below)

split (type:feature only) → qa → in-qa → done (Phase 5 — Human QA, §7 — instead of
cascading straight to done like a type:tech split does)

in-review → changes-requested → in-dev → review-ready → in-review (loop back through dev
when phase 4 blocks on something that needs an actual code change — see
`status:changes-requested` below and `.pilot/pilot-link-review-consensus.md`)
```
- `status:draft` — Discovery, or Spec's own no-ticket entry, has created the ticket but
  the human hasn't given final approval yet — assigned to whoever's session created it,
  the moment the agents' first draft exists (§4 "Interaction modes"). Excluded from every
  phase's candidate pools, never picked up bare, only reachable by an explicit
  `--resume <issue>` or once a cleared `needs-human` flag makes it resumable. Becomes
  `status:backlog`, unassigned, once the human gives final approval — or, if rejected as
  out of scope instead, `status:wont-do` for a `type:feature`/`type:tech` draft, or
  `needs-human` (still `status:draft`) for a `type:bug` one (`status:wont-do` below). The
  one exception: Spec's own no-ticket entry (§2 intro) doesn't stop at `status:backlog`
  on approval — the same session immediately continues to split/spec what it just
  drafted, so it goes straight to `status:in-spec`, still assigned to that session,
  instead of a momentary unassigned stop nothing would ever pick up.
- `status:backlog` — ticket formalized (a `level:story` from Discovery, or a standalone
  `type:tech` story/`type:bug` task created directly), Spec hasn't started on it.
- `status:in-spec` — Spec (architect+tech lead) has claimed it — covers both the split
  decision and the spec-writing, one continuous claim.
- `status:split` — Spec decided this story is too big for one pass through Dev and broke
  it into tasks instead (§2 "Three levels"). The story
  carries no further `status:` transitions of its own from here — it's a tracker for its
  tasks, the same role an Epic plays for its stories, one level down. It reaches
  `status:done` only via cascading completion (below), never directly.
- `status:dev-ready` — split (if any) and spec written, ready for phase 3 (Dev).
- `status:in-dev` — a dev has claimed it for phase 3.
- `status:review-ready` — a PR is open, unassigned, not currently claimed by phase 4: set
  by `/pilot-dev` when it opens a PR, or pushes a reclaim fix, in place of setting
  `status:in-review` directly (§4 "Claim Protocol" — this is phase 4's own pre-claim
  status, matching `status:dev-ready`'s role for phase 3).
- `status:in-review` — phase 4 has claimed it (§4 "Claim Protocol") and is running, or
  blocked on a `decision`-only point awaiting a human — `needs-human` sits alongside, same
  as any other phase's in-progress status (§3 "`needs-human`" below). Never set directly by
  `/pilot-dev`, only by phase 4's own claim.
- `status:changes-requested` — phase 4 found at least one blocking point tagged `change`
  (`.pilot/pilot-link-review-consensus.md`): an actionable code-level fix, not just a
  question for a human to weigh in on. Set instead of leaving `status:in-review`.
  `needs-human` accompanies it only when the same review also carries at least one
  `decision`-tagged point still awaiting a human — a block made entirely of `change`
  points needs no human decision, so it skips `needs-human` altogether and
  `/pilot-dev` may claim it right away, same as any other pre-claim status. When
  `needs-human` is present (a mixed review), `/pilot-dev` claims it only once a human
  clears that flag (§4 "Reclaiming a `status:changes-requested` ticket") — reclaiming
  either way is expected to find an existing assignee (phase 4's own claiming session, §4
  "Claim Protocol") and overwrite it rather than treat that as a conflict. The dev pushes
  new commits to the *same* already-open PR (never a second PR for the same ticket) and
  moves the ticket to `status:review-ready` when done — never `status:in-review` directly,
  phase 4 claims it fresh.
- `status:approved` — phase 4 ran and every reviewer approved
  (`.pilot/pilot-link-review-consensus.md`) — set instead of
  leaving `status:in-review`. This is the "ready to merge, nothing outstanding" signal.
  Without `--merge`, a human still performs the actual merge — PILOT's default is never to
  merge on its own. Given `--merge` (§4 "Interaction modes"), the same run merges the PR
  itself right after submitting the review, using the repository's normal merge method; the
  ticket then reaches `status:done` via
  `pilot-status-on-merge.yml` exactly as it would after a human merge. There's no automatic
  re-review trigger if new commits land on the PR after this before it's actually merged —
  re-run `/pilot-review` on it by hand, or move it back to `status:review-ready` yourself,
  before merging.
- `status:wont-do` — the concluding agent decided this ticket shouldn't be built after all
  (out of scope, duplicate, superseded) and closed the issue instead of continuing it.
  Settable in two places, both before any spec or code has been written: Discovery/Spec's
  own drafting pass, replacing `status:draft` when a draft is
  rejected instead of leaving a closed issue still labeled `status:draft`; or Spec, in
  place of splitting/speccing the ticket. Once a ticket has reached `status:dev-ready` or
  later, killing it is a human call instead: flag `needs-human` with the reasoning and let
  a human close it. A `type:bug` ticket never carries this label at all: an invalid report
  caught before its ticket even exists is
  simply never created, and a bug draft that only turns out invalid after its ticket
  already exists (or any bug found invalid post-creation) goes the `needs-human` route
  instead of a direct close.
- `status:qa` — a `type:feature` `status:split` story whose e2e task (and therefore
  every dev task it depends on) has reached `status:done` — set by
  `.github/workflows/pilot-status-on-merge.yml` in place of the ordinary cascade straight
  to `status:done` (§3 "Cascading completion"; §7 "Phase 5 — Human QA"). Never set on a
  `type:tech` story, and never on a task itself — which covers every `type:bug` ticket too,
  always `level:task` (§2 "Three levels"). Unclaimed, unassigned — the fresh-work pool
  `/pilot-qa` picks from.
- `status:in-qa` — `pilot-qa` has claimed it for phase 5.
- `status:done` — the ticket is closed, whether merged (a task, or an unsplit `type:tech`
  story) or completed by cascade (a `status:split` story — §3 "Cascading completion") or,
  for a `type:feature` one, by phase 5 (see below and §7). Not set by any phase skill directly
  for an actionable ticket coming out of a merge (merge is always a human action or, for
  phase 4, an explicit `--merge` run, `status:approved` above — either way outside PILOT's
  control at the moment it happens) — set instead by the
  `.github/workflows/pilot-status-on-merge.yml` GitHub Actions workflow, independent of any
  agent session being alive:
  - Triggered on `pull_request: closed` (gated on `merged == true`): resolves the issue(s)
    the merge closes via `PullRequest.closingIssuesReferences`, sets `status:done` on each,
    and, for any that's a task, runs the cascading-completion check (§3 "Cascading
    completion") against its `status:split` parent story, then, if that story itself just
    closed, against its own parent Epic in turn. This is the only place `status:done` gets
    set on an actionable ticket coming out of a merge.
  - Also triggered on `issues: closed`, for completions that never go through a PR merge at
    all: `status:wont-do` (above), which Spec sets and closes directly, and any hand-closed
    issue. This path only runs the cascading-completion check (§3
    "Cascading completion") — never sets `status:done` itself, since the closing party is
    responsible for the issue's own terminal `status:` label — and only when the issue
    already carries `status:done` or `status:wont-do`. It skips entirely when GitHub's own
    timeline shows the closure came from a merged commit reference, so the same closure is
    never processed twice.

  `/pilot-qa` is the one place a phase skill *does* set `status:done` directly, once a
  human confirms the story's behavior in phase 5 (§7) — there's no PR merge there for the
  workflow to react to.

  **Known limitation:** GitHub only recognizes a `Closes #N` reference (populating
  `closingIssuesReferences`, which the workflow reads) when the merging PR's base is the
  repository's *default* branch — never for a PR merged into an intermediate,
  not-yet-merged branch, even once that branch later reaches the default branch itself.
  If a ticket's implementing PR had to be based on another PILOT PR's branch instead of
  the default branch directly, the workflow finds zero closing issues on that merge and
  does nothing — set `status:done` on the ticket by hand once everything has actually
  landed on the default branch. Not fixable in the workflow itself; a GitHub platform
  behavior.

`level:epic` tickets never carry a `status:` label at all. `status:wont-do` and
`status:split` tickets don't carry any *other* `status:` label — the rest of the state
machine only applies to tickets still actively moving through Spec/Dev/Review themselves.

### `needs-human` — an orthogonal flag, not a pipeline state

Unlike every label above, `needs-human` never replaces the ticket's current `status:` —
it sits alongside whichever one is already there. When a phase hits something only a
human can decide, it leaves the ticket's `status:` exactly where it was and adds
`needs-human`, plus a comment. That comment is never optional and is never just the label
with no context attached — it must say, explicitly, both **why** the ticket needs a human
right now and **what's needed** from a human to unblock it. A label with no comment, or
one that only restates "needs a human" without saying why or what for, is not a valid use
of this flag — whoever reads it next has to be able to act from the comment alone.

**Phase 4 is the one exception to "`status:` stays exactly where it was."** A phase-4
block that's entirely `change`-tagged (`.pilot/pilot-link-review-consensus.md`) has nothing
for a human to decide, so it skips `needs-human` same as this section's rule requires — but
still moves to `status:changes-requested` instead of leaving `status:in-review`, since the
ticket needs code regardless of any human input, and `/pilot-dev` may reclaim it right
away. A block that also carries a `decision`-tagged point keeps `needs-human` as usual,
still alongside `status:changes-requested` — the flag gates `/pilot-dev`'s reclaim until
the `decision` point(s) are resolved, even though the `change` point(s) in the same review
are already actionable.

A human resolves it by **removing the `needs-human` label** once they've responded (a
reply comment) or decided there's nothing more to add — the label's absence is the whole
signal that the block is resolved. Whether it's also picked up automatically from there
is a separate choice — see `can-resume` below.

**Always add `needs-human` and post the why/what's-needed comment the moment a phase
hits something only a human can decide** — one path, not two, whether or not a human
happens to be live in the same session right then. What differs by context is only what
happens *next*:

- **Nobody answers on the spot** (a scheduled sweep with no human present, or an
  interactive human who says they need to think about it) — the label and comment stay
  exactly as posted, and the ticket waits for the async resume protocol below.
- **A human is live in the same session** and answers right there in conversation — the
  agent still posts the why/what's-needed comment first, exactly as it would if nobody
  were around; a quick answer is not a reason to skip straight to a resolution. Once the
  human answers, the agent proceeds with that answer and, in the same turn, posts a
  **second** comment summarizing what was actually decided — *then* **removes
  `needs-human` itself**. It never removes it silently: the ticket's GitHub history must
  show both the block and its resolution even when the conversation that resolved it never
  touched GitHub at all — a blocking comment followed by a resolution comment (or, for
  phase 4, a blocking review followed by a second, corrected review,
  `.pilot/pilot-link-review-consensus.md`), whether the
  gap between them was seconds (live) or days (async).

#### Cascading completion

This applies to `status:split` stories only — a story's tasks are a fixed set for
whichever round is currently in flight (the original split, or a later re-scope round,
"Re-scoping a `type:feature` story after its split is done" above), so completeness at
any given moment is provable from a live read of its current sub-issues — never a cached
list. An Epic's own story set can grow while it stays open — new stories are added under
it at any point — but once every one of them is closed (`status:done` or `status:wont-do`),
that's the Epic's own definition of done: the same live read, one level up, closes it too
(below), and that closure is final — it is never reopened to take another story back in.

Whenever a task reaches `status:done` or `status:wont-do`, check whether its parent
is a `status:split` story and, if so, whether *all* of that story's other tasks are
now also done/wont-do. A standalone `type:bug` task has no parent at all (§2 "Three
levels") — this check simply finds none and does nothing further; the bug ticket's own
`status:done`, set directly on merge, is the end of it.
- Not all done yet → do nothing further, the parent stays `status:split`.
- All done, parent is `type:tech` (the only root type that can reach `status:split`
  without being `type:feature` — `type:bug` is never even a `level:story`, so it never
  has a parent to check in the first place) → set the parent story itself to
  `status:done` **and close its issue** — it was never merged directly, so nothing else
  in the pipeline would otherwise ever close it (unlike a task, whose merging PR closes it
  natively via `Closes #N`).
- All done, parent is `type:feature` → set the parent to **`status:qa`** instead of
  `status:done` (§7 "Phase 5 — Human QA") — every `type:feature` split includes exactly
  one e2e task depending on all its dev siblings, so "all done" here is structurally the
  same moment the e2e task itself just finished. `status:done` for this story is set
  later, by `/pilot-qa` itself, once a human confirms the behavior.

Whenever a story's own issue closes — cascaded to `status:done` just above, closed
directly as `status:wont-do` (Spec), or closed by `/pilot-qa` once QA'd — also check
its own parent: if it belongs to an Epic and *every* one of that Epic's stories is now
closed (`status:done` or `status:wont-do`), close the Epic's issue too. An Epic carries no
`status:` label, so closing its issue is the whole of it — and it's terminal: a related
need afterward starts a new Epic, never a story reopening this one.

No periodic sweep exists beyond this — the check is event-driven, not polled. A
`status:split` story whose last task was closed without ever carrying
`status:done`/`status:wont-do` (mislabeled outside PILOT's own conventions) is the one
case this still can't catch, since there is no terminal-status event to react to.

### `on-hold` — an orthogonal pause flag, unrelated to a pending decision

Distinct from `needs-human`: `needs-human` means a phase hit something it needs a
person's judgment on *right now*, and stops until that judgment comes. `on-hold` means
the opposite kind of "don't touch this" — nobody needs to look at it or decide anything
imminently, it's paused because of something bigger in flight elsewhere (a global
restructuring, a dependency on an Epic or on several/fuzzy conditions, deliberately
deferred work) and picking it up now would just be wasted or wrong. A dependency on **one
specific, still-open ticket** is not this — that's the "Depends on #N" mechanical gate
instead (§4 "Blocked-by dependencies"), which needs no label and clears itself the moment
that ticket closes; reach for `on-hold` only when there's no single ticket a phase skill
could check. Like `needs-human`, it sits alongside whichever `status:` is already there
rather than replacing it.

A human applies it directly at any time, or a phase agent applies it itself if it notices
mid-work that the ticket depends on unresolved global work — either way, always with a
comment saying what it's waiting on. A bare `on-hold` label with no comment is as invalid
as a bare `needs-human` one.

Excluded from every phase's candidate pools exactly like `needs-human`. Only a human
removes it, once whatever it was waiting on has actually resolved — there's no
live-session "answer it and remove it in the same turn" path like `needs-human` has,
since there's no question being asked that a live human could answer on the spot. Once
removed, the ticket is simply a normal candidate again. The two flags are independent and
can coexist; clearing one has no effect on the other.

### `can-resume` — a human's mechanical signal that in-progress work is ready to be picked back up

Distinct from `needs-human`/`on-hold`: those two *block* a pool; this one *opts an
already-claimed, in-progress ticket into* one. Only a human ever adds it, in either of two
situations (§4 "Resuming a `needs-human` ticket", "Resuming an orphaned claim"): resolving
`needs-human` and deciding the ticket should resume automatically rather than wait for
someone to type `--resume`, or verifying firsthand that an orphaned claim is genuinely
dead and safe to hand to the next sweep. Either way it means the same thing: whatever's
recorded on the ticket so far is safe to continue from, unattended. A plain label check
— the same deterministic, mechanical style as "Blocked-by dependencies" below — never
something a subagent reasons about or a comment thread it has to reconstruct.

The claiming phase skill removes it the moment it claims the ticket — a one-time "ready"
signal, not a standing state. A human who'd rather resume it themselves, right now, skips
the label entirely and runs `--resume <issue>` directly; both paths preserve the ticket's
recorded progress, they only differ in who picks it back up and when.

### `priority:`
- `priority:P0` / `priority:P1` / `priority:P2` — every ticket gets an initial value at
  creation, set by whichever agent creates it: the PM for `type:feature`/grouped
  `type:tech` (business value/urgency to the user for the feature; the architect's
  technical framing for a grouped tech story), the architect for a standalone `type:tech`
  need or a `type:bug` (the technical framing below — neither has a PM angle). This is
  what lets `/pilot-spec`'s own `status:backlog` pool (§4
  "Picking the next ticket...") sort on something real instead of every fresh story
  falling back to oldest-first.
- At Spec, the architect+tech lead reconfirm or revise it: unchanged if the ticket ends up
  not split (still the one leaf — `type:tech` only, `type:feature` always splits), or
  replaced by each task's own priority if it splits — a task's priority doesn't have to
  match its story's initial one (e.g. a `type:tech` enabler task inside a `type:feature`
  split may be more or less urgent than the user-facing tasks around it). The moment a
  story reaches `status:split`, its own `priority:` label is removed — from then on it's
  a tracker, and its tasks are what any pool actually selects among.
- Not set on `level:epic` (never itself a candidate in any pool) or `status:split`
  tracker parents (superseded by its tasks' own the moment it splits, above).
- Rough default for the *technical* framing (`type:tech`, `type:bug`, and any `type:tech`
  task within a `type:feature` split) if this project has no priority convention of its
  own: P0 closes a real gap (security, correctness, a broken safety net) — do it soon;
  P1 is solid value, not urgent; P2 is nice-to-have or conditional. For `type:feature`,
  the PM's call is about business value/urgency to the user instead. Adopt this project's
  own priority convention in either case, if it already has one that differs.

---

## 4. Claim Protocol (avoiding two agents on the same ticket)

Phases 2 (Spec), 3 (Dev), 4 (Review), and 5 (QA) each start by **claiming** the ticket
before doing any real work, because several instances of the same phase (e.g. several
devs, or two overlapping scheduled review sweeps) may run concurrently. Discovery doesn't
fit this pattern the same way — there's no pre-existing ticket to claim — but the moment
it creates the ticket (`status:draft`, §3), it assigns it the same way, and that
assignment sticks if the pair session ends before final approval, exactly like a claimed
ticket left mid-phase; see "Resuming an orphaned claim" below. Spec's own no-ticket entry
(a standalone `type:tech`/`type:bug` need) behaves the same way as Discovery here — the
ticket is created and assigned the instant a first draft exists, before any human
approval.

1. Read the ticket's current `status:` and assignee.
2. If it's not in the expected pre-claim status, or already has an assignee, stop — it's
   being worked or has moved on; pick a different ticket (or report nothing to do, if a
   specific ticket number was requested explicitly).
3. Otherwise, immediately set the assignee to the current agent/session and swap the
   `status:` label to the in-progress one for this phase.
4. Re-read the ticket once more. If the assignee is no longer this agent, another run won
   the race — stand down and pick something else. This is optimistic, not a real lock:
   cheap insurance against the common case, not a guarantee under true concurrent writes.
5. Only after a successful claim does the phase's real work (the subagent call(s)) start.

Phase 4's claim only serializes separate *runs* of `/pilot-review` against the same
ticket — the reviewers **within** one claimed run (two, or a human-narrowed subset,
`pilot-review/SKILL.md` step 3) still execute independently in parallel, never seeing each
other's verdict (`.pilot/pilot-link-review-consensus.md`).

### Picking the next ticket when none is specified

Discovery has no such pool at all — it always starts from a raw idea in free text (or an
explicit `--resume <issue>`). When any other phase skill is invoked without an explicit
ticket number, it builds its candidate pool from **two** queries, not one:
1. **Fresh work** — tickets in its own pre-claim `status:`, no assignee.
2. **Resumable work** — tickets already in its own *in-progress* `status:`, still carrying
   the assignee from when they were originally claimed, now also carrying `can-resume`
   (§3) — a human's explicit signal that this one's safe to hand to the next sweep. A
   plain label check, same as any other pool query here — never something the subagent
   has to infer from a comment thread.

`/pilot-dev` alone has a **third** pool: tickets in `status:changes-requested` with
`needs-human` no longer present — phase 4 sent these back for an actual code fix
(`.pilot/pilot-link-review-consensus.md`). See
"Reclaiming a `status:changes-requested` ticket" below.

All pools that apply to a given phase skill are merged and picked from together: highest
`priority:` first, then a ticket referenced by another open ticket's "Blocks #M" comment
before one that isn't, then oldest by creation date to break ties; a ticket carrying no
`priority:` at all sorts last. A ticket still carrying
`needs-human` or `on-hold` is never a candidate in any pool, and neither is one whose body
has an unresolved "Depends on #N" reference (below, "Blocked-by dependencies") — both
re-enter automatically once resolved, no flag to remove for the dependency case. An
orphaned claim (already assigned, still in that phase's in-progress `status:`, without
`can-resume`) is likewise never a candidate in any bare pool — see "Resuming an orphaned
claim" below; it resumes only via `can-resume` (once a human has verified it's safe) or an
explicit `--resume <issue>`.

### Given an explicit ticket that has open sub-issues (an Epic, or a `status:split` story)

An explicit ticket number a phase is given isn't always the ticket it ends up acting on:
`level:epic` never carries a `status:` of its own (§3) and is never any phase's
territory. A `status:split` story is a tracker for its tasks (§3 "`status:split`") and
is likewise never `/pilot-spec`, `/pilot-dev`, or `/pilot-review`'s territory — but
`/pilot-spec` keeps its own direct path to re-scope one still in flight ("Re-scoping a
`type:feature` story after its split is done" above), the one exception. Spec, Dev, and
Review each resolve an explicit container ticket before applying any of their own bullets
below — see `.pilot/pilot-link-epic-descent.md` for the mechanism and that exception,
shared across exactly those three skills (and, by delegation, `/pilot-auto`'s ticket-dispatch
mode) rather than repeated here for every phase that doesn't need it.

### Blocked-by dependencies (mechanical gate, distinct from `on-hold`)

A ticket's body may carry one or more "Depends on #N" references. Two places write
them today: the architect, when Spec spins out a prerequisite tech or bug ticket it
judges a hard blocker (§2 "Prerequisite tech/bug tickets"); and
`pilot-dev`/`pilot-e2e` (phase 3) or `pilot-qa` (phase 5), into their own ticket's body
when they originate a bug inline (same doc — always a hard blocker there). The
mechanism itself isn't specific to any of them; anything that writes the same phrase gets
the same gate for free.

**Format: one dependency per line, always.** Multiple dependencies are multiple separate
"Depends on #N" lines, one issue number each — never several numbers combined onto one
line. This keeps the check a trivial per-line literal match, not a list/prose parse.

Whenever a phase builds a candidate pool from its own `status:` pools (above), it also
reads each candidate's body for this exact phrase and, for each `#N` found, checks whether
that issue is still open — plain open/closed, not its `status:` label. A candidate with at
least one still-open dependency is excluded from the pool for this run, the same as
`needs-human`/`on-hold` above — but unlike those two, nothing needs removing by hand: the
moment the referenced ticket closes, the next run of that phase picks the ticket back up
as an ordinary candidate. This is a deterministic tool-call check the skill performs
directly (§5), never something the subagent reasons about.

**In phases 3 (Dev) and 5 (QA), the gate applies to an explicitly-given ticket number too,
not just bare-pool selection.** `/pilot-dev` and `/pilot-qa` each run this same
check before claiming any ticket, whether it got there via the pool or because a human
named it directly — the same principle already applied to an explicit ticket number still
carrying `needs-human`/`on-hold`.

**Spec (`/pilot-spec`) is the deliberate exception, same as before.** Re-splitting/re-speccing
a ticket doesn't need its own prerequisite resolved first — only Dev does; `/pilot-spec`
doesn't check this gate on an explicitly-given ticket number, and it never meaningfully
fires from its own bare pool either, since nothing writes "Depends on #N" onto a ticket
before Spec has worked it at least once.

The same reference feeds the ordering tie-break above.

### Resuming a `needs-human` ticket

A human resolves the flag by **removing the `needs-human` label** from the ticket —
optionally after leaving a reply comment with guidance, or with no reply at all if
there's nothing to add beyond "proceed as proposed." That removal, not a reaction or a
particular comment, is the entire signal a phase skill looks for. This can happen from
any session, at any time — nothing depends on the session that raised the block still
being alive. Whether the ticket also carries `can-resume` (§3) decides whether it's a
bare/scheduled-sweep candidate or only reachable by ticket number.

A phase skill treats a ticket as **resuming**, not a fresh claim, whenever it's already
in that phase's in-progress `status:` (whether picked up via `can-resume` or given
explicitly) — skip the claim, it's already claimed:
1. Read the full blocking context, not just the ticket body — the blocking comment and
   everything posted after it, for every phase except Review. Review's own block is a
   submitted PR review, not a comment (`.pilot/pilot-link-review-consensus.md`) —
   read that, plus the PR's comment thread for
   whatever's posted after it.
2. If `needs-human` is still present, it isn't resolved yet — report that and stop (this
   only matters when a ticket number was given explicitly; the bare pool above already
   excludes these).
3. Otherwise, remove `can-resume` if present (§3) and proceed with the phase's `Agent`
   call(s), passing both the original blocking context and whatever's in the thread after it
   (a specific reply, or "no reply — treat as approved as proposed" if none). The agent(s)
   proceed, correct, or block again if that still doesn't actually resolve things.

### Resuming an orphaned claim (`--resume`, or `can-resume`)

Distinct from both "Resuming a `needs-human` ticket" above and "Reclaiming a
`status:changes-requested` ticket" below: a ticket claimed but with nobody actually still
working it — still carrying its original assignee, still in that phase's in-progress
`status:` (`status:draft`, `status:in-spec`, `status:in-dev`,
`status:in-review`, `status:in-qa`), with **no** `needs-human`. Two different causes leave
the exact same shape, indistinguishable from the ticket alone: a **pair** session
("Interaction modes" below) that ended before reaching the phase's final approval
(pair-capable phases only), or an `--auto` run that died mid-work (a quota limit, a crash,
a timeout) before it could either finish or flag `needs-human`. Left alone, the normal
claim-protocol check ("already has an assignee → stop") would treat either the same as a
ticket someone else is actively working right now, which isn't the case. For Discovery and
Spec's own no-ticket entry specifically, `status:draft` is what makes this possible at all.

Nothing on the ticket itself distinguishes an orphaned claim from one genuinely still in
progress elsewhere — only a human verifying it firsthand can. Once they have, they can
resume it themselves right now via an **explicit** `--resume <issue>` (below — never part
of any bare/scheduled-sweep pool), or add `can-resume` (§3) to hand it to the next sweep
instead, same as a cleared `needs-human` ticket above.

To resume via `--resume`:
1. Read the full ticket — body and comment thread, not just the latest checkpoint. For
   `/pilot-discovery` and `/pilot-spec`'s pair sessions this reconstructs pair
   mode's incremental checkpoint writes ("Interaction modes" below). `/pilot-review`'s own
   checkpoint is a pending GitHub PR review instead of a ticket comment
   (`pilot-review/SKILL.md` has the mechanics) — still pinned to the PR's current head
   commit → that's the recovered outcome, skip re-running the reviewers; stale or absent →
   discard any stale one and
   restart the reviewers fresh, the tech lead's re-validation needs the actual current code
   regardless. For an `--auto` run of any other phase, there's nothing to reconstruct — a
   plain restart from the ticket's original inputs.
2. Claim it the same way a `status:changes-requested` reclaim does (below) — an existing
   assignee doesn't count as a conflict here. Overwrite it (assignee → this session);
   `status:` stays at its current in-progress value.
3. Pass whatever step 1 recovered to the phase's `Agent` call(s) as its starting context,
   and continue: the normal pair loop where one exists, otherwise a fresh pass.

Finalization behaves exactly as any other run of that phase from here.

### Reclaiming a `status:changes-requested` ticket (`/pilot-dev` only)

Unlike "Resuming a `needs-human` ticket" or "Resuming an orphaned claim" above,
`status:changes-requested` is set by *phase 4*
(`.pilot/pilot-link-review-consensus.md`), not by `/pilot-dev` itself — the
ticket already has an open PR, and whatever assignee is still on it is phase 4's own
claiming session (§4 "Claim Protocol"), not necessarily whoever runs `/pilot-dev` next.

Reclaiming one follows the standard claim protocol above with a single, deliberate
exception: an existing assignee doesn't count as "already claimed" for this status — the
claiming session simply overwrites it (assignee → itself, `status:` → `in-dev`) and
re-reads to confirm the overwrite held.

Once claimed, pass the subagent the phase-4 blocking review — its submitted PR review,
read from the PR's reviews rather than the issue's comment thread
(`.pilot/pilot-link-review-consensus.md`) — with its
`change`-tagged points to fix, plus any `decision`-tagged points and their resolution,
instead of a fresh spec — the existing PR's branch is what gets more commits. The subagent
pushes to that same branch/PR (never a second PR for the same ticket) and, once satisfied,
moves the ticket to
`status:review-ready` (unassigned) exactly as it would after a first-time implementation —
never `status:in-review` directly, phase 4 claims it fresh.

Since the blocking points themselves live only in the PR review, not on the issue
(`.pilot/pilot-link-review-consensus.md`), and a pure-`change` reclaim (above) never has a
human look at the ticket before this, the subagent also posts one comment on the issue
itself when it pushes the fix — a short summary of what changed in response to the review,
so the ticket's own history stays coherent with the code without duplicating the review's
full detail (`.pilot/pilot-task-implement.md`).

### Scheduled sweeps

Each phase skill is meant to also run **bare, on a timer** — a Routine whose prompt is
nothing but the literal command. For `/pilot-spec`, `/pilot-dev`, and
`/pilot-review` — all three default to pair mode ("Interaction modes" below) — that literal
command must include `--auto` (still no ticket number computed by the routine itself),
since pair requires a human live in the session and a Routine has none. `/pilot-discovery` and
`/pilot-qa` are pair-only with no `--auto` and are therefore never driven by a Routine at
all. Beyond that flag, this works without any special-casing because "picking the next
ticket when none is specified" (above) already covers both fresh and resumed work
identically. Three independent Routines — one each for `/pilot-spec
--auto`, `/pilot-dev --auto`, and `/pilot-review --auto` (add `--merge` too if the Routine
should also merge once every reviewer approves, §3 "`status:approved`") — each on its own
schedule, so a slow or failing phase never delays the others.

### Interaction modes: pair (default) and `--auto`

The generic contract below — pair pauses at natural checkpoints and writes progress
immediately, `--auto` skips straight to the finished result — is what each phase's own
`SKILL.md` follows by default; a phase is free to specialize it for its own mechanics
(phase 4/Review does, having no natural mid-work checkpoint the other phases share). The
five phase skills split into three groups by which modes they support:

- **`/pilot-discovery`, `/pilot-qa`** — **pair only**, no `--auto` exists for either. Turning
  a raw idea into a story or a technical need into a ticket is a conversation between the
  drafting agents and a human, not something worth running unattended; symmetrically, phase
  5 is a human physically testing shipped behavior, which has no unattended equivalent at
  all. Neither is ever driven by a scheduled Routine as a result. Both still support
  `--resume <issue>` for a ticket left mid-pair.
- **`/pilot-spec`, `/pilot-dev`** — default to **pair**, with `--auto` to
  opt out of it.
- **`/pilot-review`** — default to **pair** too, with `--auto` to opt out of it, but
  specialized further: its reviewers always run fully parallel and isolated from each
  other regardless of mode — there's no natural mid-review checkpoint while they're
  working, so where pair mode's pause falls depends on the aggregated outcome instead. Any
  outcome carrying `needs-human` (a decision-only block, or a mixed `change`+`decision`
  one, `.pilot/pilot-link-review-consensus.md`) always submits immediately, in both modes —
  §3's `needs-human` rule requires the block to reach GitHub before any resolution, even a
  live one — with pair's value applying only *after*, as a live human resolving it right
  there (§3 "A human is live in the same session") instead of the usual async wait. An
  all-approve outcome, or a pure-`change` outcome (no `decision` point, so no
  `needs-human`), instead pauses *before* submitting, for a last look. `--auto` skips every
  pause. Also supports `--merge` (§3 "`status:approved`") — orthogonal to pair/`--auto` —
  and `--resume <issue>` to recover an orphaned claim (above). `pilot-review/SKILL.md` has
  the full mechanics.

Both modes still claim (above) immediately, same as always — it's concurrency
bookkeeping, unrelated to the content decision.

**Pair** requires a human live in the same session. The agent(s) stop at the phase's
natural checkpoint(s) — the drafted story, the proposed split+spec, the implementation
plan — and the skill relays that to the human as an ordinary reply,
waits for their next message, feeds it back, and repeats until they approve.
**The skill writes progress into the ticket immediately at every checkpoint** (a comment,
or a partial `issue_write`) instead of holding it in-conversation until the whole phase is
done — the ticket itself becomes the durable record of how far the pair session got, which
is what makes `--resume` possible. Where the checkpoint is a comment (Spec, Dev, QA), it's
typically **one comment kept up to date** — created once, then edited in place each round
— rather than a new comment per round; whether that comment stays as-is once the phase
finalizes (QA: it's the actual test log, the only record of what was tried) or gets
collapsed to a short final note once the real artifact exists elsewhere (Spec: the
sub-issues/spec; Dev: the PR) is each phase's own call, covered in its own `SKILL.md`.

For Dev and Review a ticket already exists before the phase starts. **Discovery, and
Spec's own no-ticket entry, are the cases where no ticket exists yet at the very first
checkpoint** — the skill creates it right there (`status:draft`, §3) as soon as the
agents' first draft is ready to show the human, instead of holding it in-conversation.
Either way, the very last thing pair mode does — setting the phase's completion `status:`
— is the same end-state `--auto` reaches in one pass, for the phases that have an `--auto`
(Discovery/QA don't, above).

**Final consolidation pass, every phase with incremental checkpoints, right before setting
the completion `status:`.** Writing progress checkpoint by checkpoint means the ticket (or,
for Dev, the diff) was assembled incrementally, round by round of the pair
conversation — nothing guarantees a decision from an early round still squares with one
added several rounds later. Once the human gives final approval, before applying the
completion label, re-read the whole thing as it now stands — the full ticket
body, not just the latest round's delta; the full diff, not just the last commit, for
Dev — and fix anything incoherent or incomplete found. This is also what
`pilot-dev` uses as its own self-review of the implementation before opening
the PR — a substitute for a separate reviewer checking the same thing again in Review
(`.pilot/pilot-link-review-consensus.md`), not an addition to it. Review has no
incremental assembly to reconcile this way — its own aggregation already collapses every
reviewer's one-shot verdict into a single outcome before its one pair checkpoint is ever
reached, so there's nothing earlier to re-read against; each reviewer instead re-checks
its own points once, right before returning its verdict
(`.pilot/pilot-link-review-consensus.md`).

**`--auto`** is the old default, before pair mode existed: the agent(s) decide everything
and the skill applies the finished result straight to GitHub in one pass, no human
checkpoint. This is what a scheduled Routine must use for `/pilot-spec`,
`/pilot-dev`, or `/pilot-review` (above, "Scheduled sweeps") — pair requires a live human.
`/pilot-discovery` has no `--auto` and is therefore never Routine-driven; Spec's own
no-ticket entry, since it needs a raw idea a human types in, is likewise never
Routine-driven regardless of `--auto`, in practice.

`--auto` and `--resume` are mutually exclusive with each other and with pair mode itself —
pick exactly one per run. `--merge` (Review only, §3 "`status:approved`") is a separate,
orthogonal flag: it controls only whether an all-approve outcome also merges the PR itself,
and combines with either pair or `--auto`. `--multi <N>` (`/pilot-discovery`, `/pilot-spec`,
`/pilot-dev`, `/pilot-review` only, "Agent dialogue" below and
`.pilot/pilot-link-multi-consensus.md`) is orthogonal to both: it runs an ensemble on the one
claimed ticket and reconciles it into the single proposal that pair reviews or `--auto`
applies straight through — combines with either mode, invalid only alongside `--resume`
(resuming already-claimed progress, not a fresh ensemble opportunity).

This is unrelated to the "ask live" behavior in §3 ("`needs-human` — an orthogonal
flag") — that one fires for a genuine blocker the agent can't resolve alone, whether or
not pair mode is active; pair is for iterating on a draft together even when nothing is
actually blocking.

### Agent dialogue: Discovery, Spec, and `--multi` on Dev

Discovery (PM+architect) and Spec (architect+tech lead) aren't one persona handing a
finished artifact to another — the two personas genuinely converse before a human ever
sees a draft, with no fixed number of turns: they judge for themselves when they've
converged, or when a disagreement is genuinely irreconcilable and belongs to `needs-human`
instead. `--multi` on Discovery, Spec, and Dev uses this same self-terminating dialogue
between its N instances, instead of the older skill-mediated, capped reconciliation.
`--multi` on Review is the one exception: it keeps that older mechanism exactly as
documented, reviewers never converse (§6). Full mechanics —
`.pilot/pilot-link-agent-dialogue.md` (Discovery/Spec/Dev's own ensemble) and
`.pilot/pilot-link-multi-consensus.md` (Review's `--multi`, unchanged).

---

## 5. Token Efficiency: Isolated Contexts Per Phase

Keeping token cost down per ticket is central to PILOT's design. Two rules keep it cheap:

1. **No phase carries a prior phase's transcript.** A phase skill reads only what that
   phase needs from the ticket (its current body, linked parent/children, the relevant
   `docs/` files) and hands that — never the pipeline's history — to a fresh `Agent` call
   using the matching persona in `.claude/agents/pilot-*.md`. This isolation is across
   phases, not within one: Discovery and Spec's own dialogue (§4 "Agent dialogue") is
   several separate `Agent` calls, one per turn, each one a genuinely fresh context that
   re-reads whatever it needs (this file included) — the running conversation is passed
   as explicit text, not shared memory, so a longer dialogue costs more, turn for turn,
   the same as any other `Agent` call would. The isolation boundary this rule actually
   buys is between phases (and, within Review, between reviewers) — never a claim that a
   multi-turn dialogue is somehow free.
2. **Claim/label bookkeeping is deterministic tool calls in the skill itself**, not
   something the agent reasons about. Only the actual thinking — writing the story,
   splitting/speccing the ticket, writing the code, reviewing the PR — goes
   through an `Agent` call.

A human driving several phases back to back for the same ticket passes forward only the
ticket number between them — never a running transcript of the prior phase's `Agent` call.

---

## 6. Phase 4 — Review Consensus

Claim/pool mechanics are the ordinary claim protocol (§4) — nothing phase-4-specific there
beyond its own pre-claim/in-progress pair, `status:review-ready`/`status:in-review` (§3).
How phase 4 reaches a verdict is split in two: the reviewer set is `pilot-review/SKILL.md`'s
own step 3, and the `change`/`decision` tags contract is `.pilot/pilot-link-review-consensus.md`
— injected whole into each reviewer's task-doc prompt, and read the same way by
`pilot-dev`/`pilot-e2e`'s task docs on reclaim. Not every phase needs either. Pair (default)
vs `--auto` for phase 4 specializes the
generic contract (§4 "Interaction modes") — `pilot-review/SKILL.md` has the full
mechanics, including how it checkpoints and recovers via `--resume`. Merge behavior
(`--merge`) is §3 "`status:approved`"/"`status:done`" — nothing phase-4-specific beyond
what's already there. Its own `--multi` stays the older, skill-mediated, capped mechanism
(§4 "Agent dialogue") — reviewers never converse, in an ensemble or otherwise.

---

## 7. Phase 5 — Human QA (`type:feature` only)

Every other phase in this document is agent-driven, code-level work. Phase 5 is neither:
it's a human physically exercising the shipped, integrated feature and saying whether it
actually behaves as intended — the one check in PILOT no agent can perform on its own,
because it requires a real environment and a real person's judgment on the experience, not
just the diff. `type:tech`/`type:bug` tickets never reach this phase: a `type:tech` split
cascades straight to `status:done` once all its tasks are done (§3 "Cascading
completion"); a `type:bug` ticket — always a standalone `level:task`, never split (§2
"Three levels") — reaches `status:done` directly once its own PR merges.

### When it fires

Set by §3 "Cascading completion": once a `type:feature` story's e2e task (and therefore
every dev sibling it depends on, per `.pilot/pilot-link-e2e-tasks.md`) reaches `status:done`,
the story itself lands on `status:qa` instead of `status:done` — unclaimed, unassigned,
the pool `/pilot-qa` picks from.

### Running it

`/pilot-qa` is **pair-only** (§4 "Interaction modes"). It claims the story (§4 "Claim
Protocol": `status:qa` → `status:in-qa`), builds a manual test plan, and walks the human
through it case by case. A failure is classified and handled exactly as
`.pilot/pilot-link-bug-tickets.md` describes — a genuine defect gets its own
`type:bug` ticket (via the same single mechanism every other phase uses) and unclaims the
story back to `status:qa`; a non-bug failure is reported
for the human to raise via Discovery themselves; anything unclassifiable is a live
`needs-human` block (§3 "A human is live in the same session"). Confirmed, or every
failure resolved to "not actually a bug" → `status:done`, the one direct exception §3
"`status:done`" already notes. `pilot-qa/SKILL.md` covers the rest of the mechanics.
