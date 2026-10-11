# PILOT task — Write the technical spec

Injected by `.claude/skills/pilot-spec/SKILL.md` into the `pilot-techlead` persona's
prompt — for a `level:story` being split, alternating turns with the architect's own
`.pilot/pilot-task-scope-story.md` (`.pilot/pilot-link-agent-dialogue.md`), one spec per
resulting ticket, written in the same pass as the split itself; for a `level:task` (always
a standalone `type:bug` — the only `level:task` that ever waits on its own spec, since a
freshly split-off task is always spec'd in the same pass that creates it) you work alone,
no split decision to make.

You receive one ticket carrying either the architect's split-time
security/architecture decisions (a `level:story`'s own task, this same pass), or, for a
`type:bug` ticket (which never splits — `.pilot/pilot-process.md` §2 "Three levels"), the
architect's original diagnosis and suggested fix, recorded when the bug was created — the
analogous judgment from a different moment.

1. The claim (assignee + `status:in-spec`) is handled before you are invoked — assume
   it's already yours.
2. Read the affected code — your own identity's project-doc habit already covers the
   conventions/README side of grounding yourself in the area the ticket touches.
3. Write the technical spec directly into the ticket body: implementation approach,
   files/modules touched, data/schema changes, API contract changes, and a test plan. If
   the ticket carries a UI/UX description, the implementation approach must account for
   it — components/screens touched, how they map to what's described.
4. If that earlier judgment (the architect's split-time decisions, or a bug ticket's
   original diagnosis/suggested fix) conflicts with the real code — not a style
   preference; for a bug this includes finding it's not reproducible or already fixed —
   don't override it silently. For a `level:story`'s split, still in dialogue with the
   architect: raise it there first — a feasibility concern can reshape the split itself in
   the same pass, before it ever needs a human. Only once the dialogue itself can't
   resolve it (`.pilot/pilot-link-agent-dialogue.md`), or for a `level:task`/bug where
   there's no architect to raise it with in this pass: add `needs-human` (keep the
   ticket's current `status:` — `.pilot/pilot-process.md` §3) and a comment stating the
   conflict and what needs deciding, every time, even with a human live in this session.
   If that human answers in conversation, proceed with their answer, post a follow-up
   comment summarizing the decision, and remove `needs-human` yourself in the same turn;
   otherwise leave the flag and comment for a human to resolve later. If the ticket
   instead just can't proceed yet due to unresolved work elsewhere (not a judgment call),
   use `on-hold` instead (`.pilot/pilot-process.md` §3 "`on-hold`") with a comment on what
   it's waiting on.
5. Otherwise, move the ticket to `status:dev-ready`.

You do not write implementation code here — that's Dev (phase 3).
