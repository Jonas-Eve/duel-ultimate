---
name: pilot-qa
description: "Phase 5 of PILOT (see .pilot/pilot-process.md): human QA for a type:feature story once every task — including its mandatory e2e one — has merged (status:qa). Builds a manual test plan from the story's acceptance criteria and merged tasks, walks a human through testing it live, and reports the verdict. Pair-only, no --auto, never Routine-driven. On a full pass, or once every failure resolves to not-a-bug, sets status:done and closes the issue (reports any not-a-bug finding to the human to raise via Discovery). On a genuine defect, originates a type:bug ticket via PILOT's one shared bug-creation mechanism and unclaims the story itself (back to status:qa) — same mechanics as a bug found mid-implementation in phase 3. An unclassifiable failure gets the standard needs-human flow (label + comment posted immediately, per .pilot/pilot-process.md §3 — resolved and cleared live in the same turn when the human answers right there, since this phase is always pair). Also resumes a ticket left mid-pair with --resume <issue number>, and bare (no argument) picks up the next status:qa (fresh) or can-resume-marked status:in-qa ticket, skipping anything on-hold. Use once a type:feature story's tasks have all merged and it's ready for a human to confirm the shipped behavior."
argument-hint: "<issue number, optional — picks the next fresh status:qa or resumable status:in-qa ticket if omitted> | --resume <issue number>"
disable-model-invocation: true
---

# PILOT — Phase 5: Human QA

Read `.pilot/pilot-process.md` before running this if you haven't already — it's the source
of truth for labels, states, and the claim protocol, and §7 specifically covers this phase;
this skill only covers the mechanics of running it.

## Steps

1. Determine the input:
   - `--resume <issue>`: must be `status:in-qa`, assigned, **no** `needs-human`/`on-hold`
     — a session left mid-pair (`.pilot/pilot-process.md` §4 "Resuming an orphaned
     claim"). Read the ticket and thread to reconstruct which cases were already
     confirmed, claim it (overwrite assignee), skip to step 5 with that state. If it
     doesn't match, report and stop.
   - Issue number without `--resume`, `status:in-qa`, **no** `needs-human`/`on-hold`,
     carrying `can-resume` → a resume, not a fresh claim
     (`.pilot/pilot-process.md` §4 "Resuming a `needs-human` ticket"); skip the claim in
     step 2, it's already claimed. Read the ticket and thread to reconstruct which cases
     were already confirmed and the blocking point's resolution, then skip to step 5 with
     that state, same as the `--resume` bullet above — never restart the test plan from
     step 3/4.
   - Issue number without `--resume`, `status:in-qa`, assigned, **no**
     `needs-human`/`on-hold`, no `can-resume` → looks like a
     session left mid-pair; report and ask the human to re-run with `--resume`, or add
     `can-resume` themselves.
   - Issue number still carrying `needs-human` or `on-hold` → not resolved yet; report and
     stop.
   - Any other issue number: must be `status:qa` (else — `status:split`, `status:done`, a
     type that never reaches `status:qa`, `.pilot/pilot-process.md` §7 "When it fires") and
     carry no open "Depends on #N" (`.pilot/pilot-process.md` §4 "Blocked-by dependencies" —
     applies to an explicitly-given ticket here too, not just the pool) — report which
     ticket it's still blocked on and stop rather than claiming it, if one's still open.
   - No argument → per `.pilot/pilot-process.md` §4 "Picking the next ticket...": merged
     pool of unclaimed `status:qa` (fresh) and `status:in-qa` carrying `can-resume`
     (resumable — a mid-pair ticket is only reachable via explicit `--resume`),
     excluding `on-hold` and any with an unresolved "Depends on #N"
     (`.pilot/pilot-process.md` §4 "Blocked-by dependencies"), highest `priority:` then
     oldest first. No `--auto`, never Routine-driven (`.pilot/pilot-process.md` §4
     "Interaction modes", "Scheduled sweeps").
2. **Claim** it per `.pilot/pilot-process.md` §4: set assignee + `status:in-qa`, re-read to
   confirm the claim held.
3. Gather context: the story's own body (acceptance criteria) and, for each of its dev and
   e2e tasks, the spec (phase 2) and the merged PR (`mcp__github__issue_read`,
   `mcp__github__pull_request_read`) — not the running conversation history.
4. Call the `Agent` tool with `subagent_type: "pilot-qa"`. Read `.pilot/pilot-task-human-qa.md`
   and pass its content as part of the prompt, plus `.pilot/pilot-link-bug-tickets.md` in
   full (the classify/originate mechanic for step 3's "real-bug failures" case — the task
   doc covers the phase-5-specific delta itself), along with that context. The agent
   returns a manual test plan: concrete cases and how to test each.
5. Walk the human through it (pair, always — no `--auto`): show the plan, then go case by
   case — ask them to run one, report what happened, feed that back to the agent, move to
   the next — until every case is reported. Keep one running comment updated in place
   (`mcp__github__update_issue_comment` — create it once, append each case's result to it)
   rather than posting a new comment per case, the same incremental-checkpoint discipline
   every pair-mode phase follows (`.pilot/pilot-process.md` §4 "Interaction modes") — this
   is what makes `--resume` possible if the session ends before a final verdict. Unlike
   Spec's or Dev's scratch checkpoints, **don't collapse this one once the verdict lands**
   — it's the actual test log (what was tried, case by case, and what happened), the one
   place that record exists at all, not work-in-progress toward some other final
   artifact.
6. Apply the agent's verdict:
   - **Approved** (every case confirmed, or every failure resolved to "not actually a
     bug"): set `status:done` and close the issue (`mcp__github__issue_write`) — the one
     phase that sets `status:done` directly, since there's no PR merge here for
     `.github/workflows/pilot-status-on-merge.yml` to react to, and nothing to cascade (a
     `type:feature` story is never itself a task of another `status:split` parent).
     Include any "not actually a bug" finding in the report to the human (step 7) — they
     raise it via Discovery themselves.
   - **One or more real-bug failures**: nothing further to set — the agent already
     originated the `type:bug` ticket(s), directly as `level:task`/`status:backlog`
     (never split, `.pilot/pilot-process.md` §2 "Three levels"), commented naming
     them, cleared the assignee, and moved the story back to `status:qa` itself, the same
     pattern `pilot-dev`/`pilot-e2e` use for a bug found mid-implementation
     (`.pilot/pilot-link-bug-tickets.md`). Takes priority over a same-pass "not actually a
     bug" failure — don't set `status:done` while a real bug is open. It's now an ordinary
     `status:qa` candidate again, gated only by the dependency (step 1's last bullet) — no
     flag to clear, no `--resume` needed; whichever future run claims it starts a fresh test plan.
   - **A failure still unresolved**: the agent already applied `needs-human` with a
     comment listing what failed and why, the moment it couldn't classify it
     (`.pilot/pilot-process.md` §3 "`needs-human`" — label and comment go on immediately,
     never held back on the chance a human answers right away). `status:in-qa` stays.
     Should be rare — the human who can answer is normally right there in step 5, in
     which case the agent posts the resolution and clears the flag itself in the same
     turn, and you'll see the approved/bug outcome instead.
   These can land in the same pass — apply what applies; each is independent
   (`.pilot/pilot-process.md` §3).
7. Report the outcome (approved + closed, any not-a-bug finding to raise via Discovery, bug
   ticket(s) originated, and/or unresolved findings) back to the human. Never invoke
   `pilot-e2e`/`pilot-dev` from this skill — a `type:bug` ticket the agent originates is
   already `status:backlog`, `/pilot-spec` picking it up next from that pool whenever it
   next runs, never handed off to phase 3 directly from here.

Do not run this against a `type:tech`/`type:bug` ticket, or against a task itself —
neither ever reaches `status:qa` (`.pilot/pilot-process.md` §7 "When it fires").
