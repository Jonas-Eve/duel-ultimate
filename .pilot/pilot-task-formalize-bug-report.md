# PILOT task — Formalize a raw `type:bug` report

Injected by `.claude/skills/pilot-spec/SKILL.md` into the `pilot-architect` persona's
prompt, for its own no-ticket entry, when the raw need auto-detects (or is declared via
`--bug`) as a defect report — the one shared mechanism every bug ticket goes through,
whether a human invoked it directly or `pilot-dev`/`pilot-e2e`/`pilot-qa` is reusing it
after finding one mid-work (`.pilot/pilot-link-bug-tickets.md`).

You receive a raw bug report in free text — a human's account, or a technical trace
(failing assertion, error log, stack trace) if it came from another phase's own
discovery (`.pilot/pilot-link-bug-tickets.md`) rather than through `/pilot-spec` directly.
Turn it into one well-formed `type:bug` issue ready to build: a bug is dev-sized by
definition and never splits (`.pilot/pilot-process.md` §2 "Three levels") — no splitting
decision to defer, though its spec is still a separate, later step (this same skill
invocation, if a human ran it directly and wants to continue; a future one, if this
ticket is left in the pool instead).

1. Confirm it's actually a defect against already-agreed behavior (regression, broken
   promise, error) — not a feature request or ambiguous product question in disguise.
   If it's the latter, say so and suggest `/pilot-discovery`, or `/pilot-spec --tech`,
   whichever fits. If instead you're confident it isn't a real, actionable
   defect at all (already fixed, not reproducible, an exact duplicate of an open bug
   ticket, or working as intended), say so and create nothing — a bug never carries
   `status:wont-do`, so there's no later checkpoint to catch this
   (`.pilot/pilot-process.md` §2 "Three levels"); the same "out of scope, create nothing"
   outcome `/pilot-discovery` already handles for any raw idea (`.pilot/pilot-process.md`
   §4 "Interaction modes").
2. Pin down the failure as concretely as you can from what's given (exact broken
   behavior, error/assertion, likely file(s)/module(s)) — enough that the spec step can
   fix it without re-diagnosing from scratch. State your confidence; if you can't
   reproduce or localize it, say so plainly rather than guessing at a root cause.
3. Write the issue body: what's broken, how to reproduce/observe it, your best
   root-cause diagnosis and suggested fix location, and severity/impact — the full
   content a `status:backlog` ticket needs, since no split pass ever follows.
4. Label it `type:bug`, `level:task`, `status:draft` if a human is live to confirm the
   classification first (this skill's own no-ticket entry), or straight to
   `status:backlog` if you're applying `.pilot/pilot-link-bug-tickets.md`'s shared
   mechanism on another phase's behalf (no live drafting session of its own — that
   phase's own task doc covers this), unassigned once finalized, with `priority:`
   set yourself (`.pilot/pilot-process.md` §3 — a bug's initial priority is never
   revisited, since it never splits). Never grouped under a `level:epic` — that's for
   `level:story` tickets only, and a bug is never one.
