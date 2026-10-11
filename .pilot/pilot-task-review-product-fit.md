# PILOT task — Review product fit

Injected by `.claude/skills/pilot-review/SKILL.md` into the `pilot-pm` persona's prompt,
alongside `.pilot/pilot-link-review-consensus.md` for the shared verdict format and
`change`/`decision` tagging rule — for a `type:feature` or `type:e2e` task (never
`type:tech`, even under a `type:feature` story's split — its own `type:` decides, never
inherited).

Which check you run depends on the task's own `type:` (`.pilot/pilot-process.md` §2 — never
inherited, so a `type:tech` task under a `type:feature` story's split is never yours to
review, even though its siblings are):

- **`type:feature`** — review the shipped PR against the *original story's acceptance
  criteria* — a product-fit check, not a code review (the architect and tech lead cover
  architecture and code quality). Read the linked story (walk up to its `level:story`
  parent — still `status:split` while its tasks are in progress — if you're reviewing one
  of its tasks; that parent holds the acceptance criteria), the PR diff, and the PR
  description. Block only on a criterion that's missing, changed, or too ambiguous to
  confirm is met — never on style or implementation details that aren't product-visible.
  If that parent (or the task itself, `.pilot/pilot-task-scope-story.md`) carries a UI/UX
  description or an attached mockup, also check the shipped UI against it and block on a
  clear deviation — never on a static image's implementation detail it couldn't have
  specified. Where it left something unspecified, check consistency against this
  project's own design system/style guide instead, if it has one — block only on a clear
  break from established components/patterns, never a style preference of your own.
- **`type:e2e`** — review whether the test actually exercises the real, already-merged
  end-to-end flow it claims to (not a mocked-out subset), and whether, with its
  scenarios, it genuinely validates the story's acceptance criteria as a whole — the one
  point confirming the split's pieces actually add up to the story, not just
  individually. Read the story's acceptance criteria (same parent-walk as above), the e2e
  task's own spec, and the PR diff.
