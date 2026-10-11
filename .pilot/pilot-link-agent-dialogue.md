# PILOT link — Agent dialogue (Discovery, Spec, `--multi` on Dev)

Read directly by `pilot-discovery`/`pilot-spec`/`pilot-dev`'s own `SKILL.md` to run their
own turn-taking loop — never injected into an `Agent` call as text for a persona to read
about itself, and never read by `.claude/agents/pilot-*.md`, which carry only identity.
See `.pilot/pilot-process.md` §4 for the generic claim protocol and interaction modes this
builds on, and `.pilot/pilot-link-multi-consensus.md` for the older, unrelated mechanism
this replaces for these three skills (Review keeps that one — never this doc).

## What it's for

Two different personas working the same ticket together, in a real back-and-forth,
before a human ever sees a draft — not one producing a finished artifact the other then
critiques separately:

- **Discovery** — `pilot-pm` + `pilot-architect`, turning a raw idea into a story (or
  several) with the architecture already anticipated, not bolted on afterward.
- **Spec** — `pilot-architect` + `pilot-techlead`, splitting a story and writing the
  resulting spec(s) in the same pass — a spec-time feasibility concern can send the split
  itself back for a rethink, in the same conversation, rather than surfacing later as a
  `needs-human` block on an already-fixed split.
- **`--multi` on Dev** — N `pilot-dev` (or `pilot-e2e`) instances, at the
  proposed-approach stage only (`pilot-dev/SKILL.md` step 3, unchanged in scope — still
  never on finished code), discussing the tradeoffs directly with each other instead of
  the skill comparing N isolated outputs afterward.

Never Review: two (or three) reviewer roles are never trying to converge on one shared
verdict, so nothing here applies there — Review's own `--multi` keeps the older,
skill-mediated, capped mechanism (`.pilot/pilot-link-multi-consensus.md`).

`--multi <N>` on Discovery or Spec runs N independent instances of the *whole two-persona
dialogue* (not N instances of one persona) — each its own isolated PM+architect, or
architect+tech lead, conversation converging on its own proposal first. Once all N have
their own proposal, compare them: every substantive point agrees across all N → adopt any
one verbatim, done. Any point genuinely diverges → that's a second round, not a skill-side
comparison — call `Agent` again for each persona involved, a fresh instance of each seeing
every one of the N proposals and exactly where they diverged, and let them converge on a
single reconciled proposal the same way the base dialogue would; repeat as long as new
rounds are actually narrowing the disagreement, no fixed cap. `--multi` on Dev instead
ensembles N instances of the *same* persona (`pilot-dev`/`pilot-e2e`) as direct peers in
one exchange from the start, per `pilot-dev/SKILL.md` step 3 — no separate "N independent
runs, then reconcile" stage, since there's only one persona to instantiate N times in the
first place.

## Mechanics

### Turn-taking

Call the `Agent` tool for one persona, then the other, passing each the running
conversation so far (the original input, plus every prior turn from both sides) — not a
single shot each reconciled by the skill, but an actual alternating exchange. Either
persona can go first (whichever naturally starts — the PM drafting the raw idea into a
first story shape for Discovery, the architect proposing a first split shape for Spec);
either can also raise a new concern that reopens a point the other thought settled. For
`--multi` on Dev, every one of the N instances is a peer in the same exchange, not paired
off — each turn is shown what every other instance has said so far.

### No fixed number of turns

Unlike `.pilot/pilot-link-multi-consensus.md`'s capped, skill-mediated retry (one retry,
then escalate), there is no external round limit here. The personas themselves judge
when they've actually converged — every substantive point agreed, nothing left either
side wants to revisit — and stop there, however many turns that took. Don't force a
minimum number of exchanges either: if the second persona has nothing to add to the
first's proposal, converging immediately is a valid outcome, not a sign the dialogue was
skipped.

### Recognizing a genuine disagreement

Keep exchanging turns as long as new information or a changed mind is actually moving the
conversation. The signal to stop and escalate is a *substantive* point where every party
involved — two personas in the base dialogue, or however many proposals/instances an
ensemble is comparing — holds an **incompatible** position (one clears it, another still
blocks on it; one proposes A, another insists on B) and has restated that same position
without anyone changing it or surfacing anything new — not a turn count. This is
different from the next section's failure mode: there, nobody actually holds a competing
position, the dialogue is just still talking after already agreeing. When incompatible
positions are the case, add `needs-human`
(`.pilot/pilot-process.md` §3) with a
comment quoting every position verbatim, never summarized or reduced to "two sides" when
more than two actually differ — a human decides
directly, including the "a human is live in the same session" path (`.pilot/pilot-process.md`
§3): pair mode running this dialogue is still pair mode, so a human present right then
answers immediately. Once they do, the agents proceed with that answer as the converged
position, and the phase continues to its normal remaining steps. No live human on the
spot → the ticket stays blocked and waits, same as any other `needs-human` ticket.

### Recognizing unproductive length (not a disagreement)

A different failure mode from a genuine disagreement above: no incompatible positions
anywhere, nobody actually blocks on anything — but the conversation keeps going regardless,
another round of wording tweaks, a detail revisited that both sides already agreed on, a
"let me reconsider" that doesn't lead anywhere new. This isn't a turn-count check either —
a long dialogue that's still visibly narrowing something real is fine, however many turns
that takes.

Check this **after** a full round (both personas have had a turn), never by predicting
whether your own next turn would be worth adding — that's a harder, less reliable
question than looking at what a round just produced. Compare this round's resulting
proposal to the one going into it: no substantive difference (only rephrasing, or a point
already settled being revisited without new information) means the dialogue has already
converged, even if neither side has explicitly said so — treat it as done rather than
running another round hoping something changes. A round that *did* change something real
— a scope boundary moved, a security concern added, a spec detail corrected — is real
progress regardless of how small it looks; only a round that changed nothing substantive
is the tell.

Unlike a genuine disagreement, this needs no
`needs-human` — there's nothing for a human to adjudicate, nobody disagrees — finalize with
the current proposal as converged and move on to the phase's normal remaining steps.
Applies the same way to `--multi`'s own reconciliation rounds (above): if a fresh round
isn't actually closing the gap between the N proposals, adopt the strongest one rather
than running another round hoping it converges on its own.

### Once converged

The converged proposal is what pair mode's own checkpoint shows the human (§4
"Interaction modes"), or what `--auto` applies straight through — exactly as if it had
been produced in one shot. For `--multi` on Dev specifically: once the N instances
converge (or a live human resolves an escalation) on one implementation approach, exactly
**one** further `Agent` call actually implements it (`pilot-dev/SKILL.md` step 4) — the
ensemble's job ends at the approach, same as before.

## `/pilot-auto` forwarding

An optional `--multi <N>` on `/pilot-auto` forwards verbatim to whichever phase actually
runs, same as documented in `.pilot/pilot-link-multi-consensus.md` — this doc only changes
*how* Discovery/Spec/Dev's own ensemble converges internally, never which ticket gets
claimed or how `/pilot-auto` itself behaves.
