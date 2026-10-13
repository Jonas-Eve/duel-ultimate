//! The balance report: integer counters per matchup and its deterministic text rendering.

/// Kind of matchup.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Kind {
    /// Two different decks: `wins_a` / `wins_b` count the wins of deck A / deck B.
    Cross,
    /// A deck against itself: `wins_a` / `wins_b` count the wins of seat One / seat Two.
    Mirror,
}

/// Counters of one matchup. Invariants (see [`Report::check`]): finished games
/// `games - interrupted` = `wins_a + wins_b` = `hero_down + exhaustion + concede`.
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
    pub first_player_wins: u64,
    pub hero_down: u64,
    pub exhaustion: u64,
    pub concede: u64,
    /// Actions summed over the finished games.
    pub total_actions: u64,
}

/// A whole run: the set, the settings and one entry per matchup, in rendering order.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Report {
    /// `name@version`.
    pub set: String,
    pub seed: u64,
    pub games_per_matchup: u64,
    pub max_actions: u32,
    pub matchups: Vec<MatchupReport>,
}

impl Report {
    /// Deterministic plain-text rendering, every line ending with `\n`.
    pub fn render(&self) -> String {
        todo!()
    }

    /// Checks the counter invariants of every matchup.
    pub fn check(&self) -> Result<(), String> {
        todo!()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[allow(clippy::too_many_arguments)]
    fn matchup(
        a: &str,
        b: &str,
        kind: Kind,
        games: u64,
        interrupted: u64,
        wins: (u64, u64),
        first: u64,
        causes: (u64, u64, u64),
        total_actions: u64,
    ) -> MatchupReport {
        MatchupReport {
            deck_a: a.into(),
            deck_b: b.into(),
            kind,
            games,
            interrupted,
            wins_a: wins.0,
            wins_b: wins.1,
            first_player_wins: first,
            hero_down: causes.0,
            exhaustion: causes.1,
            concede: causes.2,
            total_actions,
        }
    }

    /// Deck names deliberately not in sorted order.
    fn sample() -> Report {
        Report {
            set: "fixture@1.0.0".into(),
            seed: 7,
            games_per_matchup: 100,
            max_actions: 10_000,
            matchups: vec![
                matchup(
                    "Ruée",
                    "Rempart",
                    Kind::Cross,
                    100,
                    0,
                    (60, 40),
                    55,
                    (70, 20, 10),
                    4000,
                ),
                matchup(
                    "Zèbre",
                    "Zèbre",
                    Kind::Mirror,
                    100,
                    4,
                    (50, 46),
                    52,
                    (80, 16, 0),
                    3910,
                ),
                matchup(
                    "Aigle",
                    "Aigle",
                    Kind::Mirror,
                    20,
                    20,
                    (0, 0),
                    0,
                    (0, 0, 0),
                    0,
                ),
            ],
        }
    }

    const GOLDEN: &str = "\
set: fixture@1.0.0 | seed: 7 | games per matchup: 100 | action cap: 10000
matchup          games             wins A             wins B       first player  mean actions  hero down  exhaustion  concede  interrupted
---------------  -----  -----------------  -----------------  -----------------  ------------  ---------  ----------  -------  -----------
Ruée vs Rempart    100  60.0% [50.2-69.1]  40.0% [30.9-49.8]  55.0% [45.2-64.4]          40.0         70          20       10            0
Zèbre (mirror)     100                  -                  -  54.2% [44.2-63.8]          40.7         80          16        0            4
Aigle (mirror)      20                  -                  -                n/a           n/a          0           0        0           20
";

    #[test]
    fn golden_render() {
        assert_eq!(sample().render(), GOLDEN);
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
    fn empty_report_renders_header_only_without_panicking() {
        let mut report = sample();
        report.matchups.clear();
        let text = report.render();
        assert!(text.starts_with("set: fixture@1.0.0"));
        assert!(text.ends_with('\n'));
    }

    #[test]
    fn zero_games_matchup_renders_without_panicking() {
        let mut report = sample();
        report.matchups = vec![matchup(
            "A",
            "B",
            Kind::Cross,
            0,
            0,
            (0, 0),
            0,
            (0, 0, 0),
            0,
        )];
        assert!(report.render().contains("n/a"));
        assert_eq!(report.check(), Ok(()));
    }

    #[test]
    fn consistent_report_passes_check() {
        assert_eq!(sample().check(), Ok(()));
    }

    #[test]
    fn finished_games_must_equal_wins() {
        let mut report = sample();
        report.matchups[0].wins_a = 59;
        let error = report.check().unwrap_err();
        assert!(error.contains("Ruée"), "{error}");
    }

    #[test]
    fn finished_games_must_equal_causes() {
        let mut report = sample();
        report.matchups[1].exhaustion = 15;
        let error = report.check().unwrap_err();
        assert!(error.contains("Zèbre"), "{error}");
    }

    #[test]
    fn interrupted_cannot_exceed_games() {
        let mut report = sample();
        report.matchups[2].interrupted = 21;
        assert!(report.check().is_err());
    }
}
