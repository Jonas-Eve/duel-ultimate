# PILOT task — Split an already-created story

Injected by `.claude/skills/pilot-spec/SKILL.md` into the `pilot-architect` persona's
prompt, alternating turns with the tech lead's own `.pilot/pilot-task-write-spec.md`
(`.pilot/pilot-link-agent-dialogue.md`), alongside `.pilot/pilot-link-bug-tickets.md` in
full for step 5's "Prerequisite (bug)" case. You're not deciding the split in isolation
for the tech lead to spec afterward — a spec-time feasibility concern from its side of
the dialogue can send your own split proposal back for a rethink, in the same pass.

**Challenging it first**: push back where it matters before deciding how to split —
ambiguous requirements, technical risk, anything touching this project's own
identity/security/architecture boundaries, or anything out of step with its recorded
system design, network topology, or multi-tenancy model (your own identity's project-doc
habit is what surfaces these). If something needs a human decision to scope responsibly,
say so — don't guess and move on.

You receive an existing `type:feature` or `type:tech` `level:story` (its current body —
already scoped once before, if this is a re-scope) — never `type:bug`: a bug is always
`level:task` directly and never split, so it's spec'd on its own by the tech lead alone,
without this dialogue (`.pilot/pilot-process.md` §2 "Three levels"). You may also be given
its parent Epic and linked tickets for context; feel free to read further related tickets
yourself (a sibling story under the same Epic, something referenced via "Blocks #M"/"Depends on
#N") — e.g. to avoid proposing a prerequisite a sibling ticket already covers.

**If this is a re-scope of a story whose split already finished once** (the skill
claimed it from `status:qa`/`status:in-qa` into `status:in-spec` before calling you —
the one deliberate backward transition in the state machine, `.pilot/pilot-process.md`
§2 "Re-scoping a `type:feature` story after its split is done") — you're also given
which tasks from the earlier round(s) are already `status:done`, including the
original e2e task(s). Steps 4/4a below still apply, scoped to *this round only*:
propose new tasks for the new work, plus exactly one new `type:e2e` task extending the
existing e2e coverage rather than duplicating it (never reopen or touch the
already-done one), and run the PM coverage check (step 4a) against this round's new
`type:feature` tasks only — the earlier ones were already checked when proposed.

1. The claim (assignee + `status:in-spec`) is handled before you are invoked — assume
   it's already yours.
2. Challenge it (above).
3. Decide whether it needs building at all: if challenging it convinces you it's out
   of scope, a duplicate, or superseded, propose `status:wont-do` instead of scoping
   it — but only when clear-cut. If it's a judgment call, flag `needs-human` instead
   with a comment stating why and exactly what you need decided. This is a
   phase-2-only option — once a ticket has a spec or code, killing it is always a
   human call (`.pilot/pilot-process.md` §3). If instead the ticket simply can't move
   forward yet because it depends on unresolved work elsewhere (a global
   restructuring, another ticket/Epic not yet done) — not a decision needing someone's
   judgment — that's `on-hold`, not `needs-human` (`.pilot/pilot-process.md` §3
   "`on-hold`").
4. Decide the split shape — this branches by `type:` (`.pilot/pilot-process.md` §2
   "Three levels"; `.pilot/pilot-link-e2e-tasks.md`):
   - **`type:tech`**: splitting is still your judgment call — fine to leave it as one
     ticket through phases 3-4, in which case reconfirm or revise the `priority:` set when
     the story was created (Discovery, if grouped with a feature; this same phase's own
     no-ticket entry, if standalone — `.pilot/pilot-process.md` §3, it stays the ticket's
     own, since it's still the one leaf). If you do split, propose one dev-sized task per unit,
     along vertical slices (each a coherent, ideally independently shippable/testable
     piece) rather than by technical layer — a front-end-only or back-end-only task is
     rarely reviewable or testable alone, unless the two are genuinely decoupled (e.g.
     a backend API meant to be consumed later, independently). No e2e task either
     way — skip step 4a.
   - **`type:feature`: always split, never a single unsplit ticket.** Propose one or
     more dev tasks (one is enough for a small story — the only real judgment call
     left is *how many*, and each one's own `type:`, never *whether* to split at all),
     along the same vertical-slice principle above, **plus** go to step 4a for the
     mandatory e2e task. A dev task's own `type:` is whatever fits *that task*, not
     automatically `type:feature`: a technical enabler the feature tasks depend on (a
     migration, shared infra) is its own `type:tech` task in the same split, free to
     block the feature tasks the ordinary way (step 5 "Between tasks of the same
     split").
4a. **`type:feature` only**: propose exactly one additional task titled
    "E2E: <story summary>" (or "E2E: <story summary> (round N)" for a re-scope round,
    per the note above) — covering every case of the story's flow worth exercising,
    not just a happy path — labeled `type:e2e` (its own type, not stacked with
    anything else). It depends on **every** other task from step 4, dev *and* tech
    alike, from *this round* ("Depends on #N", one line per sibling — not a subset,
    and never the earlier round's already-`status:done` tasks,
    `.pilot/pilot-link-e2e-tasks.md`). Exclude it, and any `type:tech` task from step 4, from
    the PM's coverage check below — tell the PM to check coverage against the
    `type:feature` tasks only, since neither a tech enabler nor the e2e ticket itself
    implements an acceptance criterion.
5. Decide dependencies, split or not:
   - **Prerequisite (tech)** — the ticket (or one of its proposed tasks) depends on
     technical work that isn't part of it at all (infra, CI, a shared library, a
     migration) — no defect implied; applies to a `type:feature` ticket just as much
     as a `type:tech` one. Write it exactly the way `.pilot/pilot-task-formalize-tech-need.md`
     describes for a standalone need — always exactly one story, standalone, never
     grouped under an Epic (there is no `type:tech` epic; if it turns out too big for one
     story, that's its own future Spec pass's split judgment call to make, not something
     to solve by creating several stories here) — **never** as a
     sub-issue of
     the ticket you're scoping (`.pilot/pilot-process.md` §2 "Prerequisite tech/bug
     tickets"): that would make it `level:task` and wrongly tie it to this ticket's
     split-tracking lifecycle instead of its own Spec/Dev/Review phases. Land it at
     `status:backlog` for a later `/pilot-spec` run — never the live drafting loop that
     doc's own no-ticket entry adds, that's for a human reporting a need directly. State
     whether it's a
     **hard blocker** ("Depends on #N", `.pilot/pilot-process.md` §4 "Blocked-by
     dependencies") or a plain non-gating reference.
   - **Prerequisite (bug)** — while scoping, you instead run into something that
     looks like a concrete *defect* in already-shipped code outside the ticket's own
     scope, not an ambiguity within it. Classify and, if genuine, originate it per
     the mechanic passed alongside this task doc (`.pilot/pilot-link-bug-tickets.md`)
     — then finish scoping this ticket normally afterward: it isn't claimed by
     Dev yet, so recording the dependency is enough, no unclaiming needed.
   - **Between tasks of the same split** — if two of the tasks you're proposing
     depend on each other (e.g. a front-end one consuming an API a back-end one
     creates), record it the same way ("Depends on #N" on the dependent one,
     `.pilot/pilot-process.md` §2 "Dependencies between tasks of the same split").
6. Label every task `level:task` (never on the story itself once it's `status:split` —
   the story keeps its own `level:story`, but loses its own `priority:` label here,
   superseded by its tasks' own, `.pilot/pilot-process.md` §3) and its own `type:` per
   steps 4/4a above — never copied from the story, `.pilot/pilot-process.md` §2 "`type:` is
   never inherited". Set a `priority:P0/P1/P2` on each: for a `type:tech` story's tasks,
   or a `type:tech` enabler task inside a `type:feature` split, use the technical framing
   (`.pilot/pilot-process.md` §3 — P0 closes a real security/correctness/safety-net gap, P1
   solid value but not urgent, P2 nice-to-have or conditional); for a `type:feature` dev
   task, default to the story's own initial (PM-set) priority unless this specific task's
   urgency genuinely differs from its siblings. Adopt this project's own priority
   convention instead, if it already has one that differs.
7. Write explicit **Security decisions** and **Architecture decisions** sections into
   each ticket's body — the concrete choices made (or the reason none were needed),
   not just a restatement of the requirement. If the story itself carries an
   **Architecture notes** section from Discovery
   (`.pilot/pilot-task-anticipate-architecture.md`), start from what it already
   anticipated rather than deciding blind — confirm it still holds, sharpen it into a
   concrete decision, or explicitly revise it if the split itself changes the picture;
   don't silently ignore it either way. A `type:tech` story reaching Spec via its own
   no-ticket entry (`.pilot/pilot-task-formalize-tech-need.md`) carries no such section —
   there was no Discovery pass — so this is the first time its architecture gets decided.
   These decisions are what the tech lead's own spec
   (this same dialogue) builds against, and what you check conformance against in Review
   if added (`.pilot/pilot-task-review-architecture.md`).
   If the story carries a UI/UX description or a mockup attached as a comment
   (`.pilot/pilot-task-write-story.md`), also carry the relevant part into each task that
   actually implements that UI — Dev and Review read only the task's own body, not the
   parent story, so it goes stale the moment you don't.
