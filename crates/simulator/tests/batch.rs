//! `run_batch`: determinism, derived seeds, seat alternation, mirrors, the action cap,
//! termination, and tuning by data alone. No simulation rate is pinned anywhere in this file.

use std::cell::RefCell;

use duel_engine::{Action, GameSetup, Observation, PlayerId, WinReason};
use duel_simulator::{
    BatchConfig, Contender, DEFAULT_MAX_ACTIONS, GameRecord, Policy, Report, contenders_from,
    derive_seed, game_seed, play_game, policy_rng, random_policy, run_batch, run_batch_observed,
};
use rand::{Rng, SeedableRng};
use rand_chacha::ChaCha8Rng;

const FIXTURE_CARDS: &str = include_str!("../../../data/fixture/1.0.0/cards.json");
const FIXTURE_DECKS: &str = include_str!("../../../data/fixture/1.0.0/decks.json");
const STARTER_CARDS: &str = include_str!("../../../data/starter/1.0.0/cards.json");
const STARTER_DECKS: &str = include_str!("../../../data/starter/1.0.0/decks.json");

fn fixture() -> Vec<Contender> {
    contenders_from(FIXTURE_CARDS, FIXTURE_DECKS).expect("the fixture is valid")
}

fn starter() -> Vec<Contender> {
    contenders_from(STARTER_CARDS, STARTER_DECKS).expect("the starter set is valid")
}

fn config(games: u64, seed: u64) -> BatchConfig {
    BatchConfig {
        games,
        seed,
        max_actions: DEFAULT_MAX_ACTIONS,
        data_versions: vec!["test@0".to_owned()],
    }
}

/// Runs a batch and keeps every game record.
fn observed(
    contenders: &[Contender],
    config: &BatchConfig,
    policy: &dyn Policy,
) -> (Report, Vec<GameRecord>) {
    let mut records = Vec::new();
    let report = run_batch_observed(contenders, config, policy, |record| records.push(*record));
    (report, records)
}

fn of_matchup(records: &[GameRecord], matchup: usize) -> Vec<GameRecord> {
    records
        .iter()
        .filter(|record| record.matchup == matchup)
        .copied()
        .collect()
}

// 1. Same batch, same results.

#[test]
fn the_same_batch_gives_the_same_report_and_text() {
    for contenders in [fixture(), starter()] {
        let first = run_batch(&contenders, &config(40, 9), &random_policy);
        let second = run_batch(&contenders, &config(40, 9), &random_policy);
        assert_eq!(first, second);
        assert_eq!(first.render(), second.render());
    }
}

#[test]
fn another_batch_seed_gives_another_report() {
    for contenders in [fixture(), starter()] {
        let one = run_batch(&contenders, &config(60, 1), &random_policy);
        let two = run_batch(&contenders, &config(60, 2), &random_policy);
        assert_ne!(one.matchups, two.matchups);
        assert_ne!(one.render(), two.render());
    }
}

#[test]
fn the_report_records_the_settings_and_the_versions() {
    let report = run_batch(&starter(), &config(5, 77), &random_policy);
    assert_eq!(report.set, "test@0");
    assert_eq!(report.seed, 77);
    assert_eq!(report.games_per_matchup, 5);
    assert_eq!(report.max_actions, DEFAULT_MAX_ACTIONS);
    assert!(report.check().is_ok());
}

// 2. Derived seeds.

#[test]
fn derive_seed_matches_known_splitmix64_values() {
    // Computed independently (Python big integers) from the SplitMix64 finalizer; (0, 1) is
    // the well-known first output of SplitMix64 seeded with 0.
    assert_eq!(derive_seed(0, 0), 0x0);
    assert_eq!(derive_seed(0, 1), 0xE220_A839_7B1D_CDAF);
    assert_eq!(derive_seed(1, 0), 0x5692_161D_100B_05E5);
    assert_eq!(derive_seed(42, 7), 0x53AD_348A_F3DD_AF4B);
    assert_eq!(derive_seed(0xDEAD_BEEF, u64::MAX), 0x6D90_8474_B340_945D);
}

#[test]
fn derive_seed_depends_on_the_tag_and_on_the_parent() {
    assert_ne!(derive_seed(5, 0), derive_seed(5, 1));
    assert_ne!(derive_seed(5, 1), derive_seed(6, 1));
    assert_ne!(derive_seed(5, 1), 5);
}

#[test]
fn game_seeds_are_distinct_over_1000_indices_and_3_matchups() {
    let mut seen = std::collections::BTreeSet::new();
    for matchup in 0..3 {
        for index in 0..1000 {
            assert!(
                seen.insert(game_seed(0, matchup, index)),
                "matchup {matchup} game {index} repeats a seed"
            );
        }
    }
    assert_eq!(game_seed(3, 1, 4), derive_seed(derive_seed(3, 1), 4));
}

#[test]
fn the_policy_generators_differ_from_each_other_and_from_the_engine_stream() {
    let seed = game_seed(11, 0, 3);
    let draws = |mut rng: ChaCha8Rng| -> Vec<u64> { (0..10).map(|_| rng.next_u64()).collect() };
    let one = draws(policy_rng(seed, PlayerId::One));
    let two = draws(policy_rng(seed, PlayerId::Two));
    let engine = draws(ChaCha8Rng::seed_from_u64(seed));
    assert_ne!(one, two);
    assert_ne!(one, engine);
    assert_ne!(two, engine);
}

thread_local! {
    static DRAWS: RefCell<Vec<u64>> = const { RefCell::new(Vec::new()) };
}

fn recording_policy(_obs: &Observation<'_>, rng: &mut ChaCha8Rng) -> usize {
    DRAWS.with(|draws| draws.borrow_mut().push(rng.next_u64()));
    0
}

#[test]
fn a_game_feeds_the_policy_from_one_stream_per_seat_never_from_the_engine_stream() {
    let setup = fixture_setup();
    let seed = game_seed(5, 0, 2);
    DRAWS.with(|draws| draws.borrow_mut().clear());
    play_game(
        setup,
        seed,
        &recording_policy,
        DEFAULT_MAX_ACTIONS,
        |_, _| {},
    );
    let seen = DRAWS.with(|draws| draws.borrow().clone());
    assert!(seen.len() > 10, "the game lasted {} choices", seen.len());
    let stream =
        |mut rng: ChaCha8Rng| -> Vec<u64> { (0..seen.len()).map(|_| rng.next_u64()).collect() };
    let one = stream(policy_rng(seed, PlayerId::One));
    let two = stream(policy_rng(seed, PlayerId::Two));
    let engine = stream(ChaCha8Rng::seed_from_u64(seed));
    assert!(
        seen.iter()
            .all(|draw| one.contains(draw) || two.contains(draw)),
        "every draw comes from a seat's own stream"
    );
    assert!(seen.iter().any(|draw| one.contains(draw)), "seat One draws");
    assert!(seen.iter().any(|draw| two.contains(draw)), "seat Two draws");
    assert!(
        seen.iter().all(|draw| !engine.contains(draw)),
        "nothing comes from the engine's stream"
    );
    // Each seat consumes its own stream in order: its draws are a prefix of its stream.
    let from_one: Vec<u64> = seen.iter().copied().filter(|d| one.contains(d)).collect();
    let from_two: Vec<u64> = seen.iter().copied().filter(|d| two.contains(d)).collect();
    assert_eq!(from_one, one[..from_one.len()]);
    assert_eq!(from_two, two[..from_two.len()]);
}

fn fixture_setup() -> GameSetup {
    let contenders = fixture();
    GameSetup {
        one: contenders[0].player.clone(),
        two: contenders[1].player.clone(),
        data_versions: vec!["test@0".to_owned()],
    }
}

// 3. The policy signature and the Concede filter.

#[test]
fn a_policy_is_a_function_of_the_observation_and_its_generator() {
    let _: &dyn Policy = &random_policy;
    let _: &dyn Policy = &|_: &Observation<'_>, _: &mut ChaCha8Rng| 0;
}

fn always_first(_: &Observation<'_>, _: &mut ChaCha8Rng) -> usize {
    0
}

fn always_last(obs: &Observation<'_>, _: &mut ChaCha8Rng) -> usize {
    obs.actions().len() - 1
}

#[test]
fn simple_policies_run_whole_batches() {
    let policies: [&dyn Policy; 2] = [&always_first, &always_last];
    for policy in policies {
        let report = run_batch(&fixture(), &config(10, 0), policy);
        assert!(report.check().is_ok());
        assert_eq!(report.matchups.len(), 3);
    }
}

fn out_of_bounds(obs: &Observation<'_>, _: &mut ChaCha8Rng) -> usize {
    obs.actions().len()
}

#[test]
#[should_panic(expected = "policy returned index")]
fn an_index_out_of_bounds_is_a_contract_violation() {
    run_batch(&fixture(), &config(1, 0), &out_of_bounds);
}

fn refuses_concede(obs: &Observation<'_>, rng: &mut ChaCha8Rng) -> usize {
    assert!(
        !obs.actions().contains(&Action::Concede),
        "Concede reached the policy"
    );
    random_policy(obs, rng)
}

#[test]
fn concede_is_never_offered_nor_played() {
    for contenders in [fixture(), starter()] {
        let report = run_batch(&contenders, &config(30, 4), &refuses_concede);
        assert!(report.matchups.iter().all(|matchup| matchup.concede == 0));
    }
    let mut steps = 0;
    play_game(
        fixture_setup(),
        1,
        &refuses_concede,
        DEFAULT_MAX_ACTIONS,
        |actions, _| {
            steps += 1;
            assert!(!actions.contains(&Action::Concede));
            assert!(!actions.is_empty());
        },
    );
    assert!(steps > 0);
}

// 4. Seat alternation.

#[test]
fn the_deck_with_the_lowest_index_alternates_seats_in_a_cross_matchup() {
    let (report, records) = observed(&starter(), &config(100, 3), &random_policy);
    let cross = of_matchup(&records, 0);
    assert_eq!(cross.len(), 100);
    for (index, record) in cross.iter().enumerate() {
        let expected = if index % 2 == 0 {
            PlayerId::One
        } else {
            PlayerId::Two
        };
        assert_eq!(record.index, index as u64);
        assert_eq!(record.seat_of_a, expected, "game {index}");
    }
    // Wins are counted by deck identity, not by seat.
    let by_identity = cross
        .iter()
        .filter(|r| matches!(r.winner, Some((seat, _)) if seat == r.seat_of_a))
        .count() as u64;
    let by_seat_one = cross
        .iter()
        .filter(|r| matches!(r.winner, Some((PlayerId::One, _))))
        .count() as u64;
    assert_ne!(
        by_identity, by_seat_one,
        "the test data tells the two apart"
    );
    assert_eq!(report.matchups[0].wins_a, by_identity);
    assert_eq!(report.matchups[0].wins_b, 100 - by_identity);
}

/// The starter decks plus a third deck, to get three cross matchups.
fn three_decks() -> Vec<Contender> {
    let mut contenders = starter();
    let mut third = contenders[0].clone();
    third.id = "third".to_owned();
    contenders.push(third);
    contenders
}

#[test]
fn every_cross_matchup_alternates_from_its_own_first_game() {
    let (report, records) = observed(&three_decks(), &config(5, 3), &random_policy);
    let names: Vec<(String, String)> = report
        .matchups
        .iter()
        .map(|m| (m.deck_a.clone(), m.deck_b.clone()))
        .collect();
    let pair = |a: &str, b: &str| (a.to_owned(), b.to_owned());
    assert_eq!(
        names,
        [
            pair("rush", "bulwark"),
            pair("rush", "third"),
            pair("bulwark", "third"),
            pair("rush", "rush"),
            pair("bulwark", "bulwark"),
            pair("third", "third"),
        ]
    );
    for matchup in 0..3 {
        let seats: Vec<PlayerId> = of_matchup(&records, matchup)
            .iter()
            .map(|r| r.seat_of_a)
            .collect();
        assert_eq!(
            seats,
            [
                PlayerId::One,
                PlayerId::Two,
                PlayerId::One,
                PlayerId::Two,
                PlayerId::One
            ],
            "matchup {matchup}"
        );
    }
}

#[test]
fn a_mirror_matchup_has_the_same_deck_on_both_sides() {
    let (report, _) = observed(&starter(), &config(10, 3), &random_policy);
    for matchup in &report.matchups[1..] {
        assert_eq!(matchup.deck_a, matchup.deck_b);
        assert_eq!(matchup.kind, duel_simulator::Kind::Mirror);
    }
    assert_eq!(report.matchups[0].kind, duel_simulator::Kind::Cross);
    assert_eq!(report.matchups.len(), 3);
}

// 5. Mirrors.

#[test]
fn a_mirror_has_equal_initiatives_and_a_first_player_that_is_not_constant() {
    let contenders = starter();
    for contender in &contenders {
        // Both seats hold the very same hero: `run_batch` never edits one.
        let (_, records) = observed(
            std::slice::from_ref(contender),
            &config(1000, 0),
            &random_policy,
        );
        assert_eq!(records.len(), 1000);
        let one_starts = records.iter().filter(|r| r.first == PlayerId::One).count();
        assert!(
            (400..=600).contains(&one_starts),
            "{}: One starts {one_starts} of 1000",
            contender.id
        );
        // The engine has no seat bias: in a mirror the seat that wins is as even as who starts.
        let one_wins = records
            .iter()
            .filter(|r| matches!(r.winner, Some((PlayerId::One, _))))
            .count();
        assert!(
            (400..=600).contains(&one_wins),
            "{}: One wins {one_wins} of 1000",
            contender.id
        );
    }
}

// 6. The action cap.

#[test]
fn a_cap_of_3_interrupts_every_game() {
    let mut capped = config(20, 0);
    capped.max_actions = 3;
    for contenders in [fixture(), starter()] {
        let report = run_batch(&contenders, &capped, &random_policy);
        assert!(report.check().is_ok());
        for matchup in &report.matchups {
            assert_eq!(matchup.interrupted, 20);
            assert_eq!((matchup.wins_a, matchup.wins_b), (0, 0));
            assert_eq!(matchup.first_player_wins, 0);
            assert_eq!(
                (matchup.hero_down, matchup.exhaustion, matchup.concede),
                (0, 0, 0)
            );
            assert_eq!(matchup.total_actions, 0);
        }
        assert!(report.render().contains("n/a"));
    }
}

#[test]
fn the_default_cap_interrupts_nothing() {
    for contenders in [fixture(), starter()] {
        let report = run_batch(&contenders, &config(50, 0), &random_policy);
        assert!(report.matchups.iter().all(|m| m.interrupted == 0));
    }
}

#[test]
fn a_game_ending_exactly_at_the_cap_is_finished_and_one_action_less_interrupts_it() {
    let seed = game_seed(0, 0, 0);
    let full = play_game(
        fixture_setup(),
        seed,
        &random_policy,
        DEFAULT_MAX_ACTIONS,
        |_, _| {},
    );
    assert!(!full.interrupted && full.winner.is_some());
    let n = full.actions;
    assert!(n > 1);
    let exactly = play_game(fixture_setup(), seed, &random_policy, n, |_, _| {});
    assert_eq!(exactly, full);
    let short = play_game(fixture_setup(), seed, &random_policy, n - 1, |_, _| {});
    assert!(short.interrupted);
    assert_eq!((short.winner, short.actions), (None, n - 1));

    // The same edge, through the batch: matchup 0, game 0 is that very game.
    let mut at_cap = config(1, 0);
    at_cap.max_actions = n;
    assert_eq!(
        run_batch(&fixture(), &at_cap, &random_policy).matchups[0].interrupted,
        0
    );
    at_cap.max_actions = n - 1;
    assert_eq!(
        run_batch(&fixture(), &at_cap, &random_policy).matchups[0].interrupted,
        1
    );
}

#[test]
fn interrupted_games_count_apart_and_leave_every_other_figure_alone() {
    let contenders = starter();
    let (_, records) = observed(&contenders, &config(100, 6), &random_policy);
    let mut actions: Vec<u32> = records.iter().map(|r| r.actions).collect();
    actions.sort_unstable();
    let cap = actions[actions.len() / 2];
    let mut capped = config(100, 6);
    capped.max_actions = cap;
    let report = run_batch(&contenders, &capped, &random_policy);
    assert!(report.check().is_ok());
    for (index, matchup) in report.matchups.iter().enumerate() {
        // The cap only truncates: a game's course does not depend on it.
        let finished: Vec<GameRecord> = of_matchup(&records, index)
            .into_iter()
            .filter(|r| r.actions <= cap)
            .collect();
        let interrupted = 100 - finished.len() as u64;
        assert!(interrupted > 0 && interrupted < 100, "mixed batch");
        assert_eq!(matchup.interrupted, interrupted);
        let wins_a = finished
            .iter()
            .filter(|r| matches!(r.winner, Some((seat, _)) if seat == r.seat_of_a))
            .count() as u64;
        assert_eq!(matchup.wins_a, wins_a);
        assert_eq!(matchup.wins_b, finished.len() as u64 - wins_a);
        let cause = |wanted: WinReason| {
            finished
                .iter()
                .filter(|r| matches!(r.winner, Some((_, reason)) if reason == wanted))
                .count() as u64
        };
        assert_eq!(matchup.hero_down, cause(WinReason::HeroDown));
        assert_eq!(matchup.exhaustion, cause(WinReason::Exhaustion));
        assert_eq!(
            matchup.total_actions,
            finished.iter().map(|r| u64::from(r.actions)).sum::<u64>()
        );
        let first_wins = finished
            .iter()
            .filter(|r| matches!(r.winner, Some((seat, _)) if seat == r.first))
            .count() as u64;
        assert_eq!(matchup.first_player_wins, first_wins);
    }
}

// 7. Termination.

#[test]
fn every_game_ends_by_a_hero_going_down_or_exhaustion() {
    for contenders in [fixture(), starter()] {
        let (report, records) = observed(&contenders, &config(100, 8), &random_policy);
        assert_eq!(records.len(), 300);
        for record in &records {
            assert!(!record.interrupted, "{record:?}");
            assert!(matches!(
                record.winner,
                Some((_, WinReason::HeroDown | WinReason::Exhaustion))
            ));
        }
        for matchup in &report.matchups {
            assert_eq!(matchup.interrupted, 0);
            assert_eq!(matchup.hero_down + matchup.exhaustion, 100);
            assert_eq!(matchup.concede, 0);
        }
    }
}

// 8. Tuning by data alone.

/// Replaces the number after the `nth` (from 0) occurrence of `"key":` in `json`.
fn set_number(json: &str, key: &str, nth: usize, value: u64) -> String {
    let needle = format!("\"{key}\":");
    let at = json
        .match_indices(&needle)
        .nth(nth)
        .map(|(at, _)| at + needle.len())
        .expect("the key is there");
    let rest = &json[at..];
    let number_start = rest.len() - rest.trim_start().len();
    let number_len = rest[number_start..]
        .find(|c: char| !c.is_ascii_digit())
        .expect("a number follows");
    format!(
        "{} {value}{}",
        &json[..at],
        &rest[number_start + number_len..]
    )
}

fn with_decks(decks: &str) -> Vec<Contender> {
    contenders_from(FIXTURE_CARDS, decks).expect("the edited data is valid")
}

#[test]
fn a_huge_health_in_the_data_alone_wins_almost_always() {
    for (strong, weak) in [(0, 1), (1, 0)] {
        let decks = set_number(FIXTURE_DECKS, "health", strong, 10_000);
        let decks = set_number(&decks, "health", weak, 1);
        let report = run_batch(&with_decks(&decks), &config(100, 5), &random_policy);
        let cross = &report.matchups[0];
        let strong_wins = if strong == 0 {
            cross.wins_a
        } else {
            cross.wins_b
        };
        assert!(
            strong_wins >= 95,
            "strong deck {strong} won {strong_wins}/100"
        );
    }
}

#[test]
fn a_higher_initiative_in_the_data_alone_always_starts() {
    let decks = set_number(FIXTURE_DECKS, "initiative", 1, 9);
    let contenders = with_decks(&decks);
    assert!(contenders[1].player.hero.initiative > contenders[0].player.hero.initiative);
    let (report, records) = observed(&contenders, &config(100, 5), &random_policy);
    let cross = of_matchup(&records, 0);
    for record in &cross {
        assert_eq!(record.first, record.seat_of_a.opponent(), "{record:?}");
    }
    // The first player is always deck B, so its wins are the first player's wins.
    assert_eq!(
        report.matchups[0].first_player_wins,
        report.matchups[0].wins_b
    );
}
