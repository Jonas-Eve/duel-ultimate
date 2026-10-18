//! Golden of `Report::render()` under `RuleSet::standard()` (written out) with the `random` and
//! the `greedy` policies, pinned **before** story #128 refactored the `Action` contract.
//!
//! The older goldens (`report_golden.rs`) are all `legacy` and `random`: none exercises
//! `Observation::outcome` and `dry_run`, which `greedy_policy` reads at every step. This text is
//! the report proof that the behaviour of #127 is unchanged for the whole story: it never changes.
//!
//! 120 games per matchup keep the test under 30 s in debug.

use duel_ai::greedy_policy;
use duel_engine::RuleSet;
use duel_simulator::{
    BatchConfig, DEFAULT_MAX_ACTIONS, NamedPolicy, contenders_from, random_policy, run_batch,
};

#[test]
fn golden_starter_1_1_0_standard_rules_random_and_greedy() {
    let contenders = contenders_from(
        include_str!("../../../data/starter/1.1.0/cards.json"),
        include_str!("../../../data/starter/1.1.0/decks.json"),
    )
    .expect("the embedded data is valid");
    let config = BatchConfig {
        games: 120,
        seed: 5,
        max_actions: DEFAULT_MAX_ACTIONS,
        data_versions: vec!["starter@1.1.0".to_string()],
        rules: RuleSet::standard(),
    };
    let policies = [
        NamedPolicy {
            name: "random",
            policy: &random_policy,
        },
        NamedPolicy {
            name: "greedy",
            policy: &greedy_policy,
        },
    ];
    let rendered = run_batch(&contenders, &config, &policies).render();
    let expected = "\
set: starter@1.1.0 | seed: 5 | games per matchup: 120 | action cap: 10000 | policies: random, greedy | rules: hero_riposte=on board_limit=5
policy: random
matchup           games             wins A          wins B           seat One       first player  interrupted
----------------  -----  -----------------  --------------  -----------------  -----------------  -----------
rush vs bulwark     120  99.2% [95.4-99.9]  0.8% [0.1-4.6]  50.8% [42.0-59.6]  99.2% [95.4-99.9]            0
rush (mirror)       120                  -               -  49.2% [40.4-58.0]  53.3% [44.4-62.0]            0
bulwark (mirror)    120                  -               -  52.5% [43.6-61.2]  46.7% [38.0-55.6]            0

matchup           mean actions  median  p90  hero down  exhaustion  concede  vs target
----------------  ------------  ------  ---  ---------  ----------  -------  ---------
rush vs bulwark           47.5      47   65     100.0%        0.0%     0.0%     within
rush (mirror)             41.8      40   63     100.0%        0.0%     0.0%     within
bulwark (mirror)          87.6      87  119     100.0%        0.0%     0.0%      above

worst pair: rush vs bulwark 99.2% (deviation 49.2 pts from 50%)
dispersion (deviation from 50%, 1 cross pair): min 49.2 / mean 49.2 / max 49.2 pts
cross pairs in band 45-55%: 0/1 (all pairs: no)
mirror first player <= 57%: 2/2
duration target: 40-60 actions (provisional, see #124)

policy: greedy
matchup           games             wins A             wins B           seat One       first player  interrupted
----------------  -----  -----------------  -----------------  -----------------  -----------------  -----------
rush vs bulwark     120  29.2% [21.8-37.8]  70.8% [62.2-78.2]  50.8% [42.0-59.6]  29.2% [21.8-37.8]            0
rush (mirror)       120                  -                  -  45.8% [37.2-54.7]  88.3% [81.4-92.9]            0
bulwark (mirror)    120                  -                  -  41.7% [33.2-50.6]  87.5% [80.4-92.3]            0

matchup           mean actions  median  p90  hero down  exhaustion  concede  vs target
----------------  ------------  ------  ---  ---------  ----------  -------  ---------
rush vs bulwark           90.4      88  104      29.2%       70.8%     0.0%      above
rush (mirror)            103.8     102  113      97.5%        2.5%     0.0%      above
bulwark (mirror)         267.8     276  313       0.0%      100.0%     0.0%      above

ALERT exhaustion-dominant: rush vs bulwark 70.8% > 70%
ALERT exhaustion-dominant: bulwark (mirror) 100.0% > 70%
worst pair: rush vs bulwark 29.2% (deviation 20.8 pts from 50%)
dispersion (deviation from 50%, 1 cross pair): min 20.8 / mean 20.8 / max 20.8 pts
cross pairs in band 45-55%: 0/1 (all pairs: no)
mirror first player <= 57%: 0/2
duration target: 40-60 actions (provisional, see #124)
";
    assert_eq!(rendered, expected);
}
