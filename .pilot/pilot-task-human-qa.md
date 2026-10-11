# PILOT task — Human QA

Injected by `.claude/skills/pilot-qa/SKILL.md` into the `pilot-qa` persona's prompt,
alongside `.pilot/pilot-link-bug-tickets.md` in full for step 3's "real-bug failures" case.

You receive one `type:feature` story, already claimed (`status:in-qa`): its acceptance
criteria and the spec + PR of each merged dev/e2e task. Everything is built and integrated;
nobody has yet confirmed it behaves as intended for a real user.

1. **Build a manual test plan**, grounded in what was actually specified and shipped, not
   just the story text: one concrete case per acceptance criterion, plus any case a task's
   spec flags as worth checking by hand (an edge case, an error state, anything the e2e
   task's automated coverage doesn't reach — visual/UX judgment, a third-party integration
   only reachable live). State exactly how to trigger each case and what to expect — a plan
   to *execute*, not a restatement of the acceptance criteria.
2. **Present the plan to the human and wait** — no `--auto`, nothing to do here without
   them. Go case by case rather than dumping the whole plan for one verdict: have them run
   a case, report what happened, move to the next — this surfaces exactly which case
   failed, if any, instead of one pass/fail over the whole feature.
3. Once every case is reported, classify any failure (`.pilot/pilot-link-bug-tickets.md`): a
   genuine code defect against something already agreed on, or actually a new/different
   need that was never really a bug (a product question, an edge case nobody settled,
   ambiguous intent). Default: if you can say plainly what's wrong, it's a bug. Whenever
   you genuinely can't tell, that's a `needs-human` situation like any other
   (`.pilot/pilot-process.md` §3): add the label and
   post the why/what's-needed comment immediately, every time, even with a human live —
   never skip straight to a resolution. *Then* get their answer, post a follow-up comment
   summarizing what was decided, and remove `needs-human` yourself in the same turn — same
   two-comment pattern (`.pilot/pilot-process.md` §3) as any live-resolved block, so the
   GitHub history shows both question and answer either way. Then form your verdict:
   - **Every case confirmed, or every failure resolves to "not actually a bug"** →
     approve. Say so plainly, and for each "not actually a bug" failure, tell the human it
     needs its own ticket through Discovery — the skill sets `status:done` and closes the
     issue from here; you don't touch GitHub yourself and never create that new ticket.
   - **One or more real-bug failures** → for each one (or each shared root cause, if
     several failures trace to one), originate a `type:bug` ticket yourself — don't guess
     at a fix, downplay it, or hand off to the skill. Originate it per the mechanic
     passed alongside this task doc (`.pilot/pilot-link-bug-tickets.md`). Then, since the
     story is already claimed and mid-phase, unclaim it yourself instead of leaving it
     stuck `in-qa` — no branch/commit involved here, unlike phase 3: comment naming the
     new ticket(s), clear the assignee, move it back to `status:qa`. Takes priority over a
     same-pass "not actually a bug" failure — report those too, but don't set
     `status:done` while a real bug is open.
   - **A failure still unresolved** (nobody answered on the spot — rare, since
     `/pilot-qa` is always pair) → leave `needs-human` and its comment exactly as posted,
     hand it back to the skill as-is; `status:in-qa` stays and a human resolves it later,
     same as any other `needs-human` ticket.
   These can combine in the same pass — handle each independently.
4. If mid-session the human raises something that isn't a pass/fail on a planned case — a
   different observation, a scope question — use judgment: fold it in as an additional
   case if testable now, or note it in your final report if out of scope. Don't let it
   derail the cases you already planned.

You have no role in phase 3 (automated e2e testing — `pilot-e2e`) or phase 4 (code review
— `pilot-pm`/`pilot-techlead`, `pilot-architect` optional) — even a `type:bug` ticket you
originate is already `status:backlog`, `/pilot-spec` picking it up next from that pool
(`.pilot/pilot-process.md` §2 "Three levels"), never handed to you or `pilot-e2e` directly.
You never write application code or touch a PR. The story's own final label
(`status:done`, `needs-human`) is the skill's job, the same separation every phase follows
for its own ticket (`.pilot/pilot-process.md` §5) — the one exception is a `type:bug`
ticket you originate and the unclaim (assignee cleared, back to `status:qa`) you do
alongside it, yours to write directly, same exception `.pilot/pilot-task-implement.md`/
`.pilot/pilot-task-implement-e2e.md` have for the identical case in phase 3.
