# PILOT task — Review spec conformance and code quality

Injected by `.claude/skills/pilot-review/SKILL.md` into the `pilot-techlead` persona's
prompt, alongside `.pilot/pilot-link-review-consensus.md` for the shared verdict format and
`change`/`decision` tagging rule.

Before forming your verdict, **re-run the relevant validation commands directly against
the PR's branch** (the same build/test/lint commands the dev persona used in phase 3)
rather than trusting the PR description's claim that tests pass — a failure despite that
claim is an automatic block. This matters most with no CI yet, since your re-run is the
only check that actually executes anything; once CI exists, treat a red or pending run
the same way.

Then review the shipped PR for two separate things, giving each its own attention rather
than letting the first crowd out the second:
- **Spec conformance** — does the implementation match the ticket's own spec, and if it
  deviates, is the deviation justified.
- **Code quality/maintainability** — readability/naming, whether tests actually exercise
  the claimed behavior (not just present), edge cases the diff misses, and anything you'd
  flag in a normal review regardless of spec match — including whether TDD was actually
  followed: check the commit history for a failing-test commit preceding the
  implementation commit(s) that fix it (`.pilot/pilot-task-implement.md` step 3). If the
  history doesn't show this clearly (squashed/force-pushed commits, tests and
  implementation mixed together), that's itself a `change`-worthy quality gap — don't
  assume test-first happened just because the PR says so. **Exception: `type:e2e` tasks**
  (implemented by `pilot-e2e`, not `pilot-dev`) have no red-green-refactor history by
  design (`.pilot/pilot-task-implement-e2e.md`) — judge them instead on whether the test
  exercises the real, already-merged integration points it claims to, without mocking
  them away. There's no separate reviewer here: `pilot-dev` already self-reviewed the
  diff before opening the PR (`.pilot/pilot-process.md` §4 "Interaction modes"), so your
  check is independent of, not a duplicate of, that self-review.
