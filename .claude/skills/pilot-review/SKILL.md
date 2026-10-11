---
name: pilot-review
description: "Phase 4 of PILOT (see .pilot/pilot-process.md): claims a status:review-ready PR (assignee + status:in-review, .pilot/pilot-process.md §4 'Claim Protocol'), then runs the review agents (PM+tech lead by default for type:feature/type:e2e, tech lead alone for type:tech/type:bug — architect optional in either set, added via --agents) in parallel, and submits a GitHub PR review (Approve/Request changes/Comment, matching the verdict) plus the matching label — status:approved; a pure code-level verdict moves straight to status:changes-requested with no needs-human, so /pilot-dev can reclaim it immediately; a verdict blocking on any judgment call (alone or alongside code-level points) adds needs-human too (status:in-review stays for a pure judgment call, status:changes-requested for a mixed one), gating /pilot-dev's reclaim on a human clearing it. Defaults to pair mode: an all-approve or pure-code-level verdict gets a live human's last look before it's submitted; a verdict carrying any judgment call always submits first (the block must reach GitHub before any resolution, even a live one), then a live human can resolve it right there, submitting one corrected follow-up review. --auto skips every pause, applying each verdict straight through (required for a scheduled cron Routine). --merge, combinable with either mode, merges the PR itself once the final verdict is all-approve — omitted, a human always merges by hand, same as before. Also resumes a ticket once a prior needs-human flag is cleared, recovers a claim orphaned by a crashed run via --resume <issue>, and with no argument sweeps every ready/resumable PR (e.g. --auto from a scheduled cron Routine), skipping any on-hold. An optional --multi <N> runs N instances of each already-selected reviewer role independently on the one claimed PR instead of one — reviewer roles never converse even under --multi, this stays the older skill-mediated mechanism (.pilot/pilot-link-multi-consensus.md) — reconciling each role's own verdict before the usual cross-role aggregation, escalating to needs-human on a genuine, unresolved same-point disagreement. On a reclaimed PR (one that already carries a submitted review), every selected reviewer is automatically also shown that prior review's points, so it confirms what's now fixed instead of starting blind — this is separate from and always happens regardless of --agents below. An optional --agents <pm|architect|techlead>[,...], valid only with an explicit PR/issue number (never a sweep) and never combined with --auto (it always keeps the pair-mode pre-submission pause as a check against picking the wrong role), replaces step 3's default reviewer set with exactly and only the given role(s) — never merged with the default, so keeping a default role alongside an addition means listing it too (e.g. techlead,pm,architect to add the architect on a type:feature PR without dropping pm). architect is always a legal name in the list, on any type:, since it's never part of either default and adding it never counts as narrowing. Any other name must already be in the ticket's own type-based default; narrowing that default (dropping a role it would have included) additionally requires a review already submitted on this PR — typically a reclaim, where the dropped role's prior approval still holds so only the flagged one(s) need to re-run. Refused if a non-architect name isn't part of the ticket's own type-based default, or if narrowing is asked for before any review has been submitted on the PR yet. Use once /pilot-dev has opened a PR ready for review."
argument-hint: "<PR number, or issue number, optional — sweeps ready PRs if omitted> [--auto] [--merge] [--multi [N]] | <issue or PR number> --agents <pm|architect|techlead>[,...] [--merge] | <issue number> --resume [--merge]"
---

# PILOT — Phase 4: Review

Read `.pilot/pilot-process.md` first — source of truth for labels, states, and the claim
protocol (§4) — and `.pilot/pilot-link-review-consensus.md` for the shared `change`/`decision`
tags contract; this skill covers only phase 4's mechanics, including how it picks the
reviewer set (step 3).

Mode: pair by default. Every outcome is checkpointed as a pending GitHub PR review the
moment it's decided (step 7), the same durable-immediately discipline every other
pair-capable phase applies to its own ticket writes. For an all-approve or a pure-`change`
outcome (no `decision` point at all — nothing here needs a human, so no `needs-human`
either), step 8 is then a pre-submission checkpoint; for an outcome carrying `needs-human`
(decision-only, or mixed `change`+`decision`), that outcome always submits immediately
(step 9) and pair's value comes after, in step 10's live resolution. `--auto` skips step
8's pause and step 10's live engagement, applying every outcome straight through (required
for a scheduled Routine, `.pilot/pilot-process.md` §4 "Scheduled sweeps"). `--merge` is a
separate, orthogonal flag (step 12) usable with either mode — without it, this skill never
merges. `--multi <N>` runs N instances of each role already selected in step 3 (PM
and/or architect and/or tech lead, depending on `type:`) instead of one — composing with,
not replacing, this skill's existing per-role parallelism: (2, 3, or a `--agents`-narrowed
count) roles × N instances simultaneous `Agent` calls, reconciled per role before step 6's
cross-role aggregation.
Invalid combined with `--resume`. See `.pilot/pilot-link-multi-consensus.md`. `--agents
<list>` (step 3) is a separate, orthogonal override valid only with an explicit PR/issue
number, never a sweep — combinable with `--multi` or `--merge`, but never `--auto`: it's
inherently a human's own call on which role to drop, so it always keeps step 8's
pre-submission pause as the last check against picking the wrong one.

## Steps

1. Resolve the ticket:
   - Given a PR or issue number (not `--resume`) that currently has open sub-issues (an
     Epic, or a `status:split` story — never a PR itself) → don't resolve the number
     itself; search its whole sub-issue tree for this phase's own ordinary bare-pool
     candidates instead (`.pilot/pilot-link-epic-descent.md`), and claim/resolve
     whichever one that search finds — never this step's other bullets, which this
     search doesn't reach into. Nothing found anywhere in the tree → report nothing to
     do for this ticket.
   - Given `--resume`: must be `status:in-review`, assigned, no `needs-human`/`on-hold`.
     Follow `.pilot/pilot-process.md` §4 "Resuming an orphaned claim" instead of step 2 below
     — already claimed. Check for an existing pending review under this run's own identity
     (`mcp__github__pull_request_read` method `get_reviews`): still pinned to the PR's
     current head commit → skip straight to step 8 with its already-computed outcome
     (step 8 itself routes on/around the pair pause exactly as it would for a fresh run) —
     no need to re-run reviewers. Stale (commit no longer matches) or none found → discard
     any stale one (`mcp__github__pull_request_review_write` method `delete_pending`) and
     resume normally from step 3. Mismatch → report and stop.
   - Given (or pooled) without `--resume`, `status:in-review`, assigned, no
     `needs-human`/`on-hold`, carrying `can-resume` → resume,
     not a fresh claim. Follow `.pilot/pilot-process.md` §4 "Resuming a `needs-human` ticket"
     instead of step 2 — already claimed.
   - Given without `--resume`, `status:in-review`, assigned, no `needs-human`/`on-hold`, no
     `can-resume` → looks orphaned; report and ask the human to re-run with
     `--resume`, or add `can-resume` themselves.
   - `status:in-review` still carrying `needs-human`/`on-hold` → not resolved yet; report
     and stop.
   - Otherwise, or none given → per `.pilot/pilot-process.md` §4 "Picking the next
     ticket...": the given ticket, or the merged pool of unclaimed `status:review-ready`
     (fresh) and `status:in-review` carrying `can-resume` (resumable — handled by
     the second bullet above, not here), excluding `on-hold`, ordered by highest
     `priority:`, then a ticket named in another open ticket's "Blocks #M" before one that
     isn't, then oldest first (`mcp__github__search_issues`). `status:changes-requested`
     tickets are never in this pool — `/pilot-dev` reclaims those (§4).
2. **Claim** it (fresh case only, above) per `.pilot/pilot-process.md` §4: assignee +
   `status:in-review`, re-read to confirm. If the assignee changed (race lost), stand down
   and return to step 1 for a different candidate (bare pool only — report nothing to do if
   a specific ticket was requested).
3. Read the ticket's linked issue and **its own** `type:` label (never a parent's,
   `.pilot/pilot-process.md` §2 "`type:` is never inherited") to pick the **default**
   reviewer set — the architect is never part of either default, added only via
   `--agents` below:
   - `type:feature` or `type:e2e` → `pilot-pm`, `pilot-techlead`
   - `type:tech` or `type:bug` → `pilot-techlead` alone (no PM)
   - Independent of `--agents`, check whether this PR already carries at least one
     submitted review (`mcp__github__pull_request_read` method `get_reviews`) — true
     whenever this round follows a reclaim fix (`status:changes-requested` →
     `status:review-ready`, `.pilot/pilot-process.md` §4 "Reclaiming a
     `status:changes-requested` ticket"), never true for a PR's first review round. None
     found → nothing further to fetch here. Found → fetch the latest submitted review's
     full body (every point, change and decision alike, marked which is which) to pass to
     every reviewer this round in step 5 — see there and
     `.pilot/pilot-link-review-consensus.md` for how it's used. This check also feeds
     `--agents` below, when given.
   - Given `--agents <list>` (only valid with an explicit PR/issue number, never a sweep)
     → `<list>` is always the **complete, exact** reviewer set for this round — never
     merged with the type-based default, so keeping a default role alongside an addition
     means listing it too (e.g. `--agents techlead,pm,architect` on a `type:feature` PR
     keeps both defaults and adds the architect; `--agents architect` alone means *only*
     the architect reviews this round). Validate each name: `architect` is always legal,
     on any `type:`, regardless of prior-review state — the human's own call that this
     PR's architecture warrants a dedicated look. Any other name (`pm`, `techlead`) must
     already be in the ticket's own default set above (never `pm` on
     `type:tech`/`type:bug` — invalid combination, refuse and report). Then check whether
     `<list>` actually **narrows** the default — drops a role the default would have
     included (adding `architect` on top of the full default is never narrowing, whatever
     else `<list>` contains) — only narrowing needs the prior-review check above to have
     found one: none found → refuse and report, there's no prior round for the dropped
     role's approval to still be holding, so this ticket needs the full default set at
     least once first. Once every name validates, use `<list>` exactly, instead of the
     type-based default.

   Absent `--agents`, together the default set covers every dimension phase 4 checks for
   that `type:`: PM — product fit / e2e-flow validation; tech lead — spec conformance and
   code quality. The architect's own architecture/security conformance check is covered
   by construction instead — it already cross-examined the split and wrote/reviewed the
   spec in dialogue with the tech lead at Spec time (`.pilot/pilot-process.md` §4 "Agent
   dialogue") — so its phase-4 pass is worth adding only when this PR's own diff raises a
   fresh architecture/security question the tech lead's own conformance check doesn't
   settle, not by default on every PR. If a future edit changes what any agent's file
   checks, re-verify the default set still adds up to full coverage for the case where
   nobody thinks to add the architect. All
   selected reviewers run **in parallel, fully independent of each other** — none sees
   another's verdict — in both pair and `--auto`; nothing later in the run reopens that
   isolation, not even the one pair checkpoint.
4. Once this project has a CI workflow covering the affected area, check it's green on the
   PR's head commit before proceeding — red/pending CI is an automatic `change`-tagged
   block (step 6). Until then, this is a no-op and the tech lead's own re-run in step 5 is
   the only safety net.
5. Call the `Agent` tool once per reviewer — or, with `--multi <N>`, N times per
   reviewer role — **in parallel** (every role's every instance launched together,
   `subagent_type` set to that persona per step 3's independence guarantee). Pass every
   reviewer
   `.pilot/pilot-link-review-consensus.md` in full — the shared verdict format and tagging
   rule, identical for all three, one canonical statement instead of restated per persona
   — plus the matching task doc for what's specific to that persona:
   `.pilot/pilot-task-review-product-fit.md` (PM),
   `.pilot/pilot-task-review-architecture.md` (architect),
   `.pilot/pilot-task-review-spec-conformance.md` (tech lead) — the persona file itself
   (`.claude/agents/pilot-*.md`) carries only identity now, these two together are what
   tell each reviewer what to check and how to report it. Pass only
   what each needs beyond that: the PR diff/description plus, for the PM, the linked
   story's acceptance criteria and any UI/UX description or attached mockup (and its own
   spec, for `type:e2e`); for the architect, its recorded decisions; for the tech lead,
   its own spec (it re-runs validation on the PR branch itself). When step 3 found a prior
   submitted review on this PR — whether reached via step 1's needs-human-resume branch or
   an ordinary reclaim after a pure-`change` fix — also pass its full body (every point,
   change and decision alike, marked which is which) to every reviewer this round, plus,
   if present: whatever's in the PR's comment thread after it — a specific reply resolving
   a `decision` point, or "no reply — treat as approved as proposed" if none
   (`.pilot/pilot-process.md` §4 "Resuming a `needs-human` ticket") — and the dev's own
   reclaim summary comment on the ticket's *issue* (not the PR,
   `.pilot/pilot-process.md` §4 "Reclaiming a `status:changes-requested` ticket").
   `.pilot/pilot-link-review-consensus.md` has how each reviewer is expected to use this
   instead of starting blind.
5a. **Per-role reconciliation** (only when `--multi` ran more than one instance of a
    role): for each such role, reconcile its N verdicts yourself — no further `Agent`
    call — into that role's single point-list per `.pilot/pilot-link-multi-consensus.md`
    — a point only some instances raised is
    coverage, union it in; only two instances reaching opposite judgments about the
    identical point is genuine disagreement, retried once (that role's N instances again,
    that point in context) — still unresolved after that retry → turn it
    into one `decision`-tagged
    point in that role's own list, quoting every differing position verbatim, and let
    step 6's ordinary tag-based aggregation carry it from there (`needs-human` follows
    automatically, same as any other `decision` point — no separate escalation path).
    Continue to step 6 with each ensembled role's single reconciled point-list; a role
    that didn't run as an ensemble passes its one verdict through unchanged.
6. Collect the verdicts. Aggregate into exactly **one** outcome
   (`.pilot/pilot-link-review-consensus.md`):
   - All blocking points tagged `decision`, none `change` → every point, grouped by
     reviewer, `needs-human` added (`status:in-review` stays — this ticket re-enters the
     resumable half of step 1's pool once cleared, unless step 10 resolves it live first).
   - At least one blocking point tagged `change` (a step-4 CI/validation failure always
     counts) and at least one tagged `decision` → every point (marked which is which),
     `needs-human` added, move to `status:changes-requested` (`/pilot-dev` reclaims once
     cleared — the `decision` point(s) are what it's waiting on).
   - At least one blocking point tagged `change`, none tagged `decision` → every point,
     move to `status:changes-requested`, **no** `needs-human` — nothing here needs a
     human, so `/pilot-dev` may reclaim it immediately.
   - All approved → move to `status:approved`.
7. **Checkpoint it**: create a pending GitHub PR review with the outcome's full body
   already written, no `event` yet (`mcp__github__pull_request_review_write`, method
   `create`, `commitID` pinned to the PR's current head) — durable immediately, so an
   orphaned run recovers it (step 1) instead of redoing steps 3-6.
8. **Pair (default) vs `--auto`** — never for an outcome that carries `needs-human`
   (decision-only, or mixed `change`+`decision`): `.pilot/pilot-process.md` §3's
   `needs-human` rule requires the block to reach GitHub before any resolution, even a live
   one, so either of those always proceeds straight to step 9, same as `--auto`. For the
   all-approve outcome or a pure-`change` outcome (no `decision` point at all), pair mode
   pauses here — nothing for a human to decide (an approval or a code-level fix isn't a
   judgment call), but still worth a last look before it goes to GitHub: show the outcome
   from step 6, wait for confirmation, then proceed to step 9. `--auto` skips this pause and
   proceeds immediately either way.
9. Submit step 7's pending review (`mcp__github__pull_request_review_write`, method
   `submit_pending`) plus the matching label — the outcome's `event`
   (`.pilot/pilot-link-review-consensus.md`):
   - All approved → `event: APPROVE`; body names exactly which role(s) approved this
     round (every role in step 3's set — all of them, unless `--agents` narrowed it, in
     which case say so explicitly rather than implying full coverage) and, per step 12,
     whether this run also merges or a human still needs to; `status:approved`.
   - At least one blocking point tagged `change`, none tagged `decision` → `event:
     REQUEST_CHANGES`; body lists every point; **no** `needs-human`; `status:changes-requested`
     — `/pilot-dev` may reclaim it right away, no human step needed.
   - At least one point of each tag → `event: REQUEST_CHANGES`; body lists every point
     (marked which is which); `needs-human` added; `status:changes-requested` — `/pilot-dev`
     reclaims once the `decision` point(s) are cleared.
   - Blocking points all `decision` → `event: COMMENT`; body lists every point grouped by
     reviewer; `needs-human` added; `status:in-review` stays.

   If GitHub rejects an `APPROVE`/`REQUEST_CHANGES` submission above because this run's
   connected identity is the PR's own author, retry `submit_pending` on that same pending
   review (never a second `create`) with `event: COMMENT` instead — never `needs-human` on
   its own — and still apply that bullet's label/`status:` transition unchanged.
10. **Live resolution of a submitted `needs-human` block** (pair mode only, right after
    step 9 submits it; applies to both the decision-only and the mixed `change`+`decision`
    outcome — never the pure-`change` outcome, which never carries `needs-human` to
    resolve): engage the human live the same way any other phase resolves a live
    `needs-human` block (`.pilot/pilot-process.md` §3 "A human is live in the same session").
    If they answer right there and it changes the outcome, determine the corrected outcome
    (approved, or the remaining `change`-tagged points instead) and submit **one more**
    review — a fresh `create` with `event` set this time, no separate pending step needed
    for a decision made in the same breath — reflecting it, same event mapping as step 9,
    plus the matching label. If GitHub rejects that submission for the same reason as step
    9, retry with another fresh `create` (`event: COMMENT`) instead — no pending review to
    resubmit against here, so a fresh `create` is the retry itself — still applying the
    corrected label unchanged. This keeps GitHub's own review status honest, not just the
    ticket's label; never fold it into step 9's review, that one already went out. If their
    answer doesn't actually clear the block, or `--auto` was given, or nobody answers on the
    spot: leave step 9's review and `needs-human` standing — a genuine async wait like any
    other unresolved `needs-human`, resolved by a later run of this skill.
11. Never submit more than two reviews per PR — step 9's, plus step 10's
    live-resolution correction when it applies — and never one review per reviewer, or,
    with `--multi`, per ensemble instance: step 5a already reconciled each role down to
    one point-list before step 6 ever runs.
12. **Merge, only with `--merge`:** if the final outcome (step 9, or step 10's correction) is
    `status:approved` and `--merge` was passed, merge the PR now
    (`mcp__github__merge_pull_request`, the repository's normal merge method). Without
    `--merge`, or on any other outcome, never merge — a human merges by hand, same as
    before.
13. Report the outcome, including whether this run merged the PR itself. Once a PR is
    merged (by a human, or by step 12), `.github/workflows/pilot-status-on-merge.yml` sets
    `status:done` and runs the cascading-completion check (`.pilot/pilot-process.md` §3)
    — this skill's own job ends at the submitted review(s) (plus, when `--merge` applied,
    the merge itself).
