# PILOT link — End-to-end test tasks

Canonical version of this mechanic — `.pilot/pilot-task-scope-story.md` (Spec, the one
duty that actually proposes the task) restates it inline rather than read it live (no task
doc ever instructs a `Read` of this file), so keep it in sync by hand whenever this changes
(`CLAUDE.md`'s cross-reference-check rule). `.pilot/pilot-task-check-split-coverage.md`
(Spec) restates only the one fact it needs — excluded from the coverage surface — not
the rest. `.pilot/pilot-task-implement.md`/`-implement-e2e.md` (Dev) just cite it in
passing, no restatement. Never by `.claude/agents/pilot-*.md`, which carry only identity
now. See `.pilot/pilot-process.md` §2/§3/§4/§7 for the generic ticket types, labels, claim
protocol, and phase-5 trigger this builds on.

**Mandatory for every `type:feature` story, never for `type:tech`/`type:bug`.** A story is
one feature — one integrated flow a human can exercise — so every `type:feature` story
gets exactly **one** end-to-end-test task alongside its dev task(s), covering every case of
that flow worth exercising (think one test *file*/`describe` block with several cases
inside, not a single assertion), not a judgment call — decided the moment Spec splits the
story, same pass as the rest of the split. `type:tech`/`type:bug` tickets never get one —
they have no user-facing flow to exercise this way.

Title convention: "E2E: <story summary>". A dev-sized ticket like any other, `level:task`
under the story, going through Dev/Review exactly like its siblings, labeled `type:e2e` —
its own type, never stacked with anything else (`.pilot/pilot-process.md` §2 intro). No new
`status:` on the ticket itself.

It depends on **every** other task in the same split, dev and tech alike — not a
judgment-selected subset: one "Depends on #N" line per sibling, so it naturally isn't
claimable by Dev until all of them have merged, and structurally can never be the
ticket that finishes the split before its siblings.

**Excluded from the PM's split-coverage check** (`.pilot/pilot-process.md` §2 "Three
levels"): neither the e2e task nor any `type:tech` sibling implements a criterion — the
e2e task verifies criteria the `type:feature` tasks already cover, and a `type:tech` task
is an enabler, not a delivered piece of product behavior.

**Once the e2e task reaches `status:done`, the whole split is structurally finished**
(every dev sibling necessarily already is, per the dependency above) — this is what
triggers `.pilot/pilot-process.md` §7 "Phase 5 — Human QA" instead of the ordinary cascade
straight to `status:done` (`.pilot/pilot-process.md` §3 "Cascading completion").

Spec (architect+tech lead) writes the e2e task's spec as the test plan itself: which flow,
which existing test tooling/framework this project already uses for e2e, and what its
dependencies having merged now makes exercisable end-to-end. **Dev for an e2e task is
`pilot-e2e`, not `pilot-dev`** — `/pilot-dev` (the skill) reads the `type:e2e` label before
calling `Agent` and picks the `pilot-e2e` persona instead, precisely so the agent
implementing it gets a context already tailored to writing a test against real,
already-merged integration points rather than generic implementation instructions.
`pilot-e2e` writes the test against that already-merged behavior — the ticket cannot be
marked done with a test that doesn't actually pass. If running it surfaces a genuine
defect in that already-merged code rather than a gap in the e2e ticket's own scope, that's
a bug discovered mid-implementation — see `.pilot/pilot-link-bug-tickets.md`.
