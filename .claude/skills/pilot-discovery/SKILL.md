---
name: pilot-discovery
description: "Phase 1 of PILOT (see .pilot/pilot-process.md): turns a raw type:feature idea into one or more formalized GitHub issues, PM and architect working it together in dialogue (.pilot/pilot-link-agent-dialogue.md) rather than one handing a finished draft to the other — the architect anticipates architecture (interfaces, infra shape) while the PM shapes the story, instead of challenging it later from scratch. Produces one or more level:story tickets — type:feature, and, when the same effort genuinely needs a technical enabler alongside it, type:tech too — grouped under a new/reused level:epic (always type:feature) if it doesn't fit in one story, with dependencies between them. Never splits into dev-sized tasks, records a prerequisite, or writes a spec — that's /pilot-spec's job, run separately. Always pair mode: drafts live with a human, creating the ticket(s) as status:draft as soon as a first draft exists and refining in place. No --auto, never Routine-driven; --resume <issue number> picks back up a status:draft ticket left mid-pair. An optional --multi <N> runs N independent PM+architect dialogues on the same raw idea instead of one, converging the N resulting proposals with no fixed round cap the same way the base dialogue converges (see .pilot/pilot-link-agent-dialogue.md), escalating to a live question for the human only on a genuine, unresolved disagreement. A standalone type:tech need with no product framing, or any type:bug report, never comes through here — see /pilot-spec's own no-ticket entry instead. Use whenever a human wants to start a new product idea."
argument-hint: "<raw idea in free text> [--multi [N]] | --resume <issue number>"
disable-model-invocation: true
---

# PILOT — Phase 1: Discovery

Read `.pilot/pilot-process.md` before running this if you haven't already — it's the
source of truth for labels, states, and the claim protocol; this skill only covers the
mechanics of running Discovery. Read `.pilot/pilot-link-agent-dialogue.md` too — it covers
the PM/architect turn-taking mechanic this skill relies on throughout.

This phase is `type:feature` only. A standalone technical need with no product framing
(infra, CI, a migration, nothing a PM would have an opinion on) or a bug report never
comes through Discovery at all — point at `/pilot-spec --tech`/`--bug` instead (its own
no-ticket entry, `pilot-spec/SKILL.md`).

## Steps

1. Determine the input:
   - `--resume <issue>`: must be `status:draft`, assigned, **no** `needs-human`/`on-hold`
     — a draft left mid-pair (`.pilot/pilot-process.md` §4 "Resuming an orphaned
     claim"). Read the ticket and thread (`mcp__github__issue_read` `get_comments`) to
     reconstruct what's drafted, claim it (overwrite assignee), skip to step 4 with that
     state (call the agents in step 3 again first if revising the draft, passing the
     reconstructed context instead of a blank idea). If it doesn't match, report and stop.
   - A raw idea (the argument, or ask if none given): continue with steps 2-4. Don't
     pre-analyze it — that's the agents' job, not this skill's. If it reads as a
     standalone technical need or a bug report instead of a product idea, say so and
     point at `/pilot-spec --tech`/`--bug` rather than proceeding.
2. Fetch open `level:epic` issues (always `type:feature`, title + body only,
   `mcp__github__list_issues`/`search_issues`) as candidates the agents might reuse —
   cheap, deterministic bookkeeping, not the agents' job to search for itself.
3. Run the PM+architect dialogue (`.pilot/pilot-link-agent-dialogue.md`): call `Agent` for
   `pilot-pm`, then `pilot-architect`, alternating turns, each passed the running
   conversation so far. Read `.pilot/pilot-task-write-story.md` (PM's duty) and
   `.pilot/pilot-task-anticipate-architecture.md` (architect's duty) and pass the matching
   one as part of each call's prompt, along with the raw idea, the candidate Epic list, and
   the dialogue so far — nothing else from this conversation's history, so each call gets a
   clean, scoped context (`.pilot/pilot-process.md` §5). The persona files themselves
   (`.claude/agents/pilot-*.md`) carry only identity now — the task docs are what tell each
   one what to actually do. Continue until they converge (`.pilot/pilot-link-agent-dialogue.md`)
   on: out of scope (per this project's functional-scope doc, if it has one — stop, report,
   create nothing); a single story (with or without a `type:tech` companion story for a
   technical enabler the PM and architect agree the same effort needs); or several stories
   plus an existing Epic to reuse or a new Epic to create.
3a. **With `--multi <N>`**: run N independent instances of step 3's own dialogue instead
    of one, converging on a single proposal with no fixed round cap
    (`.pilot/pilot-link-agent-dialogue.md`) before continuing to step 4. A genuinely
    irreconcilable disagreement surfaces to the human right there in this same pair
    session, differing proposals quoted verbatim (never a silent pick).
4. **Create the draft ticket(s) right away** (`mcp__github__issue_write`,
   `mcp__github__sub_issue_write`), before showing anything to the human — this is what
   makes `--resume` possible if the session ends before final approval
   (`.pilot/pilot-process.md` §3 `status:draft`, §4 "Resuming an orphaned claim"):
   - Single story: create it, `type:feature` + `level:story` + `status:draft` + the
     dialogue's initial `priority:P0/P1/P2` (`.pilot/pilot-process.md` §3), assigned to
     this session.
   - Several stories, reusing an existing Epic: create each story the same way
     (its own `type:` — `type:feature` or `type:tech`, `level:story`, `status:draft`, its
     own `priority:`, assigned), link each as that Epic's sub-issue.
   - Several stories, new Epic: create the Epic (`level:epic` + `type:feature`, no
     `status:` label, unassigned — an Epic is never itself a draft, and never carries a
     `priority:`) first, then each story linked as its sub-issue, its own `type:` +
     `level:story` + `status:draft` + its own `priority:` + assigned.
5. This phase always runs paired (`.pilot/pilot-process.md` §4 "Interaction modes" —
   `/pilot-discovery` has no `--auto`): show the human the drafted story/stories (and epic
   decision, if any) and the architecture already anticipated — pointing at the real
   issue number(s) from step 4, not just conversation text. For each `type:feature` draft
   involving user-facing UI, also ask once whether the human has an existing mockup,
   wants to sketch one live (e.g. the `design` skill, if available), or is fine with the
   PM's own prose description. No tool here can upload an image to a GitHub comment the
   way the web UI's drag-and-drop does, so either way ask the human to attach it to the
   ticket themselves as a comment — not necessarily this same round; before each round
   from here on, check the ticket's comments (`mcp__github__issue_read` `get_comments`)
   and, once something's attached, pass a link to it into the dialogue so it references it
   instead of writing a competing description. Neither offered → the PM's own prose
   description stands. If the human corrects something (a story that should be `type:tech`
   instead of `type:feature`, a split that should be one story instead of two), feed it
   back into the dialogue and repeat until they approve. If the dialogue now decides the
   idea is out of scope: set `status:wont-do` and close the draft ticket(s) instead of
   leaving a closed issue still labeled `status:draft` (`.pilot/pilot-process.md` §3
   `status:wont-do`) — nothing proceeds to `status:backlog`. Otherwise, write each round's
   changes into the draft ticket(s) (`issue_write`) as agreed — repeat until approved.
   Requires a human live; never run from a scheduled sweep. Once approved, continue to
   step 6.
6. **Final consolidation pass** (`.pilot/pilot-process.md` §4 "Interaction modes"): before
   finalizing, have the dialogue re-read each draft ticket's body as a whole — not just the
   latest round's delta — and fix anything that no longer holds together across rounds
   (an out-of-scope note from an early round that no longer matches an acceptance
   criterion added later, an architecture assumption one round made that a later round's
   split contradicts).
7. Finalize (`mcp__github__issue_write`): flip each story from `status:draft` to
   `status:backlog`, unassigned.
8. Report the issue number(s)/URL(s) back to the human.

Do not decide whether a story needs splitting into tasks, record a dependency, or write a
spec as part of this skill — that's `/pilot-spec`'s job, run separately once per story.
