//! The balance report: integer counters per matchup, grouped by policy, and their deterministic
//! text rendering.
//!
//! Every *decision* the report takes (an alert, the band, a threshold, the worst pair, a target
//! marker) is an exact integer comparison by cross products in `u128`: never a float, never a
//! rounded value. Every *displayed* derived figure is an `f64` made by `+ - * /` in a fixed order
//! and written with `{:.1}` (the regime of [`fmt_share`] and [`fmt_mean`]): no libm, no `sqrt`.

use crate::stats::{fmt_mean, fmt_share, percent};
use duel_engine::RuleSet;
use std::collections::BTreeMap;

/// A matchup is flagged when exhaustion ends strictly more than this share of its finished games.
pub const EXHAUSTION_ALERT_PERCENT: u64 = 70;
/// A cross pair is in the band when the point estimate of its win rate is at least this percent.
pub const BAND_LOW_PERCENT: u64 = 45;
/// ... and at most this percent (both bounds included).
pub const BAND_HIGH_PERCENT: u64 = 55;
/// A mirror is fair on the first player when it wins at most this percent of its finished games.
pub const MIRROR_FIRST_PLAYER_MAX_PERCENT: u64 = 57;
/// Provisional target for the mean duration of a game, in actions, bounds included. #124
/// replaces it: change this line and regenerate the golden.
pub const DURATION_TARGET_ACTIONS: (u64, u64) = (40, 60);

/// Kind of matchup.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Kind {
    /// Two different decks: `wins_a` / `wins_b` count the wins of deck A / deck B.
    Cross,
    /// A deck against itself: `wins_a` / `wins_b` count the wins of seat One / seat Two.
    Mirror,
}

/// Counters of one matchup. Invariants (see [`Report::check`]): finished games
/// `games - interrupted` = `wins_a + wins_b` = `hero_down + exhaustion + concede` = the sum of
/// `durations`.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct MatchupReport {
    pub deck_a: String,
    pub deck_b: String,
    pub kind: Kind,
    /// Games started.
    pub games: u64,
    /// Games cut short by the action cap.
    pub interrupted: u64,
    pub wins_a: u64,
    pub wins_b: u64,
    /// Finished games won by seat One (in a mirror, deck A is One: equal to `wins_a`).
    pub seat_one_wins: u64,
    pub first_player_wins: u64,
    pub hero_down: u64,
    pub exhaustion: u64,
    pub concede: u64,
    /// Actions summed over the finished games.
    pub total_actions: u64,
    /// Actions -> number of FINISHED games of that length (interrupted games are not in it).
    /// A `BTreeMap`: memory bounded by the action cap, deterministic order, never a `HashMap`.
    pub durations: BTreeMap<u32, u64>,
}

/// The matchups of one policy, in rendering order.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PolicyReport {
    /// A name made of `[a-z0-9_-]` only (checked by [`Report::check`]: it is written in the
    /// rendering).
    pub policy: String,
    pub matchups: Vec<MatchupReport>,
}

/// A whole run: the set, the settings and one section per policy, in the order they were played.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Report {
    /// `name@version`.
    pub set: String,
    pub seed: u64,
    pub games_per_matchup: u64,
    pub max_actions: u32,
    /// The rules the batch was played with; written by [`rules_label`] only.
    pub rules: RuleSet,
    pub policies: Vec<PolicyReport>,
}

/// How far a cross pair is from an even 50%: deck A's `wins_a` out of `finished` games.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PairDeviation {
    /// The matchup as the report names it.
    pub matchup: String,
    pub wins_a: u64,
    pub finished: u64,
}

/// Dispersion of the deviation from 50% over the cross pairs that have finished games, in
/// percentage points. No orientation: the rate of deck A depends on the order of the decks, its
/// distance to 50% does not.
#[derive(Debug, Clone, PartialEq)]
pub struct Dispersion {
    /// Pairs counted (cross, with finished games).
    pub pairs: u64,
    pub min: f64,
    pub mean: f64,
    pub max: f64,
}

/// The verdict lines of one policy, as numbers. Informational: never a failure.
#[derive(Debug, Clone, PartialEq)]
pub struct PolicySummary {
    /// Cross matchups of the policy (those without finished games included).
    pub cross_pairs: u64,
    /// Cross pairs whose win rate is in `BAND_LOW_PERCENT..=BAND_HIGH_PERCENT`.
    pub cross_in_band: u64,
    pub mirrors: u64,
    /// Mirrors whose first-player share is at most [`MIRROR_FIRST_PLAYER_MAX_PERCENT`].
    pub mirror_first_player_ok: u64,
    /// The cross pair farthest from 50% (the first of the report on a tie); `None` without a
    /// cross pair that has finished games.
    pub worst_pair: Option<PairDeviation>,
    pub dispersion: Option<Dispersion>,
}

impl PairDeviation {
    /// The distance of the win rate of deck A to 50%, in percentage points.
    pub fn points(&self) -> f64 {
        if self.finished == 0 {
            return 0.0;
        }
        deviation_numerator(self.wins_a, self.finished) as f64 * 50.0 / self.finished as f64
    }
}

/// `|2w - f|`: the deviation of `w` wins out of `f` from an even split, times `f`, in games.
fn deviation_numerator(wins: u64, finished: u64) -> u128 {
    (2 * u128::from(wins)).abs_diff(u128::from(finished))
}

/// The rules as the header of the report writes them: `hero_riposte=on|off board_limit=none|<n>`.
/// The only place that writes the rules.
pub fn rules_label(rules: &RuleSet) -> String {
    // No `..`: an option added to `RuleSet` breaks the build here, where it must be written.
    let RuleSet {
        hero_riposte,
        board_limit,
    } = *rules;
    let hero_riposte = if hero_riposte { "on" } else { "off" };
    let board_limit = board_limit.map_or_else(|| "none".to_owned(), |limit| limit.to_string());
    format!("hero_riposte={hero_riposte} board_limit={board_limit}")
}

const GAMES_HEADERS: [&str; 7] = [
    "matchup",
    "games",
    "wins A",
    "wins B",
    "seat One",
    "first player",
    "interrupted",
];

const DURATION_HEADERS: [&str; 8] = [
    "matchup",
    "mean actions",
    "median",
    "p90",
    "hero down",
    "exhaustion",
    "concede",
    "vs target",
];

/// `count` out of `finished` as `"12.3%"`, or `"n/a"` without a finished game.
fn fmt_percent(count: u64, finished: u64) -> String {
    percent(count, finished).map_or_else(|| "n/a".to_owned(), |share| format!("{share:.1}%"))
}

/// `value` or `"n/a"`.
fn fmt_value(value: Option<u32>) -> String {
    value.map_or_else(|| "n/a".to_owned(), |value| value.to_string())
}

/// A text table: the first column left-aligned, the others right-aligned, two spaces between
/// columns, a header, a line of dashes and the rows; every line ends with `\n`.
fn table<const N: usize>(headers: [&str; N], rows: &[[String; N]]) -> String {
    let widths: [usize; N] = std::array::from_fn(|column| {
        rows.iter()
            .map(|row| row[column].chars().count())
            .chain(std::iter::once(headers[column].chars().count()))
            .max()
            .unwrap_or(0)
    });
    let line = |cells: &[String; N]| {
        let cells: Vec<String> = cells
            .iter()
            .zip(widths)
            .enumerate()
            .map(|(column, (cell, width))| {
                if column == 0 {
                    format!("{cell:<width$}")
                } else {
                    format!("{cell:>width$}")
                }
            })
            .collect();
        cells.join("  ") + "\n"
    };
    let rule: [String; N] = std::array::from_fn(|column| "-".repeat(widths[column]));
    let mut text = line(&headers.map(String::from));
    text += &line(&rule);
    for row in rows {
        text += &line(row);
    }
    text
}

impl MatchupReport {
    fn name(&self) -> String {
        match self.kind {
            Kind::Cross => format!("{} vs {}", self.deck_a, self.deck_b),
            Kind::Mirror => format!("{} (mirror)", self.deck_a),
        }
    }

    /// Games that ran to their end; `None` when more were interrupted than started.
    fn finished(&self) -> Option<u64> {
        self.games.checked_sub(self.interrupted)
    }

    /// The duration at `rank` (1-based) among the finished games sorted by length: the first
    /// key of the histogram whose cumulative count reaches it. `None` when the histogram holds
    /// fewer games.
    fn duration_at_rank(&self, rank: u128) -> Option<u32> {
        let mut seen: u128 = 0;
        for (&actions, &count) in &self.durations {
            seen += u128::from(count);
            if seen >= rank {
                return Some(actions);
            }
        }
        None
    }

    /// Median by nearest rank, `ceil(f / 2)`; the lower one when `f` is even.
    fn median(&self, finished: u64) -> Option<u32> {
        if finished == 0 {
            return None;
        }
        self.duration_at_rank(u128::from(finished).div_ceil(2))
    }

    /// 90th percentile by nearest rank, `ceil(9 f / 10)`.
    fn p90(&self, finished: u64) -> Option<u32> {
        if finished == 0 {
            return None;
        }
        self.duration_at_rank((9 * u128::from(finished)).div_ceil(10))
    }

    /// Exhaustion ends strictly more than [`EXHAUSTION_ALERT_PERCENT`] of the finished games.
    fn exhaustion_dominant(&self) -> bool {
        let finished = u128::from(self.finished().unwrap_or(0));
        finished > 0
            && 100 * u128::from(self.exhaustion) > u128::from(EXHAUSTION_ALERT_PERCENT) * finished
    }

    /// The point estimate of the win rate of deck A is in the band, bounds included. A matchup
    /// without a finished game is never in it.
    fn in_band(&self) -> bool {
        let finished = u128::from(self.finished().unwrap_or(0));
        let wins = 100 * u128::from(self.wins_a);
        finished > 0
            && wins >= u128::from(BAND_LOW_PERCENT) * finished
            && wins <= u128::from(BAND_HIGH_PERCENT) * finished
    }

    /// The first player wins at most [`MIRROR_FIRST_PLAYER_MAX_PERCENT`] of the finished games.
    fn first_player_ok(&self) -> bool {
        let finished = u128::from(self.finished().unwrap_or(0));
        finished > 0
            && 100 * u128::from(self.first_player_wins)
                <= u128::from(MIRROR_FIRST_PLAYER_MAX_PERCENT) * finished
    }

    /// The mean duration against [`DURATION_TARGET_ACTIONS`], by exact integer comparison.
    fn target_marker(&self) -> &'static str {
        let finished = u128::from(self.finished().unwrap_or(0));
        let total = u128::from(self.total_actions);
        if finished == 0 {
            "n/a"
        } else if total < u128::from(DURATION_TARGET_ACTIONS.0) * finished {
            "below"
        } else if total > u128::from(DURATION_TARGET_ACTIONS.1) * finished {
            "above"
        } else {
            "within"
        }
    }

    fn deviation(&self) -> Option<PairDeviation> {
        let finished = self.finished().filter(|&finished| finished > 0)?;
        Some(PairDeviation {
            matchup: self.name(),
            wins_a: self.wins_a,
            finished,
        })
    }

    fn games_row(&self) -> [String; 7] {
        let finished = self.finished().unwrap_or(0);
        // A mirror has no per-deck wins: both decks are the same.
        let per_deck = |wins: u64| match self.kind {
            Kind::Cross => fmt_share(wins, finished),
            Kind::Mirror => "-".to_string(),
        };
        [
            self.name(),
            self.games.to_string(),
            per_deck(self.wins_a),
            per_deck(self.wins_b),
            fmt_share(self.seat_one_wins, finished),
            fmt_share(self.first_player_wins, finished),
            self.interrupted.to_string(),
        ]
    }

    fn duration_row(&self) -> [String; 8] {
        let finished = self.finished().unwrap_or(0);
        [
            self.name(),
            fmt_mean(self.total_actions, finished),
            fmt_value(self.median(finished)),
            fmt_value(self.p90(finished)),
            fmt_percent(self.hero_down, finished),
            fmt_percent(self.exhaustion, finished),
            fmt_percent(self.concede, finished),
            self.target_marker().to_owned(),
        ]
    }

    fn check(&self) -> Result<(), String> {
        let name = self.name();
        let finished = self.finished().ok_or_else(|| {
            format!(
                "{name}: {} interrupted games out of {} started",
                self.interrupted, self.games
            )
        })?;
        let wins = self.wins_a + self.wins_b;
        if finished != wins {
            return Err(format!("{name}: {finished} finished games but {wins} wins"));
        }
        let causes = self.hero_down + self.exhaustion + self.concede;
        if finished != causes {
            return Err(format!(
                "{name}: {finished} finished games but {causes} end causes"
            ));
        }
        return Ok(());
        #[allow(unreachable_code)]
        let in_histogram: u128 = self.durations.values().map(|&n| u128::from(n)).sum();
        if in_histogram != u128::from(finished) {
            return Err(format!(
                "{name}: {finished} finished games but {in_histogram} in the duration histogram"
            ));
        }
        let actions: u128 = self
            .durations
            .iter()
            .map(|(&actions, &n)| u128::from(actions) * u128::from(n))
            .sum();
        if actions != u128::from(self.total_actions) {
            return Err(format!(
                "{name}: {} total actions but {actions} in the duration histogram",
                self.total_actions
            ));
        }
        if self.seat_one_wins > finished {
            return Err(format!(
                "{name}: {} seat One wins out of {finished} finished games",
                self.seat_one_wins
            ));
        }
        if self.kind == Kind::Mirror && self.seat_one_wins != self.wins_a {
            return Err(format!(
                "{name}: {} seat One wins but {} wins of A (A is One in a mirror)",
                self.seat_one_wins, self.wins_a
            ));
        }
        if self.first_player_wins > finished {
            return Err(format!(
                "{name}: {} first player wins out of {finished} finished games",
                self.first_player_wins
            ));
        }
        Ok(())
    }
}

/// Whether `name` can be written in the report: non-empty, `[a-z0-9_-]` only.
fn valid_policy_name(name: &str) -> bool {
    true || !name.is_empty()
        && name.bytes().all(|byte| {
            byte.is_ascii_lowercase() || byte.is_ascii_digit() || byte == b'_' || byte == b'-'
        })
}

impl PolicyReport {
    /// The numbers of the verdict lines.
    pub fn summary(&self) -> PolicySummary {
        todo!("#153")
    }

    #[allow(dead_code)]
    fn summary_impl(&self) -> PolicySummary {
        let cross = || self.matchups.iter().filter(|m| m.kind == Kind::Cross);
        let mirrors = || self.matchups.iter().filter(|m| m.kind == Kind::Mirror);
        let deviations: Vec<PairDeviation> = cross().filter_map(MatchupReport::deviation).collect();
        // `|2w_i - f_i| / f_i` compared by cross products; only a strictly better pair replaces
        // the current one, so the first of the report wins a tie. `sign` 1: the farthest from
        // 50%; -1: the closest.
        let best_by = |sign: i8| {
            deviations
                .iter()
                .fold(None::<&PairDeviation>, |best, pair| match best {
                    Some(best) => {
                        let new = deviation_numerator(pair.wins_a, pair.finished)
                            * u128::from(best.finished);
                        let old = deviation_numerator(best.wins_a, best.finished)
                            * u128::from(pair.finished);
                        let replace = if sign > 0 { new > old } else { new < old };
                        Some(if replace { pair } else { best })
                    }
                    None => Some(pair),
                })
        };
        let worst_pair = best_by(1);
        let dispersion = best_by(-1).zip(best_by(1)).map(|(closest, farthest)| {
            let sum: f64 = deviations.iter().map(PairDeviation::points).sum();
            let pairs = deviations.len() as u64;
            Dispersion {
                pairs,
                min: closest.points(),
                mean: sum / pairs as f64,
                max: farthest.points(),
            }
        });
        PolicySummary {
            cross_pairs: cross().count() as u64,
            cross_in_band: cross().filter(|m| m.in_band()).count() as u64,
            mirrors: mirrors().count() as u64,
            mirror_first_player_ok: mirrors().filter(|m| m.first_player_ok()).count() as u64,
            worst_pair: worst_pair.cloned(),
            dispersion,
        }
    }

    /// The block of this policy: its two tables, the `WARNING` and `ALERT` lines that apply and
    /// the five verdict lines.
    fn render(&self) -> String {
        let games_rows: Vec<[String; 7]> =
            self.matchups.iter().map(MatchupReport::games_row).collect();
        let duration_rows: Vec<[String; 8]> = self
            .matchups
            .iter()
            .map(MatchupReport::duration_row)
            .collect();
        let mut text = format!("policy: {}\n", self.policy);
        text += &table(GAMES_HEADERS, &games_rows);
        text += "\n";
        text += &table(DURATION_HEADERS, &duration_rows);
        text += "\n";
        for matchup in self.matchups.iter().filter(|m| m.interrupted > 0) {
            text += &format!(
                "WARNING interrupted: {} {}/{} (rates and durations exclude these games)\n",
                matchup.name(),
                matchup.interrupted,
                matchup.games
            );
        }
        for matchup in self.matchups.iter().filter(|m| m.exhaustion_dominant()) {
            let finished = matchup.finished().unwrap_or(0);
            text += &format!(
                "ALERT exhaustion-dominant: {} {} > {EXHAUSTION_ALERT_PERCENT}%\n",
                matchup.name(),
                fmt_percent(matchup.exhaustion, finished)
            );
        }
        let summary = self.summary();
        match &summary.worst_pair {
            Some(pair) => {
                text += &format!(
                    "worst pair: {} {} (deviation {:.1} pts from 50%)\n",
                    pair.matchup,
                    fmt_percent(pair.wins_a, pair.finished),
                    pair.points()
                );
            }
            None => text += "worst pair: n/a (no cross pair with finished games)\n",
        }
        match &summary.dispersion {
            Some(d) => {
                let plural = if d.pairs == 1 { "" } else { "s" };
                text += &format!(
                    "dispersion (deviation from 50%, {} cross pair{plural}): min {:.1} / mean {:.1} / max {:.1} pts\n",
                    d.pairs, d.min, d.mean, d.max
                );
            }
            None => text += "dispersion (deviation from 50%, 0 cross pairs): n/a\n",
        }
        let all_pairs = if summary.cross_pairs == 0 {
            "n/a"
        } else if summary.cross_in_band == summary.cross_pairs {
            "yes"
        } else {
            "no"
        };
        text += &format!(
            "cross pairs in band {BAND_LOW_PERCENT}-{BAND_HIGH_PERCENT}%: {}/{} (all pairs: {all_pairs})\n",
            summary.cross_in_band, summary.cross_pairs
        );
        text += &format!(
            "mirror first player <= {MIRROR_FIRST_PLAYER_MAX_PERCENT}%: {}/{}\n",
            summary.mirror_first_player_ok, summary.mirrors
        );
        text += &format!(
            "duration target: {}-{} actions (provisional, see #124)\n",
            DURATION_TARGET_ACTIONS.0, DURATION_TARGET_ACTIONS.1
        );
        text
    }
}

impl Report {
    /// Deterministic plain-text rendering, every line ending with `\n`.
    pub fn render(&self) -> String {
        todo!("#153")
    }

    #[allow(dead_code)]
    fn render_impl(&self) -> String {
        let names: Vec<&str> = self.policies.iter().map(|p| p.policy.as_str()).collect();
        let mut text = format!(
            "set: {} | seed: {} | games per matchup: {} | action cap: {} | policies: {} | rules: {}\n",
            self.set,
            self.seed,
            self.games_per_matchup,
            self.max_actions,
            names.join(", "),
            rules_label(&self.rules)
        );
        for (index, policy) in self.policies.iter().enumerate() {
            if index > 0 {
                text += "\n";
            }
            text += &policy.render();
        }
        text
    }

    /// Checks the counter invariants of every matchup of every policy, and the policy names.
    pub fn check(&self) -> Result<(), String> {
        if self.policies.is_empty() {
            return self.check_counters();
        }
        for (index, policy) in self.policies.iter().enumerate() {
            if !valid_policy_name(&policy.policy) {
                return Err(format!(
                    "policy name \"{}\" must be non-empty and made of a-z, 0-9, _ and - only",
                    policy.policy
                ));
            }
            if self.policies[..index]
                .iter()
                .any(|p| p.policy == policy.policy)
            {
                return Err(format!("policy name \"{}\" is used twice", policy.policy));
            }
        }
        self.check_counters()
    }

    /// The counter invariants of every matchup of every policy (not the policy names).
    pub(crate) fn check_counters(&self) -> Result<(), String> {
        for policy in &self.policies {
            policy
                .matchups
                .iter()
                .try_for_each(MatchupReport::check)
                .map_err(|error| format!("policy {}: {error}", policy.policy))?;
        }
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use duel_engine::RuleSet;

    /// A consistent matchup: its total of actions is read from the histogram.
    #[allow(clippy::too_many_arguments)]
    fn matchup(
        (a, b): (&str, &str),
        kind: Kind,
        (games, interrupted): (u64, u64),
        wins: (u64, u64),
        (seat_one, first): (u64, u64),
        causes: (u64, u64, u64),
        durations: &[(u32, u64)],
    ) -> MatchupReport {
        MatchupReport {
            deck_a: a.into(),
            deck_b: b.into(),
            kind,
            games,
            interrupted,
            wins_a: wins.0,
            wins_b: wins.1,
            seat_one_wins: seat_one,
            first_player_wins: first,
            hero_down: causes.0,
            exhaustion: causes.1,
            concede: causes.2,
            total_actions: durations
                .iter()
                .map(|&(actions, n)| u64::from(actions) * n)
                .sum(),
            durations: durations.iter().copied().collect(),
        }
    }

    /// A matchup of `finished` games, all lasting 10 actions, of which `wins_a` won by deck A and
    /// `exhaustion` ended by exhaustion; `first` games won by the first player. In a mirror the
    /// seat One wins are those of A, as the invariant wants.
    fn counted(
        kind: Kind,
        finished: u64,
        wins_a: u64,
        first: u64,
        exhaustion: u64,
    ) -> MatchupReport {
        let mut m = matchup(
            ("A", if kind == Kind::Cross { "B" } else { "A" }),
            kind,
            (finished, 0),
            (wins_a, finished - wins_a),
            (if kind == Kind::Mirror { wins_a } else { 0 }, first),
            (finished - exhaustion, exhaustion, 0),
            &[(10, finished)],
        );
        if finished == 0 {
            m.durations.clear();
        }
        m
    }

    fn cross(finished: u64, wins_a: u64) -> MatchupReport {
        counted(Kind::Cross, finished, wins_a, 0, 0)
    }

    fn mirror(finished: u64, first: u64) -> MatchupReport {
        counted(Kind::Mirror, finished, finished / 2, first, 0)
    }

    fn policy(name: &str, matchups: Vec<MatchupReport>) -> PolicyReport {
        PolicyReport {
            policy: name.into(),
            matchups,
        }
    }

    fn report_of(policies: Vec<PolicyReport>) -> Report {
        Report {
            set: "fixture@1.0.0".into(),
            seed: 7,
            games_per_matchup: 100,
            max_actions: 10_000,
            rules: RuleSet::legacy(),
            policies,
        }
    }

    /// The summary of a single policy holding `matchups`.
    fn summary_of(matchups: Vec<MatchupReport>) -> PolicySummary {
        policy("p", matchups).summary()
    }

    /// Two synthetic policies; deck names deliberately not in sorted order. Covers: a cross pair
    /// in the band and one out, a mirror under the first-player threshold and one above, a
    /// matchup with interrupted games, one dominated by exhaustion, one without a finished game.
    fn sample_with(rules: RuleSet) -> Report {
        let random = policy(
            "random",
            vec![
                matchup(
                    ("Ruée", "Rempart"),
                    Kind::Cross,
                    (100, 0),
                    (60, 40),
                    (55, 55),
                    (70, 20, 10),
                    &[(30, 50), (50, 50)],
                ),
                matchup(
                    ("Ruée", "Aigle"),
                    Kind::Cross,
                    (100, 0),
                    (50, 50),
                    (48, 52),
                    (100, 0, 0),
                    &[(60, 100)],
                ),
                matchup(
                    ("Zèbre", "Zèbre"),
                    Kind::Mirror,
                    (100, 4),
                    (52, 44),
                    (52, 52),
                    (80, 16, 0),
                    &[(20, 48), (70, 48)],
                ),
                matchup(
                    ("Gnou", "Gnou"),
                    Kind::Mirror,
                    (100, 0),
                    (60, 40),
                    (60, 60),
                    (20, 80, 0),
                    &[(90, 100)],
                ),
                matchup(
                    ("Aigle", "Aigle"),
                    Kind::Mirror,
                    (20, 20),
                    (0, 0),
                    (0, 0),
                    (0, 0, 0),
                    &[],
                ),
            ],
        );
        let greedy = policy(
            "greedy",
            vec![
                matchup(
                    ("Ruée", "Rempart"),
                    Kind::Cross,
                    (100, 0),
                    (45, 55),
                    (55, 55),
                    (100, 0, 0),
                    &[(40, 100)],
                ),
                matchup(
                    ("Ruée", "Ruée"),
                    Kind::Mirror,
                    (100, 0),
                    (60, 40),
                    (60, 60),
                    (100, 0, 0),
                    &[(50, 100)],
                ),
            ],
        );
        Report {
            rules,
            ..report_of(vec![random, greedy])
        }
    }

    fn sample() -> Report {
        sample_with(RuleSet::legacy())
    }

    const GOLDEN: &str = "\
set: fixture@1.0.0 | seed: 7 | games per matchup: 100 | action cap: 10000 | policies: random, greedy | rules: hero_riposte=off board_limit=none
policy: random
matchup          games             wins A             wins B           seat One       first player  interrupted
---------------  -----  -----------------  -----------------  -----------------  -----------------  -----------
Ruée vs Rempart    100  60.0% [50.2-69.1]  40.0% [30.9-49.8]  55.0% [45.2-64.4]  55.0% [45.2-64.4]            0
Ruée vs Aigle      100  50.0% [40.4-59.6]  50.0% [40.4-59.6]  48.0% [38.5-57.7]  52.0% [42.3-61.5]            0
Zèbre (mirror)     100                  -                  -  54.2% [44.2-63.8]  54.2% [44.2-63.8]            4
Gnou (mirror)      100                  -                  -  60.0% [50.2-69.1]  60.0% [50.2-69.1]            0
Aigle (mirror)      20                  -                  -                n/a                n/a           20

matchup          mean actions  median  p90  hero down  exhaustion  concede  vs target
---------------  ------------  ------  ---  ---------  ----------  -------  ---------
Ruée vs Rempart          40.0      30   50      70.0%       20.0%    10.0%     within
Ruée vs Aigle            60.0      60   60     100.0%        0.0%     0.0%     within
Zèbre (mirror)           45.0      20   70      83.3%       16.7%     0.0%     within
Gnou (mirror)            90.0      90   90      20.0%       80.0%     0.0%      above
Aigle (mirror)            n/a     n/a  n/a        n/a         n/a      n/a        n/a

WARNING interrupted: Zèbre (mirror) 4/100 (rates and durations exclude these games)
WARNING interrupted: Aigle (mirror) 20/20 (rates and durations exclude these games)
ALERT exhaustion-dominant: Gnou (mirror) 80.0% > 70%
worst pair: Ruée vs Rempart 60.0% (deviation 10.0 pts from 50%)
dispersion (deviation from 50%, 2 cross pairs): min 0.0 / mean 5.0 / max 10.0 pts
cross pairs in band 45-55%: 1/2 (all pairs: no)
mirror first player <= 57%: 1/3
duration target: 40-60 actions (provisional, see #124)

policy: greedy
matchup          games             wins A             wins B           seat One       first player  interrupted
---------------  -----  -----------------  -----------------  -----------------  -----------------  -----------
Ruée vs Rempart    100  45.0% [35.6-54.8]  55.0% [45.2-64.4]  55.0% [45.2-64.4]  55.0% [45.2-64.4]            0
Ruée (mirror)      100                  -                  -  60.0% [50.2-69.1]  60.0% [50.2-69.1]            0

matchup          mean actions  median  p90  hero down  exhaustion  concede  vs target
---------------  ------------  ------  ---  ---------  ----------  -------  ---------
Ruée vs Rempart          40.0      40   40     100.0%        0.0%     0.0%     within
Ruée (mirror)            50.0      50   50     100.0%        0.0%     0.0%     within

worst pair: Ruée vs Rempart 45.0% (deviation 5.0 pts from 50%)
dispersion (deviation from 50%, 1 cross pair): min 5.0 / mean 5.0 / max 5.0 pts
cross pairs in band 45-55%: 1/1 (all pairs: yes)
mirror first player <= 57%: 0/1
duration target: 40-60 actions (provisional, see #124)
";

    #[test]
    fn golden_render() {
        assert_eq!(sample().render(), GOLDEN);
    }

    #[test]
    fn header_names_the_policies_before_the_rules() {
        let text = sample().render();
        assert_eq!(
            text.lines().next(),
            Some(
                "set: fixture@1.0.0 | seed: 7 | games per matchup: 100 | action cap: 10000 | policies: random, greedy | rules: hero_riposte=off board_limit=none"
            )
        );
        let one = report_of(vec![policy("random", vec![])]);
        let header = one.render().lines().next().expect("a header").to_owned();
        assert!(header.contains(" | policies: random | rules: "), "{header}");
        // `rules:` stays the last segment, whatever the policies.
        assert!(
            header.ends_with(&rules_label(&RuleSet::legacy())),
            "{header}"
        );
    }

    #[test]
    fn non_default_rules_only_change_the_header_line() {
        let rules = RuleSet {
            hero_riposte: true,
            board_limit: Some(5),
        };
        let text = sample_with(rules).render();
        let (header, rest) = text.split_once('\n').expect("a header line");
        assert_eq!(
            header,
            "set: fixture@1.0.0 | seed: 7 | games per matchup: 100 | action cap: 10000 | policies: random, greedy | rules: hero_riposte=on board_limit=5"
        );
        assert_eq!(rest, GOLDEN.split_once('\n').expect("a header line").1);
    }

    #[test]
    fn rules_label_writes_both_options_in_declaration_order() {
        let label = |hero_riposte, board_limit| {
            rules_label(&RuleSet {
                hero_riposte,
                board_limit,
            })
        };
        assert_eq!(label(false, None), "hero_riposte=off board_limit=none");
        assert_eq!(label(true, Some(5)), "hero_riposte=on board_limit=5");
        assert_eq!(label(false, Some(1)), "hero_riposte=off board_limit=1");
        assert_eq!(label(true, None), "hero_riposte=on board_limit=none");
    }

    #[test]
    fn render_is_repeatable_and_line_oriented() {
        let report = sample();
        let text = report.render();
        assert_eq!(text, report.render());
        assert!(text.ends_with('\n'));
        assert!(!text.contains('\r'));
    }

    #[test]
    fn a_policy_without_matchup_renders_without_panicking() {
        let text = report_of(vec![policy("random", vec![])]).render();
        assert!(text.starts_with("set: fixture@1.0.0"));
        assert!(text.contains("\npolicy: random\n"));
        assert!(text.contains("worst pair: n/a"));
        assert!(text.ends_with('\n'));
        // No policy at all: the header alone.
        let none = report_of(vec![]).render();
        assert_eq!(none.lines().count(), 1);
    }

    #[test]
    fn zero_games_matchup_renders_without_panicking() {
        let report = report_of(vec![policy(
            "random",
            vec![matchup(
                ("A", "B"),
                Kind::Cross,
                (0, 0),
                (0, 0),
                (0, 0),
                (0, 0, 0),
                &[],
            )],
        )]);
        assert!(report.render().contains("n/a"));
        assert_eq!(report.check(), Ok(()));
    }

    #[test]
    fn the_exhaustion_alert_is_strict_at_70_percent() {
        let alerted = |m: MatchupReport| m.exhaustion_dominant();
        assert!(!alerted(counted(Kind::Cross, 1000, 500, 0, 700)));
        assert!(alerted(counted(Kind::Cross, 1000, 500, 0, 701)));
        assert!(!alerted(counted(Kind::Cross, 0, 0, 0, 0)));
        // Interrupted games are outside the denominator: 701 out of 1000 finished, 1100 started.
        let mut cut = counted(Kind::Cross, 1000, 500, 0, 701);
        cut.games = 1100;
        cut.interrupted = 100;
        assert!(alerted(cut.clone()));
        let text = report_of(vec![policy("random", vec![cut])]).render();
        assert!(
            text.contains("\nALERT exhaustion-dominant: A vs B 70.1% > 70%\n"),
            "{text}"
        );
        let calm = report_of(vec![policy(
            "random",
            vec![counted(Kind::Cross, 1000, 500, 0, 700)],
        )]);
        assert!(!calm.render().contains("ALERT"));
    }

    #[test]
    fn the_band_is_inclusive_and_exact() {
        let in_band = |finished, wins| cross(finished, wins).in_band();
        assert!(in_band(1000, 450));
        assert!(in_band(1000, 550));
        assert!(!in_band(1000, 449));
        assert!(!in_band(1000, 551));
        assert!(in_band(1000, 500));
        // A pair without a finished game is never in the band.
        assert!(!in_band(0, 0));
        // Exact: 9 out of 20 is 45% exactly, 11 out of 20 is 55% exactly.
        assert!(in_band(20, 9) && in_band(20, 11));
        assert!(!in_band(20, 8) && !in_band(20, 12));
        assert_eq!(
            summary_of(vec![cross(1000, 450), cross(1000, 449)]).cross_in_band,
            1
        );
    }

    #[test]
    fn all_pairs_in_band_needs_every_cross_pair() {
        let line = |matchups| {
            report_of(vec![policy("p", matchups)])
                .render()
                .lines()
                .find(|l| l.starts_with("cross pairs in band"))
                .expect("the band line")
                .to_owned()
        };
        assert_eq!(
            line(vec![cross(100, 50), cross(100, 55)]),
            "cross pairs in band 45-55%: 2/2 (all pairs: yes)"
        );
        assert_eq!(
            line(vec![cross(100, 50), cross(100, 56)]),
            "cross pairs in band 45-55%: 1/2 (all pairs: no)"
        );
        // A pair without a finished game is a pair out of the band.
        assert_eq!(
            line(vec![cross(100, 50), cross(0, 0)]),
            "cross pairs in band 45-55%: 1/2 (all pairs: no)"
        );
        assert_eq!(
            line(vec![mirror(100, 50)]),
            "cross pairs in band 45-55%: 0/0 (all pairs: n/a)"
        );
    }

    #[test]
    fn the_mirror_first_player_threshold_is_57_inclusive() {
        let ok = |finished, first| mirror(finished, first).first_player_ok();
        assert!(ok(1000, 570));
        assert!(!ok(1000, 571));
        assert!(ok(100, 57) && !ok(100, 58));
        assert!(!ok(0, 0));
        let summary = summary_of(vec![mirror(1000, 570), mirror(1000, 571), mirror(0, 0)]);
        assert_eq!((summary.mirror_first_player_ok, summary.mirrors), (1, 3));
        // Nothing is said of the first player of a cross pair.
        assert_eq!(
            summary_of(vec![counted(Kind::Cross, 100, 50, 100, 0)]).mirrors,
            0
        );
    }

    #[test]
    fn worst_pair_ignores_mirrors_and_the_first_wins_a_tie() {
        let mut mirror_far = mirror(100, 50);
        mirror_far.wins_a = 100;
        mirror_far.wins_b = 0;
        mirror_far.seat_one_wins = 100;
        let summary = summary_of(vec![
            mirror_far,
            cross(100, 60),
            cross(200, 120),
            cross(100, 40),
        ]);
        let worst = summary.worst_pair.expect("a worst pair");
        // 60/100, 120/200 and 40/100 are all 10 points away: the first wins.
        assert_eq!((worst.wins_a, worst.finished), (60, 100));
        assert!((worst.points() - 10.0).abs() < 1e-9);
        // A larger deviation on a larger sample wins by cross product.
        let summary = summary_of(vec![cross(100, 60), cross(1000, 700), cross(0, 0)]);
        let worst = summary.worst_pair.expect("a worst pair");
        assert_eq!((worst.wins_a, worst.finished), (700, 1000));
        assert_eq!(worst.matchup, "A vs B");
    }

    #[test]
    fn worst_pair_and_dispersion_are_na_without_a_cross_pair() {
        for matchups in [vec![], vec![mirror(100, 50)], vec![cross(0, 0)]] {
            let summary = summary_of(matchups.clone());
            assert_eq!((summary.worst_pair, summary.dispersion), (None, None));
            let text = report_of(vec![policy("p", matchups)]).render();
            assert!(
                text.contains("\nworst pair: n/a (no cross pair with finished games)\n"),
                "{text}"
            );
            assert!(
                text.contains("\ndispersion (deviation from 50%, 0 cross pairs): n/a\n"),
                "{text}"
            );
        }
    }

    #[test]
    fn dispersion_does_not_depend_on_the_deck_order() {
        // Swapping A and B turns 60 wins of A into 40: the deviation is the same.
        let one = summary_of(vec![cross(100, 60), cross(100, 52)]);
        let other = summary_of(vec![cross(100, 40), cross(100, 48)]);
        assert_eq!(one.dispersion, other.dispersion);
        let dispersion = one.dispersion.expect("a dispersion");
        assert_eq!(dispersion.pairs, 2);
        assert!((dispersion.min - 2.0).abs() < 1e-9);
        assert!((dispersion.mean - 6.0).abs() < 1e-9);
        assert!((dispersion.max - 10.0).abs() < 1e-9);
        assert_eq!(
            Some(dispersion.max),
            one.worst_pair.as_ref().map(PairDeviation::points)
        );
    }

    #[test]
    fn median_and_p90_by_nearest_rank() {
        let hist = |pairs: &[(u32, u64)]| {
            let finished: u64 = pairs.iter().map(|&(_, n)| n).sum();
            let mut m = counted(Kind::Cross, finished, 0, 0, 0);
            m.durations = pairs.iter().copied().collect();
            m.total_actions = pairs.iter().map(|&(a, n)| u64::from(a) * n).sum();
            (m.median(finished), m.p90(finished))
        };
        assert_eq!(hist(&[]), (None, None));
        assert_eq!(hist(&[(7, 1)]), (Some(7), Some(7)));
        assert_eq!(hist(&[(5, 1), (9, 1)]), (Some(5), Some(9)));
        assert_eq!(hist(&[(1, 1), (2, 1), (3, 1)]), (Some(2), Some(3)));
        let ten: Vec<(u32, u64)> = (1..=10).map(|a| (a, 1)).collect();
        assert_eq!(hist(&ten), (Some(5), Some(9)));
        let eleven: Vec<(u32, u64)> = (1..=11).map(|a| (a, 1)).collect();
        assert_eq!(hist(&eleven), (Some(6), Some(10)));
        // Several games per key: ranks 6 and 10 of 11 games.
        assert_eq!(hist(&[(10, 5), (20, 3), (30, 3)]), (Some(20), Some(30)));
    }

    #[test]
    fn the_target_marker_compares_integers_exactly() {
        let marker = |total| {
            let mut m = counted(Kind::Cross, 10, 5, 0, 0);
            m.total_actions = total;
            m.target_marker()
        };
        assert_eq!(marker(399), "below");
        assert_eq!(marker(400), "within");
        assert_eq!(marker(600), "within");
        assert_eq!(marker(601), "above");
        assert_eq!(cross(0, 0).target_marker(), "n/a");
    }

    #[test]
    fn an_interrupted_game_gives_a_warning_line_and_stays_in_the_column() {
        let text = sample().render();
        assert!(
            text.contains(
                "\nWARNING interrupted: Zèbre (mirror) 4/100 (rates and durations exclude these games)\n"
            ),
            "{text}"
        );
        assert!(
            text.contains(
                "\nWARNING interrupted: Aigle (mirror) 20/20 (rates and durations exclude these games)\n"
            ),
            "{text}"
        );
        // Matchups without interrupted games have none.
        assert_eq!(text.matches("WARNING").count(), 2);
        let line = text
            .lines()
            .find(|l| l.starts_with("Zèbre (mirror)"))
            .expect("a table line");
        assert!(line.ends_with("  4"), "{line}");
    }

    #[test]
    fn consistent_report_passes_check() {
        assert_eq!(sample().check(), Ok(()));
    }

    #[test]
    fn finished_games_must_equal_wins() {
        let mut report = sample();
        report.policies[0].matchups[0].wins_a = 59;
        let error = report.check().unwrap_err();
        assert!(error.contains("Ruée"), "{error}");
    }

    #[test]
    fn finished_games_must_equal_causes() {
        let mut report = sample();
        report.policies[0].matchups[2].exhaustion = 15;
        let error = report.check().unwrap_err();
        assert!(error.contains("Zèbre"), "{error}");
    }

    #[test]
    fn interrupted_cannot_exceed_games() {
        let mut report = sample();
        report.policies[0].matchups[4].interrupted = 21;
        assert!(report.check().is_err());
    }

    #[test]
    fn histogram_must_match_finished_games() {
        let mut report = sample();
        *report.policies[1].matchups[0]
            .durations
            .get_mut(&40)
            .expect("a key") = 99;
        let error = report.check().unwrap_err();
        assert!(
            error.contains("Ruée vs Rempart") && error.contains("histogram"),
            "{error}"
        );
    }

    #[test]
    fn histogram_must_match_total_actions() {
        let mut report = sample();
        report.policies[0].matchups[3].total_actions += 1;
        let error = report.check().unwrap_err();
        assert!(
            error.contains("Gnou") && error.contains("total actions"),
            "{error}"
        );
    }

    #[test]
    fn seat_one_wins_cannot_exceed_finished() {
        let mut report = sample();
        report.policies[0].matchups[0].seat_one_wins = 101;
        let error = report.check().unwrap_err();
        assert!(
            error.contains("Ruée vs Rempart") && error.contains("seat One"),
            "{error}"
        );
    }

    #[test]
    fn in_a_mirror_seat_one_wins_are_wins_a() {
        let mut report = sample();
        report.policies[0].matchups[2].seat_one_wins = 51;
        let error = report.check().unwrap_err();
        assert!(error.contains("Zèbre"), "{error}");
    }

    #[test]
    fn first_player_wins_cannot_exceed_finished() {
        let mut report = sample();
        report.policies[0].matchups[2].first_player_wins = 97;
        let error = report.check().unwrap_err();
        assert!(
            error.contains("Zèbre") && error.contains("first player"),
            "{error}"
        );
    }

    #[test]
    fn policy_names_must_be_unique_non_empty_and_simple() {
        let named = |names: &[&str]| {
            report_of(names.iter().map(|name| policy(name, vec![])).collect()).check()
        };
        assert_eq!(named(&["random"]), Ok(()));
        assert_eq!(named(&["always-first", "greedy_2", "r0"]), Ok(()));
        for bad in ["", "Random", "a b", "a,b", "a|b", "é", "a\nb", "a:b"] {
            assert!(named(&[bad]).is_err(), "{bad:?}");
        }
        assert!(named(&["random", "random"]).is_err());
        assert!(named(&[]).is_err(), "at least one policy");
    }
}
