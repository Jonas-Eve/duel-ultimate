---
name: pilot-spec
description: "Phase 2 of PILOT (see .pilot/pilot-process.md): the architect and tech lead work an already-formalized level:story together, in dialogue (.pilot/pilot-link-agent-dialogue.md) rather than one handing a finished decomposition to the other — split it into dev-sized tasks (mandatory for type:feature, a judgment call for type:tech) and write each resulting ticket's technical spec, in one continuous pass, so a spec-time feasibility concern can reshape the split immediately instead of surfacing later. For type:feature, the PM also checks the proposed type:feature tasks (excluding tech/e2e siblings) against the story's acceptance criteria before finalizing. Given a level:task instead (always a standalone type:bug ticket, dev-sized by nature — the only level:task that ever waits on its own spec, since a freshly split-off task is always spec'd in the same pass that creates it), just writes its spec directly, no split decision. Given no ticket at all and a raw need (--tech, or --bug, explicit or auto-detected), originates it first — architect alone classifies/drafts it, then the same pass splits/specs it — the one way a type:tech or type:bug ticket ever reaches this phase without a prior /pilot-discovery pass (type:bug never goes through /pilot-discovery at all). Also records dependencies (a prerequisite type:tech/type:bug ticket, and/or between split tasks), decides status:wont-do, or flags needs-human. Defaults to pair mode (walks the decomposition/spec with a live human, checkpointing into the ticket); --auto finalizes straight away (for a scheduled cron Routine, which has no live human) — except the no-ticket entry, which needs a human's raw input and so is pair in practice regardless. Also resumes a needs-human ticket once cleared, resumes a mid-pair ticket with --resume <issue number>, reclaims a type:feature story at status:qa/status:in-qa for a new round (status:in-spec then status:split — refuses on status:done, which needs a new ticket), and with no argument picks up fresh or resumable work (e.g. --auto from a cron Routine), skipping on-hold or unresolved-'Depends on #N' tickets. An optional --multi <N> runs an N-instance ensemble in dialogue with itself instead of one, converging with no fixed round cap, escalating to needs-human only on a genuine, unresolved disagreement (see .pilot/pilot-link-agent-dialogue.md). Use for splitting/speccing an already-created story, speccing a standalone task/bug, or originating and speccing a standalone technical need or bug report from scratch."
argument-hint: "<issue number, optional — picks the next fresh/resumable status:backlog ticket if omitted> [--auto] [--multi [N]] | <issue number> --resume | --tech <raw need> | --bug <raw need>"
---

# PILOT — Phase 2: Spec

Read `.pilot/pilot-process.md` first — it's the source of
truth for labels, states, and the claim protocol; this skill covers only the
mechanics of running this phase. Read `.pilot/pilot-link-agent-dialogue.md` too — it
covers the architect/tech lead turn-taking mechanic this skill relies on throughout, and
what `--multi` means here.

## Steps

1. Determine the input:
   - `--tech <raw need>` or `--bug <raw need>` (explicit), or free text with no issue
     number that auto-detects as one of these (infra/CI/security/deployment/migration/
     optimization work with no defect implied → tech; a report of already-shipped
     behavior that's broken/regressed → bug; genuinely ambiguous, or reads as a product
     idea instead → say so and point at `/pilot-discovery`) → **no-ticket entry**, skip to
     step 1a.
   - `--resume <issue>`: must be `status:in-spec`, assigned, **no** `needs-human`/
     `on-hold` — a ticket left mid-pair. Follow `.pilot/pilot-process.md` §4 "Resuming
     an orphaned claim", skipping step 2's claim. If it
     doesn't match, report and stop.
   - No `--resume`, `status:in-spec`, **no** `needs-human`/`on-hold`, carrying
     `can-resume` → a resume, not a fresh claim. Follow
     `.pilot/pilot-process.md` §4 "Resuming a `needs-human` ticket", skipping step 2's
     claim.
   - No `--resume`, `status:in-spec`, assigned, **no** `needs-human`/`on-hold`, and
     no `can-resume` → looks like a ticket left mid-pair. Report and ask the human to
     re-run with `--resume`, or add `can-resume` themselves.
   - `status:in-spec` still carrying `needs-human` or `on-hold` → not resolved yet,
     report and stop.
   - Given an issue number that's a `level:epic` currently carrying open
     sub-issues → don't resolve the number itself; search its whole sub-issue tree for
     this phase's own ordinary bare-pool candidates instead
     (`.pilot/pilot-link-epic-descent.md`), and claim/resolve whichever one that search
     finds — never this step's other bullets, which this search doesn't reach into.
     Nothing found anywhere in the tree → report nothing to do for this ticket. (A
     `status:split` `level:story` given directly, still open, is never intercepted this
     way — see "Otherwise" below.)
   - GitHub issue **closed** (`status:done`/`status:wont-do`) → terminal, refuse
     (`.pilot/pilot-process.md` §2 "Re-scoping a `type:feature` story after its split is
     done"): report it's closed, point at opening a new ticket instead. Never claim it.
     Check the issue's actual open/closed state, not just the `status:` label —
     `status:qa`/`status:in-qa` below are still *open*, which is what distinguishes them
     from this.
   - `status:qa` (unclaimed) or `status:in-qa` (assigned, any owner) → a valid
     re-scope entry point, not an ordinary claim (`.pilot/pilot-process.md` §2
     "Re-scoping a `type:feature` story after its split is done"). Read the ticket, its
     parent Epic (if linked), and its tasks (which are `status:done`, including the
     original e2e task) for context, comment why (new scope found; any `status:in-qa`
     session set aside), then claim it — overwrite the assignee if any (same non-conflict
     exception a `status:changes-requested` reclaim gets, below) and set `status:in-spec`
     — before continuing to step 3, passing that extra context alongside the ticket
     body. Skip step 2, already claimed here.
   - Otherwise → an existing, open ticket: a `level:story` (`type:feature`/`type:tech`)
     being split/specced or re-split/re-specced, or a `level:task` (always a standalone
     `type:bug`) just needing a spec. Read it
     (`mcp__github__issue_read`) plus its parent Epic (if linked) and anything referenced
     via "Blocks #M"/"Depends on #N" or a sub-issue relationship, as context. If
     `level:epic`, there's nothing to work on the epic itself — stop and point at its
     stories.
   - No argument → per `.pilot/pilot-process.md` §4 "Picking the next ticket...": the
     merged pool of unclaimed `status:backlog` (fresh — a story from Discovery, or a
     standalone bug/tech ticket) and `status:in-spec` carrying
     `can-resume` (resumable — a mid-pair ticket is never in this pool, only reachable
     via `--resume <issue>`), highest `priority:` then oldest first. What a scheduled
     cron Routine drives with `--auto` (`.pilot/pilot-process.md` §4 "Scheduled
     sweeps").
1a. **No-ticket entry** (`--tech`/`--bug`, explicit or auto-detected): call `Agent` with
    `subagent_type: "pilot-architect"` alone — read the matching task doc,
    `.pilot/pilot-task-formalize-tech-need.md` (`--tech`) or
    `.pilot/pilot-task-formalize-bug-report.md` (`--bug`, alongside
    `.pilot/pilot-link-bug-tickets.md` in full), and pass its content with the raw need.
    - `--tech`: out of scope / not actionable → report, create nothing. Otherwise →
      always exactly one `type:tech` `level:story` (standalone — never grouped under an
      epic, there is no `type:tech` epic, `.pilot/pilot-process.md` §2 "Three levels") —
      a need too big for one story is never several stories here, that's what step 3's
      own split judgment call is for, right after this same story is approved. Create it
      `status:draft`, assigned, its own initial `priority:`.
    - `--bug`: not a bug / not actionable → report, create nothing (same as
      `.pilot/pilot-task-formalize-bug-report.md`'s own step 1). Genuine → create it
      directly, `type:bug` + `level:task` + `status:draft`, assigned, its own
      `priority:` (`.pilot/pilot-link-bug-tickets.md`) — always exactly one, a bug never
      splits.
    Either way, exactly one ticket exists at this point. Show the draft to the human
    (pair — a raw need has no live human otherwise) and refine in place, writing each
    round into the ticket, until approved — same discipline as `/pilot-discovery`'s own
    drafting loop (`.pilot/pilot-process.md` §4 "Interaction modes"). Once approved, this
    session already holds the claim (assignee set at creation) — continue directly to
    step 3 with this same ticket, skipping step 2 (already claimed) and step 1's pool
    selection entirely: `status:draft` → `status:in-spec` at this point, no
    `status:backlog` stop in between, since this same run is about to split (if the
    `--tech` case needs it) and spec it. Never combine with
    `--resume` (a fresh idea, not a paused session) — `--resume <issue>` instead recovers
    a no-ticket entry left mid-pair, `status:draft`, the same as `/pilot-discovery`'s own
    resume.
2. **Claim** the ticket per `.pilot/pilot-process.md` §4: set assignee + `status:in-spec`,
   re-read to confirm the claim held. (Skipped for the no-ticket entry, step 1a, and for
   a resume/re-scope case per step 1's own bullets.)
3. Branch on `level:`:
   - **`level:story`**: run the architect+tech lead dialogue
     (`.pilot/pilot-link-agent-dialogue.md`) — or, with `--multi <N>`, an N-instance
     ensemble of the same dialogue, converging with no fixed round cap before continuing.
     Call `Agent`, alternating `pilot-architect` and `pilot-techlead` turns. Read
     `.pilot/pilot-task-scope-story.md` (architect's split-decision duty) and
     `.pilot/pilot-task-write-spec.md` (tech lead's spec-writing duty) and pass the
     matching one as part of each call's prompt, plus `.pilot/pilot-link-bug-tickets.md`
     in full (the classify/originate mechanic for a prerequisite bug found mid-pass) —
     plus only what this phase needs beyond that: the ticket's current body (including,
     if resuming, the comment thread's resolution per §4), its parent Epic/linked tickets
     if any, pointers to this project's own coding standards/security conventions and
     architecture docs, and, for a `status:qa`/`status:in-qa` reclaim (step 1), which
     existing tasks are already `status:done` from the earlier round, including the e2e
     one. Not the conversation history. **Checkpoint as the dialogue progresses, not only
     once it converges**: this pass can produce a split decision plus several full specs
     before a human ever sees any of it, all before the sub-issues themselves exist to
     write into (step 5) — so keep the *parent story's* current state in **one** scratch
     comment, updated in place (`mcp__github__update_issue_comment` — create it once,
     from then on edit that same comment) each time a piece settles between the two
     personas (the split shape once agreed, then each task's spec as it's drafted), rather
     than posting a new one per milestone: that comment's latest edit is always the whole
     current state, not a trail to reconstruct from piece by piece. Same "write
     immediately, don't hold it in conversation" discipline pair mode already applies
     everywhere else (`.pilot/pilot-process.md` §4 "Interaction modes"), just one comment
     kept current instead of one comment per round. This is what an orphaned-claim
     `--resume` reads (`.pilot/pilot-process.md` §4 "Resuming an orphaned
     claim") if the session dies partway through this step, instead of restarting the
     whole dialogue from nothing. Once step 5 actually applies the result, this scratch
     comment has done its job — see step 5's own note on retiring it.
   - **`level:task`** (always a standalone `type:bug` — never a fresh split-off task,
     which is already spec'd in the same pass that created it, below): no split decision
     to make — call `Agent` with
     `subagent_type: "pilot-techlead"`, reading `.pilot/pilot-task-write-spec.md`, passing
     the ticket's body (for a bug: the architect's original diagnosis, recorded at
     creation) and this project's own coding standards. If the diagnosis doesn't hold up
     against the real code, the tech lead flags `needs-human` per that task doc — never
     re-runs the architect itself; a genuinely wrong bug diagnosis is a human call, not a
     reason to re-open the classify step.
4. The dialogue (or, for a `level:task`, the tech lead alone) returns one of, for a
   `level:story`:
   - `type:tech`: a single scoped-and-spec'd body (no split, with its `priority:`
     reconfirmed or revised), or a set of proposed tasks (split, judgment call) each with
     security/architecture decisions, dependencies, its own suggested priority, and its
     own written spec.
   - `type:feature`: always a set of proposed tasks — one or more dev tasks (each
     its own `type:feature` or `type:tech`, whichever fits — `.pilot/pilot-process.md`
     §2 "`type:` is never inherited"), plus exactly one flagged as the mandatory
     end-to-end-test task, its own `type:e2e` (`.pilot/pilot-link-e2e-tasks.md`),
     dependent on every other task in the set — each with its own written spec. Never a
     single unsplit body.
   - Or, for either: a verdict that it shouldn't be built at all.
   Independently, it may also flag one or more **prerequisite** needs — `type:tech`
   (`.pilot/pilot-process.md` §2 "Prerequisite tech/bug tickets") or `type:bug`
   (`.pilot/pilot-link-bug-tickets.md`) — and whether each is a hard blocker; for a
   `level:task`, just its written spec, or a `needs-human` flag.
4a. **If `type:feature`** (always split, step 4): call `Agent` again with
    `subagent_type: "pilot-pm"`. Read `.pilot/pilot-task-check-split-coverage.md` and
    pass its content as part of the prompt, plus the story's acceptance criteria and
    **only the `type:feature` tasks** — excluding `type:tech`/`type:e2e`
    (`.pilot/pilot-link-e2e-tasks.md` — neither covers a
    criterion: a tech task is an enabler, the e2e task verifies what its
    `type:feature` siblings already cover). If the PM blocks with a gap, feed it back
    to the dialogue and repeat until approved, before step 4b. Runs regardless of
    `--auto`/pair — a validation step, not a human checkpoint.
4b. **Unless `--auto`** (`.pilot/pilot-process.md` §4 "Interaction modes" — pair is
    default; never applies to the no-ticket entry, step 1a, which is pair in practice):
    don't finalize yet. Show the human the proposed split (or not) and each ticket's
    spec — security/architecture decisions, dependencies, any e2e task, wont-do
    verdict, prerequisite need(s) and blocker status, the PM's coverage check if
    run — as a normal reply, wait for their response, feed it back into the dialogue —
    repeat until approved. Keep updating step 3's same scratch comment as each round is
    approved, rather than holding it in-conversation or starting a second comment — this
    is what `--resume` picks back up if the session ends first
    (`.pilot/pilot-process.md` §4 "Resuming an orphaned claim"). Requires a live human;
    a scheduled Routine must pass `--auto`. Once approved, continue to step 4c.
4c. **Final consolidation pass** (`.pilot/pilot-process.md` §4 "Interaction modes"):
    before applying anything, have the dialogue re-read the ticket (and every proposed
    task and its spec, if split) as a whole, not just the latest delta, fixing anything
    that no longer holds together across rounds (an earlier security decision at
    odds with a later one, a spec that no longer matches a since-revised split). Do this
    for both pair and `--auto`.
5. Apply the result (`mcp__github__issue_write`, `mcp__github__sub_issue_write`):
   - No split (`type:tech` `level:story` only, or a `level:task` needing just a spec —
     a `type:feature` story is never this case, step 4): update the ticket body with the
     spec and decisions, set `status:dev-ready`, and write the reconfirmed/revised
     `priority:` (for a `level:story` — still the ticket's own, since it's still the one
     leaf). If step 3 left a scratch comment, edit it down to one concise line (e.g.
     "Spec written, status:dev-ready") — the content now lives in the ticket body itself,
     so the comment's job is done; same retirement as the split case below, just with
     nothing to link to.
   - Split into tasks: create the sub-issues, link to the parent as native
     sub-issues, each labeled `level:task` plus its own `type:` as decided in step 4
     (never the parent's — `.pilot/pilot-process.md` §2 "`type:` is never
     inherited": `type:feature`/`type:tech` for a dev task, `type:e2e` for the
     end-to-end one), its own written spec, and **`status:dev-ready` directly** — split
     and spec happen in the same pass, so a fresh task never stops at an intermediate
     "scoped but not yet spec'd" status. For a
     recorded dependency between two tasks, add a "Depends on #N" line to the
     dependent one's body (`.pilot/pilot-process.md` §2 "Dependencies between tasks of
     the same split" — also how the e2e task's dependencies on every sibling get
     recorded). Set the parent's `status:` to `split` (its `level:` stays
     `level:story`) and remove its own `priority:` label
     (`.pilot/pilot-process.md` §3 — superseded by its tasks'), leave it open and
     unassigned as a tracker. **Retire step 3's scratch comment**: its whole purpose was
     letting a crash mid-dialogue recover the in-progress split/specs before the
     sub-issues existed — now that they do, and hold the real content, edit that same
     comment down to one concise line (e.g. "Split into #A, #B, #C — see each for its own
     spec") instead of leaving the full in-progress draft sitting on the parent.
   - Won't-do (clear-cut only): label `status:wont-do`, close the issue. If not
     clear-cut, add `needs-human` with the reasoning instead (keep
     `status:in-spec`) — don't close it. Never for a `type:bug` ticket past creation —
     that's a human call from here (`.pilot/pilot-process.md` §3 `status:wont-do`).
   - Needs-human (a `level:task`'s diagnosis doesn't hold, or the dialogue can't resolve
     a genuine disagreement, `.pilot/pilot-link-agent-dialogue.md`): add `needs-human`
     with a comment, keep `status:in-spec`.
   - Prerequisite tech ticket(s) flagged (alongside whichever of the above
     applies): write each the same way `.pilot/pilot-task-formalize-tech-need.md`
     describes for a standalone need — its own `level:story`, standalone, never grouped
     under an Epic — but **never** step 1a's live drafting loop or its "continue directly
     to step 3" behavior: no human is here to co-draft this one, and this run stays
     focused on the ticket it already claimed. Create it directly at `status:backlog` for
     a later, separate `/pilot-spec` run — **never** a sub-issue of
     the ticket being worked. Add a "Blocks #M" comment on each new ticket, and a separate
     line in this ticket's body naming it — "Depends on #N" if judged a hard blocker (the
     exact phrase `.pilot/pilot-process.md` §4 "Blocked-by dependencies" mechanically
     gates future phases on), else a plain non-gating reference. Several prerequisites
     means several separate lines, one `#N` each — never combined.
   - Prerequisite bug ticket(s) flagged (alongside whichever of the above applies):
     same idea — write each per `.pilot/pilot-task-formalize-bug-report.md`'s content
     steps, but skip its live-human classification loop (no one's here to confirm it live)
     and create it directly at `status:backlog` —
     `type:bug`, `level:task` — never a sub-issue — same linking rules,
     always a hard blocker here.
   - Either prerequisite case above, when step 1 found a parent Epic for the ticket
     being worked: also `mcp__github__add_issue_comment` on that Epic naming the
     blocking relationship — skip this when the worked ticket has no parent Epic.
6. Report the outcome (ticket(s) split/spec'd, dependencies recorded, prerequisite
   ticket(s) spun out, or closed as won't-do) back to the human.
