//! Golden of `VersusReport::render()`: the exact text of a whole confrontation (#190). Any change
//! to the games, the seeds, the seat crossing or the counters changes the text.

use duel_ai::policy_named;
use duel_engine::RuleSet;
use duel_simulator::{
    BatchConfig, DEFAULT_MAX_ACTIONS, NamedPolicy, contenders_from, random_policy, run_versus,
};

const GOLDEN: &str = "";

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
