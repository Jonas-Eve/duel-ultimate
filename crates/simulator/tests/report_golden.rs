//! Golden of `Report::render()`: the exact text of whole batches, recorded on the code that
//! predates the `Policy`-on-`Observation` migration (#140) and never regenerated since. Any
//! change to the game, the seeds or the policy's consumption of its generator changes it.
//! Only the header line was regenerated, once, by #148 (the `rules:` segment): the matchup lines
//! are byte for byte those recorded before. The rules are `RuleSet::legacy()`, explicitly.

use duel_engine::RuleSet;
use duel_simulator::{
    BatchConfig, DEFAULT_MAX_ACTIONS, Report, contenders_from, game_seed, random_policy, run_batch,
};

fn report(set: &str, version: &str, games: u64, seed: u64) -> Report {
    let (cards, decks) = match (set, version) {
        ("fixture", "1.0.0") => (
            include_str!("../../../data/fixture/1.0.0/cards.json"),
            include_str!("../../../data/fixture/1.0.0/decks.json"),
        ),
        ("starter", "1.0.0") => (
            include_str!("../../../data/starter/1.0.0/cards.json"),
            include_str!("../../../data/starter/1.0.0/decks.json"),
        ),
        ("starter", "1.1.0") => (
            include_str!("../../../data/starter/1.1.0/cards.json"),
            include_str!("../../../data/starter/1.1.0/decks.json"),
        ),
        _ => panic!("unknown set {set}@{version}"),
    };
    let contenders = contenders_from(cards, decks).expect("the embedded data is valid");
    let config = BatchConfig {
        games,
        seed,
        max_actions: DEFAULT_MAX_ACTIONS,
        data_versions: vec![format!("{set}@{version}")],
        // The reference of the behaviour before the options: valid whatever the default becomes.
        rules: RuleSet::legacy(),
    };
    run_batch(&contenders, &config, &random_policy)
}

fn rendered(set: &str, version: &str, games: u64, seed: u64) -> String {
    report(set, version, games, seed).render()
}

#[test]
fn golden_starter_1_1_0_1000_games_seed_0() {
    let expected = "\
set: starter@1.1.0 | seed: 0 | games per matchup: 1000 | action cap: 10000 | rules: hero_riposte=off board_limit=none
matchup           games             wins A          wins B       first player  mean actions  hero down  exhaustion  concede  interrupted
----------------  -----  -----------------  --------------  -----------------  ------------  ---------  ----------  -------  -----------
rush vs bulwark    1000  98.1% [97.1-98.8]  1.9% [1.2-2.9]  98.1% [97.1-98.8]          42.1       1000           0        0            0
rush (mirror)      1000                  -               -  58.5% [55.4-61.5]          37.6       1000           0        0            0
bulwark (mirror)   1000                  -               -  59.2% [56.1-62.2]          86.1       1000           0        0            0
";
    assert_eq!(rendered("starter", "1.1.0", 1000, 0), expected);
}

#[test]
fn golden_starter_1_1_0_200_games_seed_7() {
    let expected = "\
set: starter@1.1.0 | seed: 7 | games per matchup: 200 | action cap: 10000 | rules: hero_riposte=off board_limit=none
matchup           games             wins A          wins B       first player  mean actions  hero down  exhaustion  concede  interrupted
----------------  -----  -----------------  --------------  -----------------  ------------  ---------  ----------  -------  -----------
rush vs bulwark     200  97.0% [93.6-98.6]  3.0% [1.4-6.4]  97.0% [93.6-98.6]          40.7        200           0        0            0
rush (mirror)       200                  -               -  61.0% [54.1-67.5]          38.4        200           0        0            0
bulwark (mirror)    200                  -               -  62.5% [55.6-68.9]          85.7        200           0        0            0
";
    assert_eq!(rendered("starter", "1.1.0", 200, 7), expected);
}

#[test]
fn golden_starter_1_0_0_120_games_seed_12345() {
    let expected = "\
set: starter@1.0.0 | seed: 12345 | games per matchup: 120 | action cap: 10000 | rules: hero_riposte=off board_limit=none
matchup           games             wins A          wins B       first player  mean actions  hero down  exhaustion  concede  interrupted
----------------  -----  -----------------  --------------  -----------------  ------------  ---------  ----------  -------  -----------
rush vs bulwark     120  97.5% [92.9-99.1]  2.5% [0.9-7.1]  97.5% [92.9-99.1]          42.5        120           0        0            0
rush (mirror)       120                  -               -  58.3% [49.4-66.8]          35.2        120           0        0            0
bulwark (mirror)    120                  -               -  62.5% [53.6-70.6]          87.9        120           0        0            0
";
    assert_eq!(rendered("starter", "1.0.0", 120, 12345), expected);
}

#[test]
fn golden_fixture_1_0_0_150_games_seed_3() {
    let expected = "\
set: fixture@1.0.0 | seed: 3 | games per matchup: 150 | action cap: 10000 | rules: hero_riposte=off board_limit=none
matchup           games            wins A             wins B       first player  mean actions  hero down  exhaustion  concede  interrupted
----------------  -----  ----------------  -----------------  -----------------  ------------  ---------  ----------  -------  -----------
aggro vs control    150  12.7% [8.3-18.9]  87.3% [81.1-91.7]   12.7% [8.3-18.9]          32.1        150           0        0            0
aggro (mirror)      150                 -                  -  54.7% [46.7-62.4]          40.1        150           0        0            0
control (mirror)    150                 -                  -  68.7% [60.9-75.5]          40.5        150           0        0            0
";
    assert_eq!(rendered("fixture", "1.0.0", 150, 3), expected);
}

/// Integer counters of one matchup, as recorded on the code that predates #153:
/// `(deck_a, wins_a, wins_b, first_player_wins, hero_down, exhaustion, concede, interrupted,
/// total_actions)`.
type Counters<'a> = (&'a str, u64, u64, u64, u64, u64, u64, u64, u64);

/// The counters of the four batches above, captured on the code BEFORE the report format of #123
/// changed and never touched since: only the rendering may differ, never a game.
#[test]
fn counters_unchanged() {
    let batches: [(&str, &str, u64, u64, [Counters<'static>; 3]); 4] = [
        (
            "starter",
            "1.1.0",
            1000,
            0,
            [
                ("rush", 981, 19, 981, 1000, 0, 0, 0, 42053),
                ("rush", 504, 496, 585, 1000, 0, 0, 0, 37589),
                ("bulwark", 485, 515, 592, 1000, 0, 0, 0, 86091),
            ],
        ),
        (
            "starter",
            "1.1.0",
            200,
            7,
            [
                ("rush", 194, 6, 194, 200, 0, 0, 0, 8131),
                ("rush", 112, 88, 122, 200, 0, 0, 0, 7687),
                ("bulwark", 96, 104, 125, 200, 0, 0, 0, 17146),
            ],
        ),
        (
            "starter",
            "1.0.0",
            120,
            12345,
            [
                ("rush", 117, 3, 117, 120, 0, 0, 0, 5104),
                ("rush", 58, 62, 70, 120, 0, 0, 0, 4230),
                ("bulwark", 64, 56, 75, 120, 0, 0, 0, 10551),
            ],
        ),
        (
            "fixture",
            "1.0.0",
            150,
            3,
            [
                ("aggro", 19, 131, 19, 150, 0, 0, 0, 4816),
                ("aggro", 76, 74, 82, 150, 0, 0, 0, 6009),
                ("control", 75, 75, 103, 150, 0, 0, 0, 6074),
            ],
        ),
    ];
    for (set, version, games, seed, expected) in batches {
        let report = report(set, version, games, seed);
        let actual: Vec<Counters<'_>> = report
            .matchups
            .iter()
            .map(|m| {
                (
                    m.deck_a.as_str(),
                    m.wins_a,
                    m.wins_b,
                    m.first_player_wins,
                    m.hero_down,
                    m.exhaustion,
                    m.concede,
                    m.interrupted,
                    m.total_actions,
                )
            })
            .collect();
        assert_eq!(actual, expected, "{set}@{version} {games}/{seed}");
    }
}

/// The engine seed of a game is part of the determinism contract: captured before #153.
#[test]
fn game_seed_values_are_pinned() {
    assert_eq!(game_seed(1, 0, 0), 8_841_707_400_507_832_957);
    assert_eq!(game_seed(7, 2, 13), 4_170_119_748_415_989_943);
    assert_eq!(game_seed(u64::MAX, 5, 999), 2_055_176_720_511_709_744);
}
