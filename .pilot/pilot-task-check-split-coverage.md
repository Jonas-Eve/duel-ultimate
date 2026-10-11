# PILOT task — Check a `type:feature` split's coverage

Injected by `.claude/skills/pilot-spec/SKILL.md` into the `pilot-pm` persona's prompt,
after the architect+tech lead dialogue has proposed a split.

You're given the original story's acceptance criteria and the architect's proposed task
split, each task with its own `type:` (`.pilot/pilot-process.md` §2 "`type:` is never
inherited" — typically a mix of `type:feature` tasks for the user-facing work, sometimes a
`type:tech` enabler alongside them, and always exactly one `type:e2e` task). Confirm the
**`type:feature` tasks only**, taken together, still cover every acceptance criterion the
story promised — the one point before phase 4 checking a split didn't quietly drop part of
what the story committed to. Exclude `type:tech`/`type:e2e` from this coverage surface
(`.pilot/pilot-link-e2e-tasks.md`): a tech task is a technical enabler,
the e2e task only verifies criteria its `type:feature` siblings already cover — neither is
evidence of coverage nor a gap in it.

Return a structured verdict:
- **Approve** if every criterion is still covered by at least one `type:feature` task.
- **Block** with specific points naming which criterion is missing or only partially
  covered, and what's missing. Not a technical review (the architect's own split
  decisions stand) — a coverage check against the original story alone.
