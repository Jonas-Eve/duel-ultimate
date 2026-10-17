//! The diff of two reports ([`diff_reports`]): pure, no I/O, no clock, no libm, no float in a
//! decision. The caller plays the two batches; nothing here parses a rendered report.
//!
//! Direction: `before` is the reference, `after` the candidate, delta = after - before.
//! Policies are paired by name; matchups by the key `(deck A, deck B, kind)`, never by index (the
//! index moves when a deck is added). A pair present on one side only is `new` / `removed`; a cross
//! pair whose decks are in the opposite order on the two sides is therefore `removed` + `new`
//! (the order of `decks.json` is canonical).
//!
//! The two batches are independent samples when the data differ: the seed of a game depends on
//! the index of its matchup (`game_seed`), and different data change the games anyway. Only a
//! set diffed with itself replays the very same games. The `significant` column is a conservative
//! heuristic (the 95% Wilson intervals do not overlap), and a delta inside the intervals is noise.
//!
//! Percentages are integer per-mille rounded half up ([`pm`]); a delta is the difference of the
//! displayed integers, so it always agrees with the numbers beside it.

use crate::report::{
    BAND_HIGH_PERCENT, BAND_LOW_PERCENT, Kind, MIRROR_FIRST_PLAYER_MAX_PERCENT, MatchupReport,
    PolicyReport, Report, rules_label, table,
};
use crate::stats::{fmt_mean, wilson_95};

const NOT_AVAILABLE: &str = "n/a";

const HEADERS: [&str; 8] = [
    "matchup",
    "status",
    "wins A",
    "delta",
    "significant",
    "first player",
    "mean actions",
    "exhaustion",
];

const SAMPLES_NOTE: &str = "NOTE: the two batches are independent samples (the seed of a game depends on the index of its matchup): a delta within the 95% intervals is noise";

const SIGNIFICANT_LEGEND: &str = "significant: yes when the 95% Wilson intervals do not overlap (conservative heuristic; cross pair: share of A, mirror: first player)";

/// `1000 n / d` rounded to the nearest integer, half up, in integers (`d > 0`).
fn pm(n: u64, d: u64) -> u128 {
    (2000 * u128::from(n) + u128::from(d)) / (2 * u128::from(d))
}

/// Per-mille as `"12.3%"`.
fn fmt_pm(value: u128) -> String {
    format!("{}.{}%", value / 10, value % 10)
}

/// The difference of two per-mille values as a signed number of points: `+36.2`, `-0.4`, `0.0`.
fn fmt_delta(before: u128, after: u128) -> String {
    let (sign, magnitude) = match after.cmp(&before) {
        std::cmp::Ordering::Greater => ("+", after - before),
        std::cmp::Ordering::Less => ("-", before - after),
        std::cmp::Ordering::Equal => ("", 0),
    };
    format!("{sign}{}.{}", magnitude / 10, magnitude % 10)
}

/// `before -> after`.
fn arrow(before: &str, after: &str) -> String {
    format!("{before} -> {after}")
}

/// The finished games of a matchup, or `None` without any.
fn finished_of(matchup: &MatchupReport) -> Option<u64> {
    matchup.finished().filter(|&finished| finished > 0)
}

/// The Wilson 95% intervals of two shares do not overlap.
fn disjoint(before: (u64, u64), after: (u64, u64)) -> Option<bool> {
    let (before_low, before_high) = wilson_95(before.0, before.1)?;
    let (after_low, after_high) = wilson_95(after.0, after.1)?;
    Some(before_high < after_low || after_high < before_low)
}

fn same_key(a: &MatchupReport, b: &MatchupReport) -> bool {
    a.deck_a == b.deck_a && a.deck_b == b.deck_b && a.kind == b.kind
}

/// The row of a pair present on one side only.
fn lone_row(matchup: &MatchupReport, status: &str) -> [String; 8] {
    let dash = || "-".to_owned();
    [
        matchup.name(),
        status.to_owned(),
        dash(),
        dash(),
        dash(),
        dash(),
        dash(),
        dash(),
    ]
}

/// The row of a pair present on both sides.
fn common_row(before: &MatchupReport, after: &MatchupReport) -> [String; 8] {
    let (Some(before_finished), Some(after_finished)) = (finished_of(before), finished_of(after))
    else {
        let na = || NOT_AVAILABLE.to_owned();
        return [
            after.name(),
            "=".to_owned(),
            na(),
            na(),
            "-".to_owned(),
            na(),
            na(),
            na(),
        ];
    };
    let share = |a: u64, b: u64| (pm(a, before_finished), pm(b, after_finished));
    let (wins_before, wins_after) = share(before.wins_a, after.wins_a);
    let (first_before, first_after) = share(before.first_player_wins, after.first_player_wins);
    let (exhaustion_before, exhaustion_after) = share(before.exhaustion, after.exhaustion);
    let (wins_cell, delta_cell, tested) = match after.kind {
        Kind::Cross => (
            arrow(&fmt_pm(wins_before), &fmt_pm(wins_after)),
            fmt_delta(wins_before, wins_after),
            (
                (before.wins_a, before_finished),
                (after.wins_a, after_finished),
            ),
        ),
        Kind::Mirror => (
            "-".to_owned(),
            "-".to_owned(),
            (
                (before.first_player_wins, before_finished),
                (after.first_player_wins, after_finished),
            ),
        ),
    };
    let significant = match disjoint(tested.0, tested.1) {
        Some(true) => "yes",
        Some(false) => "no",
        None => "-",
    };
    [
        after.name(),
        "=".to_owned(),
        wins_cell,
        delta_cell,
        significant.to_owned(),
        arrow(&fmt_pm(first_before), &fmt_pm(first_after)),
        arrow(
            &fmt_mean(before.total_actions, before_finished),
            &fmt_mean(after.total_actions, after_finished),
        ),
        arrow(&fmt_pm(exhaustion_before), &fmt_pm(exhaustion_after)),
    ]
}

/// `worst pair` of one side: `name 97.2%` or `n/a`.
fn worst_side(pair: Option<&crate::report::PairDeviation>) -> (String, String) {
    pair.map_or_else(
        || (NOT_AVAILABLE.to_owned(), NOT_AVAILABLE.to_owned()),
        |pair| (pair.matchup.clone(), fmt_pm(pm(pair.wins_a, pair.finished))),
    )
}

/// The block of a policy present on both sides: the table, the legend and the summary.
fn common_block(before: &PolicyReport, after: &PolicyReport) -> String {
    let mut rows: Vec<[String; 8]> = Vec::new();
    for matchup in &after.matchups {
        rows.push(
            match before.matchups.iter().find(|b| same_key(b, matchup)) {
                Some(reference) => common_row(reference, matchup),
                None => lone_row(matchup, "new"),
            },
        );
    }
    for matchup in &before.matchups {
        if !after.matchups.iter().any(|a| same_key(a, matchup)) {
            rows.push(lone_row(matchup, "removed"));
        }
    }
    let mut text = format!("policy: {}\n", after.policy);
    text += &table(HEADERS, &rows);
    text += SIGNIFICANT_LEGEND;
    text += "\n";
    let (before_summary, after_summary) = (before.summary(), after.summary());
    let (before_name, before_rate) = worst_side(before_summary.worst_pair.as_ref());
    let (after_name, after_rate) = worst_side(after_summary.worst_pair.as_ref());
    text += &if before_name == after_name {
        format!(
            "worst pair: {after_name} {}\n",
            arrow(&before_rate, &after_rate)
        )
    } else {
        let side = |name: &str, rate: &str| {
            if name == NOT_AVAILABLE {
                NOT_AVAILABLE.to_owned()
            } else {
                format!("{name} {rate}")
            }
        };
        format!(
            "worst pair: {}\n",
            arrow(
                &side(&before_name, &before_rate),
                &side(&after_name, &after_rate)
            )
        )
    };
    text += &format!(
        "cross pairs in band {BAND_LOW_PERCENT}-{BAND_HIGH_PERCENT}%: {}/{} -> {}/{}\n",
        before_summary.cross_in_band,
        before_summary.cross_pairs,
        after_summary.cross_in_band,
        after_summary.cross_pairs
    );
    text += &format!(
        "mirror first player <= {MIRROR_FIRST_PLAYER_MAX_PERCENT}%: {}/{} -> {}/{}\n",
        before_summary.mirror_first_player_ok,
        before_summary.mirrors,
        after_summary.mirror_first_player_ok,
        after_summary.mirrors
    );
    text
}

fn policy_names(report: &Report) -> String {
    let names: Vec<&str> = report.policies.iter().map(|p| p.policy.as_str()).collect();
    names.join(", ")
}

/// The diff between `before` (the reference) and `after` (the candidate), as text: a header,
/// `NOTE:` lines (never a failure), then one block per policy.
pub fn diff_reports(before: &Report, after: &Report) -> String {
    let mut text = format!(
        "diff: {} -> {} | seed: {} | games per matchup: {} | action cap: {} | policies: {} | rules: {}\n",
        before.set,
        after.set,
        after.seed,
        after.games_per_matchup,
        after.max_actions,
        policy_names(after),
        rules_label(&after.rules)
    );
    let mut differs = |field: &str, was: String, now: String| {
        if was != now {
            text += &format!("NOTE: {field} differs ({was} vs {now})\n");
        }
    };
    differs("seed", before.seed.to_string(), after.seed.to_string());
    differs(
        "games per matchup",
        before.games_per_matchup.to_string(),
        after.games_per_matchup.to_string(),
    );
    differs(
        "action cap",
        before.max_actions.to_string(),
        after.max_actions.to_string(),
    );
    differs(
        "rules",
        rules_label(&before.rules),
        rules_label(&after.rules),
    );
    differs("policies", policy_names(before), policy_names(after));
    if before.set != after.set {
        text += SAMPLES_NOTE;
        text += "\n";
    }
    let mut blocks: Vec<String> = Vec::new();
    for policy in &after.policies {
        blocks.push(
            match before.policies.iter().find(|b| b.policy == policy.policy) {
                Some(reference) => common_block(reference, policy),
                None => format!("policy: {} (new)\n", policy.policy),
            },
        );
    }
    for policy in &before.policies {
        if !after.policies.iter().any(|a| a.policy == policy.policy) {
            blocks.push(format!("policy: {} (removed)\n", policy.policy));
        }
    }
    text += &blocks.join("\n");
    text
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
matchup            status          wins A  delta  significant    first player  mean actions    exhaustion\n\
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
        assert!(one(1000).contains("50.0% -> 50.0%    0.0"), "{}", one(1000));
        // 998/2000 = 49.9%: a delta of -0.1.
        assert!(one(998).contains("50.0% -> 49.9%   -0.1"), "{}", one(998));
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
            row.split_whitespace().nth(8).map(str::to_owned).unwrap()
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
