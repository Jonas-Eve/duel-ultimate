//! Golden of `Report::render()`: the exact text of whole batches, recorded on the code that
//! predates the `Policy`-on-`Observation` migration (#140) and never regenerated since. Any
//! change to the game, the seeds or the policy's consumption of its generator changes it.

use duel_simulator::{BatchConfig, DEFAULT_MAX_ACTIONS, contenders_from, random_policy, run_batch};

fn rendered(set: &str, version: &str, games: u64, seed: u64) -> String {
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
    };
    run_batch(&contenders, &config, &random_policy).render()
}

#[test]
fn golden_starter_1_1_0_1000_games_seed_0() {
    let expected = "\
set: starter@1.1.0 | seed: 0 | games per matchup: 1000 | action cap: 10000
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
set: starter@1.1.0 | seed: 7 | games per matchup: 200 | action cap: 10000
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
set: starter@1.0.0 | seed: 12345 | games per matchup: 120 | action cap: 10000
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
set: fixture@1.0.0 | seed: 3 | games per matchup: 150 | action cap: 10000
matchup           games            wins A             wins B       first player  mean actions  hero down  exhaustion  concede  interrupted
----------------  -----  ----------------  -----------------  -----------------  ------------  ---------  ----------  -------  -----------
aggro vs control    150  12.7% [8.3-18.9]  87.3% [81.1-91.7]   12.7% [8.3-18.9]          32.1        150           0        0            0
aggro (mirror)      150                 -                  -  54.7% [46.7-62.4]          40.1        150           0        0            0
control (mirror)    150                 -                  -  68.7% [60.9-75.5]          40.5        150           0        0            0
";
    assert_eq!(rendered("fixture", "1.0.0", 150, 3), expected);
}
