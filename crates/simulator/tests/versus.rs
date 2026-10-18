//! `run_versus`: two policies face each other on the same engine seeds, with the seats of the
//! decks and of the policies crossed every four games, and the counters of a confrontation (#190).

use std::collections::BTreeMap;

use duel_ai::policy_named;
use duel_engine::{CardInstanceId, Event, PlayerId, RuleSet, WinReason};
use duel_simulator::{
    BatchConfig, Contender, DEFAULT_MAX_ACTIONS, GameRecord, NamedPolicy, Policy, StepCounts,
    VersusGame, VersusReport, contenders_from, random_policy, run_batch_observed, run_versus,
    run_versus_observed, step_counts,
};

const FIXTURE_CARDS: &str = include_str!("../../../data/fixture/1.0.0/cards.json");
const FIXTURE_DECKS: &str = include_str!("../../../data/fixture/1.0.0/decks.json");

fn fixture() -> Vec<Contender> {
    contenders_from(FIXTURE_CARDS, FIXTURE_DECKS).expect("the fixture is valid")
}

fn config(games: u64, seed: u64) -> BatchConfig {
    BatchConfig {
        games,
        seed,
        max_actions: DEFAULT_MAX_ACTIONS,
        data_versions: vec!["fixture@1.0.0".to_owned()],
        rules: RuleSet::default(),
    }
}

fn named<'a>(name: &'a str, policy: &'a dyn Policy) -> NamedPolicy<'a> {
    NamedPolicy { name, policy }
}

fn versus(games: u64, seed: u64) -> VersusReport {
    let greedy = policy_named("greedy").expect("greedy is registered");
    run_versus(
        &fixture(),
        &config(games, seed),
        named("random", &random_policy),
        named("greedy", &greedy),
    )
    .expect("a multiple of 4")
}

#[test]
fn versus_seat_pattern_crosses_deck_and_policy_seats() {
    let greedy = policy_named("greedy").expect("greedy is registered");
    let mut games: Vec<VersusGame> = Vec::new();
    run_versus_observed(
        &fixture(),
        &config(8, 3),
        named("random", &random_policy),
        named("greedy", &greedy),
        |game| games.push(*game),
    )
    .expect("a multiple of 4");
    // The fixture has two decks: matchup 0 is the cross pair, matchups 1 and 2 are the mirrors.
    let mut combos: BTreeMap<(usize, bool, bool), u32> = BTreeMap::new();
    for game in &games {
        let a_is_one = game.record.seat_of_a == PlayerId::One;
        let candidate_is_one = game.candidate_seat == PlayerId::One;
        *combos
            .entry((game.record.matchup, a_is_one, candidate_is_one))
            .or_default() += 1;
    }
    for (a_is_one, candidate_is_one) in [(true, true), (true, false), (false, true), (false, false)]
    {
        assert_eq!(combos.get(&(0, a_is_one, candidate_is_one)), Some(&2));
    }
    for mirror in [1, 2] {
        assert_eq!(combos.get(&(mirror, true, true)), Some(&4));
        assert_eq!(combos.get(&(mirror, true, false)), Some(&4));
        assert_eq!(combos.get(&(mirror, false, true)), None);
        assert_eq!(combos.get(&(mirror, false, false)), None);
    }
    assert_eq!(games.len(), 24);
}

#[test]
fn versus_refuses_a_number_of_games_not_a_multiple_of_4() {
    let greedy = policy_named("greedy").expect("greedy is registered");
    for games in [1, 2, 3, 6, 10] {
        let error = run_versus(
            &fixture(),
            &config(games, 0),
            named("random", &random_policy),
            named("greedy", &greedy),
        )
        .expect_err("not a multiple of 4");
        assert!(error.contains("multiple of 4"), "{error}");
    }
}

#[test]
fn versus_is_deterministic() {
    let first = versus(16, 5);
    let second = versus(16, 5);
    assert_eq!(first, second);
    assert_eq!(first.render(), second.render());
    assert_ne!(first, versus(16, 6));
}

#[test]
fn versus_with_one_policy_on_both_sides_replays_run_batch() {
    let greedy = policy_named("greedy").expect("greedy is registered");
    let contenders = fixture();
    let cfg = config(12, 7);
    let mut batch_records: Vec<GameRecord> = Vec::new();
    let batch = run_batch_observed(&contenders, &cfg, &[named("greedy", &greedy)], |record| {
        batch_records.push(*record);
    });
    let mut versus_records: Vec<GameRecord> = Vec::new();
    let report = run_versus_observed(
        &contenders,
        &cfg,
        named("greedy", &greedy),
        named("greedy", &greedy),
        |game| versus_records.push(game.record),
    )
    .expect("a multiple of 4");
    assert_eq!(batch_records, versus_records);
    for (matchup, tally) in batch.policies[0].matchups.iter().zip(&report.matchups) {
        let tally = &tally.tally;
        assert_eq!(tally.games, matchup.games);
        assert_eq!(tally.interrupted, matchup.interrupted);
        assert_eq!(tally.deck_a_wins, matchup.wins_a);
        assert_eq!(tally.seat_one_wins, matchup.seat_one_wins);
        assert_eq!(tally.total_actions, matchup.total_actions);
        assert_eq!(tally.durations, matchup.durations);
        // The report counts the wins of seat One per policy too.
        assert_eq!(
            tally.candidate.wins_as_one + tally.reference.wins_as_one,
            matchup.seat_one_wins
        );
    }
}

fn attacked(player: PlayerId) -> Event {
    Event::Attacked {
        player,
        attacker: CardInstanceId(1),
        cost: 2,
    }
}

#[test]
fn hero_attack_share_counts_damaged_after_attacked() {
    // An attack on the hero: `Attacked` then `Damaged` in the events of the same action.
    let on_hero = [
        attacked(PlayerId::One),
        Event::Damaged {
            player: PlayerId::Two,
            amount: 3,
        },
    ];
    assert_eq!(
        step_counts(&on_hero),
        StepCounts {
            attack_by: Some(PlayerId::One),
            hero_hit: true,
            hand_over_by: None,
        }
    );
    // An attack on a creature: no `Damaged`.
    let on_creature = [
        attacked(PlayerId::Two),
        Event::CreatureDamaged {
            player: PlayerId::One,
            creature: CardInstanceId(9),
            amount: 2,
        },
        Event::ActivePlayerChanged {
            player: PlayerId::One,
        },
    ];
    assert_eq!(
        step_counts(&on_creature),
        StepCounts {
            attack_by: Some(PlayerId::Two),
            hero_hit: false,
            hand_over_by: Some(PlayerId::Two),
        }
    );
    // Not an attack: nothing, and `GameWon` is no hand-over.
    let won = [Event::GameWon {
        winner: PlayerId::One,
        reason: WinReason::HeroDown,
    }];
    assert_eq!(step_counts(&won), StepCounts::default());
}

#[test]
fn versus_counters_satisfy_their_invariants() {
    let report = versus(40, 11);
    assert_eq!(report.check(), Ok(()));
    let mut attacks = 0;
    for matchup in &report.matchups {
        let t = &matchup.tally;
        assert_eq!(t.games, 40);
        assert_eq!(t.candidate.wins + t.reference.wins + t.interrupted, t.games);
        for side in [&t.candidate, &t.reference] {
            assert!(side.hero_attacks <= side.attacks);
            assert!(side.wins_as_one <= side.wins);
            attacks += side.attacks;
        }
    }
    assert!(attacks > 0, "the policies attack");
    let total = report.total();
    assert_eq!(total.games, 120);
    assert_eq!(
        total.candidate.wins + total.reference.wins + total.interrupted,
        120
    );
}
