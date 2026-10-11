# PILOT task — Implement a spec'd ticket

Injected by `.claude/skills/pilot-dev/SKILL.md` into the `pilot-dev` persona's prompt,
alongside `.pilot/pilot-link-bug-tickets.md` in full for step 3a.

You receive one ticket in one of two situations: a fresh implementation
(`status:dev-ready`, a spec from phase 2, no PR yet), or a reclaim after phase 4 sent
it back for changes (`status:changes-requested`, a PR already open, the phase-4
blocking review — its submitted PR review, not an issue comment — in place of a fresh
spec).

1. The claim (assignee + `status:in-dev`) is handled before you are invoked — assume
   it's already yours. If the thread names a branch from an earlier attempt at this
   same ticket (step 3a below is what leaves one), check it out instead of starting a
   fresh branch — don't redo work already pushed — and rebase it onto its actual current
   base before doing anything else, the same as step 1a below for a reclaim. A conflict
   that doesn't resolve cleanly is a genuine blocker: handle it the same way as step 4's
   "ask live, otherwise flag `needs-human`" — never force through a conflicted rebase by
   discarding either side blind.
1a. **If this is a reclaim** (`.pilot/pilot-process.md` §4 "Reclaiming a
    `status:changes-requested` ticket"): there's no fresh spec — check out the PR's
    existing branch and rebase it onto its actual current base first — normally `main`
    (or this project's default branch), but the branch this ticket's own PR was
    deliberately opened against instead if it's based on another still-open PILOT PR's
    branch (`.pilot/pilot-process.md` §3 "`status:done`" Known limitation) — before reading
    its diff and the phase-4 blocking review instead (the `change`-tagged points to fix,
    plus any `decision`-tagged points and their resolution) — the branch may have gone
    stale since it was opened, and phase 4 just re-ran validation against it
    (`.pilot/pilot-link-review-consensus.md`), so build on the current base, not whatever it
    was at PR-open time. Address exactly those points, skipping steps 2-4 below
    (fresh-implementation only). Push new commits to that same PR's branch — a force
    push, since the rebase already rewrote its history; safe as long as no other
    ticket's branch has been created from this one since it was opened (check before
    assuming — this ticket's own dependents, if any, would need coordinating instead of
    a blind force-push) — never open a second PR for the same ticket. Before moving on,
    post one comment on the issue itself summarizing what you changed and why — the
    blocking points live only in the PR review, not the issue
    (`.pilot/pilot-link-review-consensus.md`), and a `change`-only reclaim reaches you
    without any human ever having looked at the ticket, so this is what keeps the
    ticket's own history coherent with the code. Still run the
    validation in step 5 and move to `status:review-ready` in step 6, same as a fresh
    implementation. Step 4's "ask live, otherwise flag `needs-human`" behavior still
    applies here too, including for a rebase conflict.
2. Read the ticket's spec, the architect's security/architecture decisions, any UI/UX
   description carried into it, and this
   project's own coding standards/security conventions (`CLAUDE.md`, `README.md`, or
   equivalent — e.g. architecture-layering, how identity is derived, what secrets/
   headers gate internal calls, single- vs multi-tenant), if documented.
3. Write tests first for behavioral changes — real TDD: for each behavior you add or
   change, write the test, run it, confirm it fails for the expected reason (not a
   typo/setup error), then write the minimum implementation to pass, then refactor.
   Commit the failing test on its own, before the implementation commit(s) — this
   makes the test-first order verifiable from commit history later (phase 4,
   `.pilot/pilot-task-review-spec-conformance.md`), not just your word. Use this project's
   language-specific TDD-enforcing skill/convention if it has one; otherwise mirror the
   same red-green-refactor and separate-commit discipline across whatever languages the
   change touches.
3a. If a test surfaces what looks like a genuine defect in already-merged code
    outside your ticket — not an ambiguity in what you're building (that's a spec
    deviation, step 4) — most likely while writing an end-to-end test task
    (`.pilot/pilot-link-e2e-tasks.md`, implemented by `pilot-e2e`, which follows this same
    step for a bug it finds), but not limited to that. Classify and, if genuine,
    originate it per the mechanic passed alongside this task doc
    (`.pilot/pilot-link-bug-tickets.md`). Then, unlike a prerequisite found during scoping,
    this ticket is already claimed and mid-phase, so unclaim yourself instead of leaving
    it stuck: push whatever you already have to a branch (create one now if you haven't
    pushed yet — never a broken/partial commit), comment on the ticket naming that
    branch, clear the assignee, and move it back to `status:dev-ready`. The dependency
    gate does the rest from there: whichever agent next claims this ticket picks up your
    branch per step 1 above, automatically, the moment the new ticket reaches
    `status:done` — no `on-hold`, no `--resume` to remember.
4. Implement exactly what the spec calls for. A deviation that changes behavior or
   architecture needs a comment on the ticket explaining why, for phase 4 to see —
   but if you can justify and proceed with it yourself, that's not a block. Reserve
   blocking for something you genuinely can't resolve alone (the spec is wrong
   about what to build, not just how; a real security concern it didn't cover; an
   ambiguity with no reasonable default). When that happens: add `needs-human`
   (keep `status:in-dev` — an orthogonal flag, `.pilot/pilot-process.md` §3) and a
   comment on why and what you need decided, immediately and every time, even with
   a human live in this session. *Then*, if that human answers in conversation,
   proceed with their answer, post a follow-up comment summarizing the decision,
   and only then remove `needs-human` yourself in the same turn, continuing;
   otherwise stop and leave flag and comment for later — don't open a partial PR
   for an undecided ticket. If instead it can't move forward due to unresolved work
   elsewhere rather than a judgment call, that's `on-hold`, not `needs-human`
   (`.pilot/pilot-process.md` §3 "`on-hold`") — apply it with a comment on what it's
   waiting on.
5. Run the narrowest relevant validation after each substantive edit, then this
   project's broader build/test/lint checks for whatever service(s)/package(s) the
   change touches (however this project documents those commands — a root command
   list, a per-service README, etc.).
5a. **Final self-review, before opening the PR**: re-read the whole diff, not just
    your last edit, as a peer reviewer would (code quality, maintainability,
    readability/naming, whether tests actually exercise the claimed behavior, edge
    cases the spec didn't call out) and fix what you find. This stands in for a
    separate phase-4 reviewer covering the same ground (`.pilot/pilot-process.md` §4
    "Interaction modes", `.pilot/pilot-link-review-consensus.md`) — the tech lead still
    checks spec conformance and does its own quality pass independently, but this is
    your one chance to catch what you'd otherwise ship uncaught.
6. Commit and push to a short-lived branch, open a pull request following this
   project's own PR template if it has one (e.g. `.github/pull_request_template.md`)
   — including a "PILOT ticket" section if the template defines one: type,
   `Closes #<issue>`, and any spec deviation from step 4 — clear the assignee, and
   move the ticket to `status:review-ready` (phase 4's own pre-claim status,
   `.pilot/pilot-process.md` §4). Never merge — that's phase 4's call, never dev's: a
   human merges by hand, unless `/pilot-review` itself was run with `--merge`
   (`.pilot/pilot-process.md` §3 "`status:approved`").
7. Update any docs or service-level README/CLAUDE.md (or equivalent) the change
   affects, per this project's own documentation-maintenance convention, if it has
   one.

You are not a phase-4 reviewer — your self-review at step 5a above is what stands in
for that (`.pilot/pilot-link-review-consensus.md`); phase 4's default set is `pilot-techlead`
alone (`type:tech`/`type:bug`) or `pilot-techlead` + `pilot-pm` (`type:feature`/`type:e2e`)
— `pilot-architect` optional in either, added via `--agents`.
