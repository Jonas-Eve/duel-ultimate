//! Golden of `Report::render()`: the exact text of whole batches. Any change to the game, the
//! seeds or the policy's consumption of its generator changes the counters, and the text.
//!
//! The games were recorded on the code that predates the `Policy`-on-`Observation` migration
//! (#140) and have not changed since: `counters_unchanged` pins the integer counters of the four
//! batches (captured before #123 changed the format) and `game_seed_values_are_pinned` the seeds.
//! The text was regenerated twice, on purpose, and only its format moved: the header line by
//! #148 (the `rules:` segment), then the whole layout by #123 / #153 (a `policies:` segment, a
//! block per policy, two tables, the verdict lines). The rates, the means and the end causes of
//! the old table are those of the new tables, figure for figure. The rules are
//! `RuleSet::legacy()`, explicitly.

use duel_engine::RuleSet;
use duel_simulator::{
    BatchConfig, DEFAULT_MAX_ACTIONS, NamedPolicy, Report, contenders_from, game_seed,
    random_policy, run_batch,
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
    let policies = [NamedPolicy {
        name: "random",
        policy: &random_policy,
    }];
    run_batch(&contenders, &config, &policies)
}

fn rendered(set: &str, version: &str, games: u64, seed: u64) -> String {
    report(set, version, games, seed).render()
}

#[test]
fn golden_starter_1_1_0_1000_games_seed_0() {
    let expected = "\
set: starter@1.1.0 | seed: 0 | games per matchup: 1000 | action cap: 10000 | policies: random | rules: hero_riposte=off board_limit=none
policy: random
matchup           games             wins A          wins B           seat One       first player  interrupted
----------------  -----  -----------------  --------------  -----------------  -----------------  -----------
rush vs bulwark    1000  98.1% [97.1-98.8]  1.9% [1.2-2.9]  49.1% [46.0-52.2]  98.1% [97.1-98.8]            0
rush (mirror)      1000                  -               -  50.4% [47.3-53.5]  58.5% [55.4-61.5]            0
bulwark (mirror)   1000                  -               -  48.5% [45.4-51.6]  59.2% [56.1-62.2]            0

matchup           mean actions  median  p90  hero down  exhaustion  concede  vs target
----------------  ------------  ------  ---  ---------  ----------  -------  ---------
rush vs bulwark           42.1      40   64     100.0%        0.0%     0.0%     within
rush (mirror)             37.6      36   58     100.0%        0.0%     0.0%      below
bulwark (mirror)          86.1      83  126     100.0%        0.0%     0.0%      above

worst pair: rush vs bulwark 98.1% (deviation 48.1 pts from 50%)
dispersion (deviation from 50%, 1 cross pair): min 48.1 / mean 48.1 / max 48.1 pts
cross pairs in band 45-55%: 0/1 (all pairs: no)
mirror first player <= 57%: 0/2
duration target: 40-60 actions (provisional, see #124)
";
    assert_eq!(rendered("starter", "1.1.0", 1000, 0), expected);
}

#[test]
fn golden_starter_1_1_0_200_games_seed_7() {
    let expected = "\
set: starter@1.1.0 | seed: 7 | games per matchup: 200 | action cap: 10000 | policies: random | rules: hero_riposte=off board_limit=none
policy: random
matchup           games             wins A          wins B           seat One       first player  interrupted
----------------  -----  -----------------  --------------  -----------------  -----------------  -----------
rush vs bulwark     200  97.0% [93.6-98.6]  3.0% [1.4-6.4]  52.0% [45.1-58.8]  97.0% [93.6-98.6]            0
rush (mirror)       200                  -               -  56.0% [49.1-62.7]  61.0% [54.1-67.5]            0
bulwark (mirror)    200                  -               -  48.0% [41.2-54.9]  62.5% [55.6-68.9]            0

matchup           mean actions  median  p90  hero down  exhaustion  concede  vs target
----------------  ------------  ------  ---  ---------  ----------  -------  ---------
rush vs bulwark           40.7      39   63     100.0%        0.0%     0.0%     within
rush (mirror)             38.4      35   59     100.0%        0.0%     0.0%      below
bulwark (mirror)          85.7      81  127     100.0%        0.0%     0.0%      above

worst pair: rush vs bulwark 97.0% (deviation 47.0 pts from 50%)
dispersion (deviation from 50%, 1 cross pair): min 47.0 / mean 47.0 / max 47.0 pts
cross pairs in band 45-55%: 0/1 (all pairs: no)
mirror first player <= 57%: 0/2
duration target: 40-60 actions (provisional, see #124)
";
    assert_eq!(rendered("starter", "1.1.0", 200, 7), expected);
}

#[test]
fn golden_starter_1_0_0_120_games_seed_12345() {
    let expected = "\
set: starter@1.0.0 | seed: 12345 | games per matchup: 120 | action cap: 10000 | policies: random | rules: hero_riposte=off board_limit=none
policy: random
matchup           games             wins A          wins B           seat One       first player  interrupted
----------------  -----  -----------------  --------------  -----------------  -----------------  -----------
rush vs bulwark     120  97.5% [92.9-99.1]  2.5% [0.9-7.1]  49.2% [40.4-58.0]  97.5% [92.9-99.1]            0
rush (mirror)       120                  -               -  48.3% [39.6-57.2]  58.3% [49.4-66.8]            0
bulwark (mirror)    120                  -               -  53.3% [44.4-62.0]  62.5% [53.6-70.6]            0

matchup           mean actions  median  p90  hero down  exhaustion  concede  vs target
----------------  ------------  ------  ---  ---------  ----------  -------  ---------
rush vs bulwark           42.5      40   64     100.0%        0.0%     0.0%     within
rush (mirror)             35.2      34   56     100.0%        0.0%     0.0%      below
bulwark (mirror)          87.9      86  128     100.0%        0.0%     0.0%      above

worst pair: rush vs bulwark 97.5% (deviation 47.5 pts from 50%)
dispersion (deviation from 50%, 1 cross pair): min 47.5 / mean 47.5 / max 47.5 pts
cross pairs in band 45-55%: 0/1 (all pairs: no)
mirror first player <= 57%: 0/2
duration target: 40-60 actions (provisional, see #124)
";
    assert_eq!(rendered("starter", "1.0.0", 120, 12345), expected);
}

#[test]
fn golden_fixture_1_0_0_150_games_seed_3() {
    let expected = "\
set: fixture@1.0.0 | seed: 3 | games per matchup: 150 | action cap: 10000 | policies: random | rules: hero_riposte=off board_limit=none
policy: random
matchup           games            wins A             wins B           seat One       first player  interrupted
----------------  -----  ----------------  -----------------  -----------------  -----------------  -----------
aggro vs control    150  12.7% [8.3-18.9]  87.3% [81.1-91.7]  52.0% [44.1-59.8]   12.7% [8.3-18.9]            0
aggro (mirror)      150                 -                  -  50.7% [42.7-58.6]  54.7% [46.7-62.4]            0
control (mirror)    150                 -                  -  50.0% [42.1-57.9]  68.7% [60.9-75.5]            0

matchup           mean actions  median  p90  hero down  exhaustion  concede  vs target
----------------  ------------  ------  ---  ---------  ----------  -------  ---------
aggro vs control          32.1      31   51     100.0%        0.0%     0.0%      below
aggro (mirror)            40.1      38   64     100.0%        0.0%     0.0%     within
control (mirror)          40.5      39   63     100.0%        0.0%     0.0%     within

worst pair: aggro vs control 12.7% (deviation 37.3 pts from 50%)
dispersion (deviation from 50%, 1 cross pair): min 37.3 / mean 37.3 / max 37.3 pts
cross pairs in band 45-55%: 0/1 (all pairs: no)
mirror first player <= 57%: 1/2
duration target: 40-60 actions (provisional, see #124)
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
        let actual: Vec<Counters<'_>> = report.policies[0]
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
