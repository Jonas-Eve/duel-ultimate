//! Golden of `VersusReport::render()`: the exact text of a whole confrontation (#190). Any change
//! to the games, the seeds, the seat crossing or the counters changes the text.

use duel_ai::policy_named;
use duel_engine::RuleSet;
use duel_simulator::{
    BatchConfig, DEFAULT_MAX_ACTIONS, NamedPolicy, contenders_from, random_policy, run_versus,
};

const GOLDEN: &str = r#"versus | set: fixture@1.0.0 | seed: 7 | games per matchup: 40 | action cap: 10000 | candidate: random | reference: greedy | rules: hero_riposte=on board_limit=5
matchup           games     candidate wins  reference wins  interrupted  seat One  deck A
----------------  -----  -----------------  --------------  -----------  --------  ------
aggro vs control     40   12.5% [5.5-26.1]              35            0        17      25
aggro (mirror)       40     0.0% [0.0-8.8]              40            0        20      20
control (mirror)     40  72.5% [57.2-83.9]              11            0        23      23
total               120  28.3% [21.0-37.0]              86            0        60      68

matchup           mean actions  cand attacks       cand on hero  ref attacks        ref on hero  hand-overs c/r  wins as One c  wins as One r
----------------  ------------  ------------  -----------------  -----------  -----------------  --------------  -------------  -------------
aggro vs control         105.1           338  32.0% [27.2-37.1]         1228   10.1% [8.5-11.9]       1290/1293              1             16
aggro (mirror)            66.6           135  29.6% [22.6-37.8]          935  17.4% [15.1-20.0]         898/885              0             20
control (mirror)          64.0           302  67.9% [62.4-72.9]          349  20.9% [17.0-25.5]         849/860             16              7
total                     78.6           775  45.5% [42.1-49.1]         2512  14.3% [13.0-15.8]       3037/3038             17             43
"#;

#[test]
fn versus_report_golden_fixture() {
    let contenders = contenders_from(
        include_str!("../../../data/fixture/1.0.0/cards.json"),
        include_str!("../../../data/fixture/1.0.0/decks.json"),
    )
    .expect("the fixture is valid");
    let config = BatchConfig {
        games: 40,
        seed: 7,
        max_actions: DEFAULT_MAX_ACTIONS,
        data_versions: vec!["fixture@1.0.0".to_owned()],
        rules: RuleSet {
            hero_riposte: true,
            board_limit: Some(5),
        },
    };
    let greedy = policy_named("greedy").expect("greedy is registered");
    let report = run_versus(
        &contenders,
        &config,
        NamedPolicy {
            name: "random",
            policy: &random_policy,
        },
        NamedPolicy {
            name: "greedy",
            policy: &greedy,
        },
    )
    .expect("40 is a multiple of 4");
    assert_eq!(report.check(), Ok(()));
    assert_eq!(report.render(), GOLDEN);
}
