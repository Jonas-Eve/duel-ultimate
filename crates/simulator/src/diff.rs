//! The diff of two reports ([`diff_reports`]): pure, no I/O, no clock, no libm.

use crate::report::Report;

/// `pm(n, d)`: `1000 n / d` rounded to the nearest integer, half up, in integers.
fn pm(_n: u64, _d: u64) -> u128 {
    0
}

/// The diff between `before` (the reference) and `after` (the candidate), as text.
pub fn diff_reports(_before: &Report, _after: &Report) -> String {
    String::new()
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::report::{Kind, MatchupReport, PolicyReport, rules_label};
    use duel_engine::RuleSet;

    #[allow(clippy::too_many_arguments)]
    fn m(
        (a, b): (&str, &str),
        kind: Kind,
        finished: u64,
        wins_a: u64,
        first: u64,
        exhaustion: u64,
        total_actions: u64,
    ) -> MatchupReport {
        MatchupReport {
            deck_a: a.into(),
            deck_b: b.into(),
            kind,
            games: finished,
            interrupted: 0,
            wins_a,
            wins_b: finished - wins_a,
            seat_one_wins: if kind == Kind::Mirror { wins_a } else { 0 },
            first_player_wins: first,
            hero_down: finished - exhaustion,
            exhaustion,
            concede: 0,
            total_actions,
            durations: Default::default(),
        }
    }

    fn cross(a: &str, b: &str, wins_a: u64, first: u64) -> MatchupReport {
        m((a, b), Kind::Cross, 2000, wins_a, first, 0, 80_000)
    }

    fn mirror(a: &str, first: u64) -> MatchupReport {
        m((a, a), Kind::Mirror, 2000, 1000, first, 0, 80_000)
    }

    fn report(set: &str, policies: Vec<(&str, Vec<MatchupReport>)>) -> Report {
        Report {
            set: set.into(),
            seed: 1,
            games_per_matchup: 2000,
            max_actions: 10_000,
            rules: RuleSet::default(),
            policies: policies
                .into_iter()
                .map(|(policy, matchups)| PolicyReport {
                    policy: policy.into(),
                    matchups,
                })
                .collect(),
        }
    }

    const SAMPLES: &str = "NOTE: the two batches are independent samples (the seed of a game depends on the index of its matchup): a delta within the 95% intervals is noise";
    const SIGNIFICANT: &str = "significant: yes when the 95% Wilson intervals do not overlap (conservative heuristic; cross pair: share of A, mirror: first player)";

    fn rules() -> String {
        rules_label(&RuleSet::default())
    }

    #[test]
    fn pm_rounds_half_up_in_integers() {
        assert_eq!(pm(0, 1), 0);
        assert_eq!(pm(1, 1), 1000);
        assert_eq!(pm(1, 3), 333);
        assert_eq!(pm(2, 3), 667);
        // 1/8 = 125 exactly; 1/16 = 62.5 -> 63 (half up); 3/16 = 187.5 -> 188.
        assert_eq!(pm(1, 8), 125);
        assert_eq!(pm(1, 16), 63);
        assert_eq!(pm(3, 16), 188);
        // 1/2000 = 0.5 -> 1; 1/2001 < 0.5 -> 0.
        assert_eq!(pm(1, 2000), 1);
        assert_eq!(pm(1, 2001), 0);
        assert_eq!(pm(u64::MAX, u64::MAX), 1000);
        assert_eq!(pm(u64::MAX - 1, u64::MAX), 1000);
        assert_eq!(pm(u64::MAX / 2, u64::MAX), 500);
    }

    #[test]
    fn golden_with_a_common_a_new_and_a_removed_pair() {
        let before = report(
            "starter@1.1.0",
            vec![(
                "random",
                vec![
                    m(
                        ("rush", "bulwark"),
                        Kind::Cross,
                        2000,
                        1944,
                        1100,
                        0,
                        83_800,
                    ),
                    m(("rush", "rush"), Kind::Mirror, 2000, 1000, 1240, 0, 74_400),
                    m(("bulwark", "bulwark"), Kind::Mirror, 2000, 1000, 1100, 0, 0),
                ],
            )],
        );
        let after = report(
            "starter@2.0.0",
            vec![(
                "random",
                vec![
                    m(
                        ("rush", "bulwark"),
                        Kind::Cross,
                        2000,
                        1220,
                        1100,
                        82,
                        104_600,
                    ),
                    m(("rush", "rush"), Kind::Mirror, 2000, 1000, 1102, 40, 96_000),
                    m(("rush", "tempo"), Kind::Cross, 2000, 1000, 1000, 0, 0),
                ],
            )],
        );
        let expected = format!(
            "diff: starter@1.1.0 -> starter@2.0.0 | seed: 1 | games per matchup: 2000 | action cap: 10000 | policies: random | rules: {}\n\
{SAMPLES}\n\
policy: random\n\
matchup           status          wins A  delta  significant    first player  mean actions    exhaustion\n\
----------------  -------  --------------  -----  -----------  --------------  ------------  ------------\n\
rush vs bulwark         =  97.2% -> 61.0%  -36.2          yes  55.0% -> 55.0%  41.9 -> 52.3  0.0% -> 4.1%\n\
rush (mirror)           =               -      -          yes  62.0% -> 55.1%  37.2 -> 48.0  0.0% -> 2.0%\n\
rush vs tempo         new               -      -            -               -             -             -\n\
bulwark (mirror)  removed               -      -            -               -             -             -\n\
{SIGNIFICANT}\n\
worst pair: rush vs bulwark 97.2% -> 61.0%\n\
cross pairs in band 45-55%: 0/1 -> 1/2\n\
mirror first player <= 57%: 1/2 -> 1/1\n",
            rules()
        );
        assert_eq!(diff_reports(&before, &after), expected);
    }

    #[test]
    fn a_report_diffed_with_itself_has_only_zero_deltas() {
        let r = report(
            "starter@1.1.0",
            vec![(
                "random",
                vec![
                    cross("rush", "bulwark", 1500, 1100),
                    mirror("rush", 1200),
                    mirror("bulwark", 1000),
                ],
            )],
        );
        let text = diff_reports(&r, &r);
        assert!(!text.contains("NOTE"), "{text}");
        let rows: Vec<&str> = text
            .lines()
            .filter(|l| l.contains("rush") || l.contains("bulwark"))
            .filter(|l| l.contains(" = "))
            .collect();
        assert_eq!(rows.len(), 3, "{text}");
        for row in &rows {
            assert!(!row.contains("+") && !row.contains("-3"), "{row}");
            assert!(row.contains("no"), "{row}");
            assert!(!row.contains("yes"), "{row}");
        }
        assert!(rows[0].contains(" 0.0"), "{}", rows[0]);
    }

    #[test]
    fn delta_is_the_difference_of_the_displayed_integers_with_its_sign() {
        let one = |after_wins: u64| {
            let before = report("a@1", vec![("random", vec![cross("a", "b", 1000, 1000)])]);
            let after = report(
                "a@1",
                vec![("random", vec![cross("a", "b", after_wins, 1000)])],
            );
            diff_reports(&before, &after)
        };
        assert!(one(1724).contains("50.0% -> 86.2%  +36.2"), "{}", one(1724));
        assert!(one(1000).contains("50.0% -> 50.0%   0.0"), "{}", one(1000));
        // 998/2000 = 49.9%: a delta of -0.1.
        assert!(one(998).contains("50.0% -> 49.9%  -0.1"), "{}", one(998));
    }

    #[test]
    fn significance_is_the_disjointness_of_the_wilson_intervals() {
        let sig = |after_wins: u64| {
            let before = report("a@1", vec![("random", vec![cross("a", "b", 1000, 1000)])]);
            let after = report(
                "a@2",
                vec![("random", vec![cross("a", "b", after_wins, 1000)])],
            );
            let text = diff_reports(&before, &after);
            let row = text
                .lines()
                .find(|l| l.starts_with("a vs b"))
                .expect("a row")
                .to_owned();
            row.split_whitespace().nth(7).map(str::to_owned).unwrap()
        };
        // 50% vs 51%: the intervals (+-2.2 points) overlap.
        assert_eq!(sig(1020), "no");
        // 50% vs 60%: disjoint.
        assert_eq!(sig(1200), "yes");
    }

    #[test]
    fn different_parameters_are_noted_never_failed() {
        let before = report("a@1", vec![("random", vec![mirror("a", 1000)])]);
        let mut after = report("a@1", vec![("random", vec![mirror("a", 1000)])]);
        after.seed = 2;
        after.games_per_matchup = 500;
        after.max_actions = 99;
        after.rules.hero_riposte = !before.rules.hero_riposte;
        let text = diff_reports(&before, &after);
        let notes: Vec<&str> = text.lines().filter(|l| l.starts_with("NOTE:")).collect();
        assert_eq!(notes.len(), 4, "{text}");
        assert_eq!(notes[0], "NOTE: seed differs (1 vs 2)");
        assert_eq!(notes[1], "NOTE: games per matchup differs (2000 vs 500)");
        assert_eq!(notes[2], "NOTE: action cap differs (10000 vs 99)");
        assert_eq!(
            notes[3],
            format!(
                "NOTE: rules differs ({} vs {})",
                rules_label(&before.rules),
                rules_label(&after.rules)
            )
        );
        // Same set: no independent-sample note.
        assert!(!text.contains("independent samples"), "{text}");
    }

    #[test]
    fn a_policy_list_that_differs_is_noted_and_sections_are_new_or_removed() {
        let before = report(
            "a@1",
            vec![
                ("random", vec![mirror("a", 1000)]),
                ("old", vec![mirror("a", 1000)]),
            ],
        );
        let after = report(
            "a@1",
            vec![
                ("random", vec![mirror("a", 1000)]),
                ("greedy", vec![mirror("a", 1000)]),
            ],
        );
        let text = diff_reports(&before, &after);
        assert!(
            text.contains("NOTE: policies differs (random, old vs random, greedy)\n"),
            "{text}"
        );
        assert!(text.contains("policy: greedy (new)\n"), "{text}");
        assert!(text.contains("policy: old (removed)\n"), "{text}");
        assert!(text.contains("policy: random\n"), "{text}");
    }

    #[test]
    fn a_pair_without_a_finished_game_is_n_a() {
        let mut none = cross("a", "b", 0, 0);
        none.games = 10;
        none.interrupted = 10;
        none.wins_b = 0;
        none.hero_down = 0;
        let before = report("a@1", vec![("random", vec![cross("a", "b", 1000, 1000)])]);
        let after = report("a@1", vec![("random", vec![none])]);
        let text = diff_reports(&before, &after);
        let row = text
            .lines()
            .find(|l| l.starts_with("a vs b"))
            .expect("a row");
        let cells: Vec<&str> = row.split_whitespace().collect();
        assert_eq!(
            cells,
            ["a", "vs", "b", "=", "n/a", "n/a", "-", "n/a", "n/a", "n/a"],
            "{row}"
        );
    }

    #[test]
    fn pairs_are_matched_by_key_not_by_index() {
        let rows = |text: &str| -> Vec<String> {
            let mut rows: Vec<String> = text
                .lines()
                .filter(|l| l.contains(" vs ") || l.contains("(mirror)"))
                .filter(|l| !l.starts_with("worst"))
                .map(str::to_owned)
                .collect();
            rows.sort();
            rows
        };
        let ab = cross("a", "b", 1100, 1000);
        let aa = mirror("a", 1100);
        let bb = mirror("b", 1000);
        let before = report(
            "a@1",
            vec![("random", vec![ab.clone(), aa.clone(), bb.clone()])],
        );
        // The same pairs in another order: only the order of the lines changes.
        let reordered = report(
            "a@1",
            vec![("random", vec![bb.clone(), ab.clone(), aa.clone()])],
        );
        let straight = diff_reports(&before, &before);
        let shuffled = diff_reports(&before, &reordered);
        assert!(
            !shuffled.contains("new") && !shuffled.contains("removed"),
            "{shuffled}"
        );
        assert_eq!(rows(&straight), rows(&shuffled));
        // A deck inserted first moves every index: its pairs are new, the others keep a zero delta.
        let inserted = report(
            "a@1",
            vec![(
                "random",
                vec![
                    cross("z", "a", 900, 1000),
                    cross("z", "b", 900, 1000),
                    mirror("z", 1000),
                    ab,
                    aa,
                    bb,
                ],
            )],
        );
        let text = diff_reports(&before, &inserted);
        assert_eq!(text.matches(" new ").count(), 0, "status is a cell: {text}");
        let news = text
            .lines()
            .filter(|l| l.split_whitespace().any(|c| c == "new"))
            .count();
        assert_eq!(news, 3, "{text}");
        assert!(!text.contains("removed"), "{text}");
        let common = text.lines().filter(|l| l.contains(" = ")).count();
        assert_eq!(common, 3, "{text}");
        assert!(text.contains("a vs b"), "{text}");
        assert!(!text.contains("+0") && !text.contains("-0"), "{text}");
    }

    #[test]
    fn diff_is_deterministic() {
        let before = report(
            "a@1",
            vec![(
                "random",
                vec![cross("a", "b", 1100, 1000), mirror("a", 1200)],
            )],
        );
        let after = report(
            "a@2",
            vec![(
                "random",
                vec![cross("a", "b", 900, 1000), mirror("a", 1100)],
            )],
        );
        assert_eq!(diff_reports(&before, &after), diff_reports(&before, &after));
    }
}
