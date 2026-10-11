# PILOT link — Resolving an explicit ticket that's actually a container

Injected whole by `.claude/skills/pilot-spec/SKILL.md`,
`pilot-dev/SKILL.md`, and `pilot-review/SKILL.md` — the one place each of these three
checks this before applying its own status-based bullets to a freshly-given ticket
number. Not Discovery (no such ticket to resolve, it starts from a raw idea or
`--resume`) or phase 5/QA (`status:qa`/`status:in-qa` stories never have open sub-issues,
`.pilot/pilot-process.md` §3 "Cascading completion"). `/pilot-auto` needs no awareness
of this at all — giving it an Epic or a split story's number still just means "try the
fixed order against that one ticket" (`pilot-auto/SKILL.md`); it's each of the three
phases above that resolves what "that one ticket" actually is, the same delegation
`/pilot-auto` already relies on for everything else. See `.pilot/pilot-process.md`
§2/§3/§4 for the generic ticket levels, labels, and claim protocol this builds on.

**Not symmetric across all three.** `level:epic` never carries a `status:` of its own
and is never any of the three's own territory, so all three trigger on it the same way.
A `status:split` `level:story` is different: once split, it's never `/pilot-dev` or
`/pilot-review`'s own territory either (an unsplit `type:tech` story,
with no sub-issues at all, `.pilot/pilot-process.md` §2 "Three levels", is unaffected
and still claimed by those two directly, same as always) — so those two trigger on
a `status:split` story too — but `/pilot-spec` is the one phase that keeps a
legitimate, documented direct path to a still-open `status:split` story: re-scoping it
in place, adding more tasks to the existing split
(`.pilot/pilot-process.md` §2 "Re-scoping a `type:feature` story after its split is
done", "`status:split`, original e2e task not yet done"). This mechanism must never
intercept that case, so `/pilot-spec` only triggers it for a `level:epic` — never for a
`status:split` story, which it resolves through its own ordinary bullets untouched.

Before applying its own status-based resolution to a freshly-given ticket number, a
phase skill checks whether it's a container it should search into rather than act on
directly — for `/pilot-spec`, a `level:epic`; for the other two, a `level:epic` or a
`status:split` `level:story` — and, if so, whether it currently has any open sub-issues
(`mcp__github__issue_read` method `get_sub_issues`, filtered to still-open):
- **Not that kind of container, or no open sub-issues** → resolve it exactly as the
  phase's own step normally would, no detour. This is also what a fresh `level:story`
  still at `status:backlog` hits — `/pilot-spec`'s own resolution is what actually does
  something with it, e.g. splitting it into tasks that become its own open sub-issues
  for a later call to search; and what a still-open `status:split` story hits for
  `/pilot-spec` specifically, per the asymmetry above.
- **Open sub-issues found** → don't act on the given ticket itself. Instead, collect
  every ticket in its sub-issue tree, at every depth — an Epic's open stories, plus, for
  any of those that's itself `status:split`, that story's own open tasks in turn
  (capped at two hops: the tree is never deeper than Epic → story → task,
  `.pilot/pilot-process.md` §2 "Three levels", so this always terminates). Run this
  phase's own **ordinary bare-pool query** against that whole collected set — exactly
  the same query it runs with no ticket at all (`.pilot/pilot-process.md` §4 "Picking
  the next ticket when none is specified": its own pre-claim status, plus
  `can-resume`-marked in-progress work, plus, for `/pilot-dev` only, reclaimable
  `status:changes-requested`) — with the same exclusions
  (`needs-human`/`on-hold`/an unresolved "Depends on #N") and the same ordering (highest
  `priority:` first, then "Blocks #M"-referenced before one that isn't, then oldest).
  Whatever that query's top pick is, claim and resolve it exactly as bare/no-argument
  mode would for that same candidate — not the given ticket's own step-1 bullets, which
  cover cases (`--resume`, a `status:qa`/`status:in-qa` reclaim, an orphaned claim) this
  search deliberately doesn't reach into, the same way a bare pool never does either.
  Nothing in the whole tree matches → report nothing to do for this ticket, exactly as
  if it had been given directly and didn't match.

This is a search across the entire subtree, not a single greedy path down by priority:
an Epic with one high-priority story still at `status:backlog` and a lower-priority,
already-split sibling story with an open `status:dev-ready` task still finds that task
for `/pilot-dev` — the fresh-`status:backlog` story simply isn't a `/pilot-dev` pool
candidate at all, wherever it sits.

An Epic whose every story is closed but whose own issue is still open by hand has no
open sub-issues to search, so it falls through to the ordinary resolution above — the
one case `pilot-spec/SKILL.md`'s own "there's nothing to scope on the epic itself"
line still fires.
