//! Batches of seeded AI-vs-AI games: the matchups between decks, the seeds derived from one
//! batch seed, alternating seats, mirrors and the action cap. Single-threaded and free of any
//! source of randomness other than [`BatchConfig::seed`] (no clock, no `thread_rng`, no
//! `HashMap`): the same batch always gives the same [`Report`].

use crate::report::{Kind, MatchupReport, Report};
use duel_ai::{Policy, derive_seed, policy_rng};
use duel_cards::{parse_cards, parse_decks};
use duel_engine::{
    Action, Event, GameSetup, Observation, PlayerId, PlayerSetup, RuleSet, WinReason, apply,
    new_game, player_setup,
};

/// Defensive cap on the actions of one game: every game ends by itself, but a rule bug must
/// not hang the batch.
pub const DEFAULT_MAX_ACTIONS: u32 = 10_000;

/// A deck ready to play: its id (from the data) and its hero and cards.
#[derive(Debug, Clone)]
pub struct Contender {
    pub id: String,
    pub player: PlayerSetup,
}

/// Every deck of `decks` as a contender, in file order, with `cards` resolving their card ids.
/// Pure: nothing is read from disk; invalid data is an `Err` message, never a panic.
pub fn contenders_from(cards: &str, decks: &str) -> Result<Vec<Contender>, String> {
    let cards = parse_cards(cards).map_err(|error| error.to_string())?;
    let decks = parse_decks(decks).map_err(|error| error.to_string())?;
    decks
        .iter()
        .map(|deck| {
            Ok(Contender {
                id: deck.id.clone(),
                player: player_setup(deck, &cards).map_err(|error| error.to_string())?,
            })
        })
        .collect()
}

/// The settings of a batch.
#[derive(Debug, Clone)]
pub struct BatchConfig {
    /// Games played by each matchup.
    pub games: u64,
    /// The only source of randomness of the batch.
    pub seed: u64,
    /// Actions after which a game without a winner is interrupted.
    pub max_actions: u32,
    /// Recorded in every game (and in the report: joined with `, `).
    pub data_versions: Vec<String>,
}

/// What one game came to. `matchup`, `index` and `seat_of_a` are filled in by the batch;
/// [`play_game`] leaves them at `0`, `0` and `One`.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct GameRecord {
    pub matchup: usize,
    pub index: u64,
    /// The seat of deck A (the lowest-indexed deck of the matchup).
    pub seat_of_a: PlayerId,
    /// The player who was active right after `new_game`.
    pub first: PlayerId,
    /// `None` for an interrupted game.
    pub winner: Option<(PlayerId, WinReason)>,
    /// Actions applied.
    pub actions: u32,
    /// Creatures that died.
    pub deaths: usize,
    pub interrupted: bool,
}

/// The seed handed, as is, to the engine for game `index` of `matchup`.
pub fn game_seed(batch_seed: u64, matchup: usize, index: u64) -> u64 {
    derive_seed(derive_seed(batch_seed, matchup as u64), index)
}

/// Plays one game of `setup` seeded `seed`, showing `on_step` each step: the actions the
/// policy chose among (never `Concede`) and the events of the action it applied.
///
/// The game is interrupted (never hidden) when `max_actions` actions went by without a winner,
/// when no action is playable while the game runs, or when the engine refuses a legal action.
///
/// # Panics
/// When the policy returns an index outside the list it was given.
pub fn play_game(
    setup: GameSetup,
    seed: u64,
    policy: &dyn Policy,
    max_actions: u32,
    mut on_step: impl FnMut(&[Action], &[Event]),
) -> GameRecord {
    let (mut state, _) = new_game(setup, seed).expect("the setup is valid");
    let mut rng_one = policy_rng(seed, PlayerId::One);
    let mut rng_two = policy_rng(seed, PlayerId::Two);
    let mut record = GameRecord {
        matchup: 0,
        index: 0,
        seat_of_a: PlayerId::One,
        first: state.active(),
        winner: None,
        actions: 0,
        deaths: 0,
        interrupted: false,
    };
    loop {
        if let Some(winner) = state.winner() {
            record.winner = Some(winner);
            return record;
        }
        if record.actions >= max_actions {
            break;
        }
        let Some(obs) = Observation::of(&state) else {
            break;
        };
        if obs.actions().is_empty() {
            break;
        }
        let rng = match obs.seat() {
            PlayerId::One => &mut rng_one,
            PlayerId::Two => &mut rng_two,
        };
        let chosen = policy.choose(&obs, rng);
        assert!(
            chosen < obs.actions().len(),
            "policy returned index {chosen} for {} actions",
            obs.actions().len()
        );
        // The observation borrows the state: keep the list it offered, then let go of it.
        let offered = obs.actions().to_vec();
        let Some(&action) = offered.get(chosen) else {
            break;
        };
        let Ok(events) = apply(&mut state, action) else {
            break;
        };
        on_step(&offered, &events);
        record.actions += 1;
        record.deaths += events
            .iter()
            .filter(|event| matches!(event, Event::Died { .. }))
            .count();
    }
    record.interrupted = true;
    record
}

/// The matchups of `decks` decks: every cross pair `i < j` in lexicographic order, then every
/// mirror in deck order. The order is part of the determinism contract (it enters the seeds).
fn matchups(decks: usize) -> Vec<(usize, usize)> {
    let mut list = Vec::new();
    for i in 0..decks {
        for j in i + 1..decks {
            list.push((i, j));
        }
    }
    list.extend((0..decks).map(|i| (i, i)));
    list
}

/// Plays `config.games` games of every matchup of `contenders` and reports them.
pub fn run_batch(contenders: &[Contender], config: &BatchConfig, policy: &dyn Policy) -> Report {
    run_batch_observed(contenders, config, policy, |_| {})
}

/// [`run_batch`], showing `on_game` every game once it is over.
pub fn run_batch_observed(
    contenders: &[Contender],
    config: &BatchConfig,
    policy: &dyn Policy,
    mut on_game: impl FnMut(&GameRecord),
) -> Report {
    let mut report = Report {
        set: config.data_versions.join(", "),
        seed: config.seed,
        games_per_matchup: config.games,
        max_actions: config.max_actions,
        matchups: Vec::new(),
    };
    for (matchup, (a, b)) in matchups(contenders.len()).into_iter().enumerate() {
        let mut tally = MatchupReport {
            deck_a: contenders[a].id.clone(),
            deck_b: contenders[b].id.clone(),
            kind: if a == b { Kind::Mirror } else { Kind::Cross },
            games: config.games,
            interrupted: 0,
            wins_a: 0,
            wins_b: 0,
            first_player_wins: 0,
            hero_down: 0,
            exhaustion: 0,
            concede: 0,
            total_actions: 0,
        };
        for index in 0..config.games {
            // In a cross matchup A alternates between the seats; in a mirror, A is One.
            let a_is_one = a == b || index % 2 == 0;
            let (one, two) = if a_is_one { (a, b) } else { (b, a) };
            let setup = GameSetup {
                one: contenders[one].player.clone(),
                two: contenders[two].player.clone(),
                data_versions: config.data_versions.clone(),
                rules: RuleSet::default(),
            };
            let seed = game_seed(config.seed, matchup, index);
            let mut record = play_game(setup, seed, policy, config.max_actions, |_, _| {});
            record.matchup = matchup;
            record.index = index;
            record.seat_of_a = if a_is_one {
                PlayerId::One
            } else {
                PlayerId::Two
            };
            tally_game(&mut tally, &record);
            on_game(&record);
        }
        report.matchups.push(tally);
    }
    debug_assert!(report.check().is_ok(), "{:?}", report.check());
    report
}

/// Adds one game to its matchup's counters; an interrupted game only counts as such.
fn tally_game(tally: &mut MatchupReport, record: &GameRecord) {
    let Some((winner, reason)) = record.winner else {
        tally.interrupted += 1;
        return;
    };
    if winner == record.seat_of_a {
        tally.wins_a += 1;
    } else {
        tally.wins_b += 1;
    }
    if winner == record.first {
        tally.first_player_wins += 1;
    }
    match reason {
        WinReason::HeroDown => tally.hero_down += 1,
        WinReason::Exhaustion => tally.exhaustion += 1,
        WinReason::Concede => tally.concede += 1,
    }
    tally.total_actions += u64::from(record.actions);
}
