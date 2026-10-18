//! A confrontation of two policies (`--versus`, #190): the same decks, rules and engine seeds,
//! the candidate policy on one seat and the reference on the other, the seats crossed every four
//! games. Pure and deterministic, like [`run_batch`](crate::run_batch): integer counters, no
//! clock, and the only floats are the ones [`fmt_share`] and [`fmt_mean`] already write.
//!
//! In a matchup of `N` games, game `i` seats deck A at One when `i % 2 == 0` (always, in a mirror)
//! and the candidate policy at One when `(i / 2) % 2 == 0`: the four (deck, policy) seat
//! combinations come back every four games, so `N` must be a multiple of 4. The game seed is the
//! one of [`run_batch`](crate::run_batch) ([`game_seed`]), so the two policies, and any two batches,
//! replay the same engine seeds (paired comparison); the policy of a seat gets the generator of
//! its seat, never one by name.

use std::collections::BTreeMap;

use crate::batch::{
    BatchConfig, Contender, GameRecord, NamedPolicy, game_seed, game_setup, matchups,
    play_game_seats,
};
use crate::report::{Kind, rules_label, table};
use crate::stats::{fmt_mean, fmt_share};
use duel_engine::{Event, PlayerId, RuleSet, WinReason};

/// What one game of a confrontation came to, shown to the observer of
/// [`run_versus_observed`] once the game is over.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct VersusGame {
    /// The game as [`run_batch_observed`](crate::run_batch_observed) records it.
    pub record: GameRecord,
    /// The seat the candidate policy played.
    pub candidate_seat: PlayerId,
}

/// What one action did, read from the events it produced (an attack carries no target).
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub struct StepCounts {
    /// The player whose creature attacked (`Event::Attacked`).
    pub attack_by: Option<PlayerId>,
    /// The attack hit the hero: an `Event::Damaged` follows the `Attacked` of the same action.
    pub hero_hit: bool,
    /// The acting player passed the hand (`Event::ActivePlayerChanged` names the new active one).
    pub hand_over_by: Option<PlayerId>,
}

/// The attack and the hand-over of the action that produced `events`.
pub fn step_counts(events: &[Event]) -> StepCounts {
    let mut counts = StepCounts::default();
    for event in events {
        match event {
            Event::Attacked { player, .. } => counts.attack_by = Some(*player),
            Event::Damaged { .. } if counts.attack_by.is_some() => counts.hero_hit = true,
            Event::ActivePlayerChanged { player } => {
                counts.hand_over_by = Some(player.opponent());
            }
            _ => {}
        }
    }
    counts
}

/// The counters of one policy in one matchup.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct VersusSide {
    pub wins: u64,
    /// Wins of the games this policy played at seat One.
    pub wins_as_one: u64,
    /// Attacks its creatures made.
    pub attacks: u64,
    /// ... of which on the enemy hero.
    pub hero_attacks: u64,
    /// Times it passed the hand.
    pub hand_overs: u64,
}

/// The counters of a set of games (one matchup, or all of them). Invariants (see
/// [`VersusReport::check`]): `candidate.wins + reference.wins + interrupted = games`, the finished
/// games are the sum of `durations`, `hero_attacks <= attacks`.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct VersusTally {
    pub games: u64,
    /// Games cut short by the action cap: won by nobody, never hidden.
    pub interrupted: u64,
    /// Finished games won by deck A (in a mirror, by seat One).
    pub deck_a_wins: u64,
    /// Finished games won by seat One.
    pub seat_one_wins: u64,
    /// Actions summed over the finished games.
    pub total_actions: u64,
    /// Actions -> number of FINISHED games of that length.
    pub durations: BTreeMap<u32, u64>,
    pub candidate: VersusSide,
    pub reference: VersusSide,
}

impl VersusTally {
    /// Games that ran to their end.
    fn finished(&self) -> u64 {
        self.games.saturating_sub(self.interrupted)
    }

    fn add(&mut self, other: &Self) {
        self.games += other.games;
        self.interrupted += other.interrupted;
        self.deck_a_wins += other.deck_a_wins;
        self.seat_one_wins += other.seat_one_wins;
        self.total_actions += other.total_actions;
        for (&actions, &count) in &other.durations {
            *self.durations.entry(actions).or_default() += count;
        }
        for (mine, theirs) in [
            (&mut self.candidate, &other.candidate),
            (&mut self.reference, &other.reference),
        ] {
            mine.wins += theirs.wins;
            mine.wins_as_one += theirs.wins_as_one;
            mine.attacks += theirs.attacks;
            mine.hero_attacks += theirs.hero_attacks;
            mine.hand_overs += theirs.hand_overs;
        }
    }
}

/// One matchup of decks of a confrontation.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct VersusMatchup {
    pub deck_a: String,
    pub deck_b: String,
    pub kind: Kind,
    pub tally: VersusTally,
}

impl VersusMatchup {
    fn name(&self) -> String {
        match self.kind {
            Kind::Cross => format!("{} vs {}", self.deck_a, self.deck_b),
            Kind::Mirror => format!("{} (mirror)", self.deck_a),
        }
    }
}

/// A whole confrontation: the settings and one tally per matchup, in the order of the batch.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct VersusReport {
    /// `name@version`.
    pub set: String,
    pub seed: u64,
    pub games_per_matchup: u64,
    pub max_actions: u32,
    pub rules: RuleSet,
    /// The names of the two policies (`[a-z0-9_-]`, checked by [`Self::check`]; the binary makes them distinct, the library lets a policy face itself).
    pub candidate: String,
    pub reference: String,
    pub matchups: Vec<VersusMatchup>,
}

const RESULT_HEADERS: [&str; 7] = [
    "matchup",
    "games",
    "candidate wins",
    "reference wins",
    "interrupted",
    "seat One",
    "deck A",
];

const BEHAVIOUR_HEADERS: [&str; 9] = [
    "matchup",
    "mean actions",
    "cand attacks",
    "cand on hero",
    "ref attacks",
    "ref on hero",
    "hand-overs c/r",
    "wins as One c",
    "wins as One r",
];

impl VersusReport {
    /// The counters of every matchup added up.
    pub fn total(&self) -> VersusTally {
        let mut total = VersusTally::default();
        for matchup in &self.matchups {
            total.add(&matchup.tally);
        }
        total
    }

    /// Deterministic plain-text rendering, every line ending with `\n`: the header, then the
    /// results (the Wilson 95% interval of the candidate's wins out of ALL the games, interrupted
    /// ones included) and the behaviour of each policy, one row per matchup and a total row.
    pub fn render(&self) -> String {
        let total = self.total();
        let rows: Vec<(String, &VersusTally)> = self
            .matchups
            .iter()
            .map(|matchup| (matchup.name(), &matchup.tally))
            .chain(std::iter::once(("total".to_owned(), &total)))
            .collect();
        let results: Vec<[String; 7]> = rows
            .iter()
            .map(|(name, t)| {
                [
                    name.clone(),
                    t.games.to_string(),
                    fmt_share(t.candidate.wins, t.games),
                    t.reference.wins.to_string(),
                    t.interrupted.to_string(),
                    t.seat_one_wins.to_string(),
                    t.deck_a_wins.to_string(),
                ]
            })
            .collect();
        let behaviour: Vec<[String; 9]> = rows
            .iter()
            .map(|(name, t)| {
                [
                    name.clone(),
                    fmt_mean(t.total_actions, t.finished()),
                    t.candidate.attacks.to_string(),
                    fmt_share(t.candidate.hero_attacks, t.candidate.attacks),
                    t.reference.attacks.to_string(),
                    fmt_share(t.reference.hero_attacks, t.reference.attacks),
                    format!("{}/{}", t.candidate.hand_overs, t.reference.hand_overs),
                    t.candidate.wins_as_one.to_string(),
                    t.reference.wins_as_one.to_string(),
                ]
            })
            .collect();
        let mut text = format!(
            "versus | set: {} | seed: {} | games per matchup: {} | action cap: {} | candidate: {} | reference: {} | rules: {}\n",
            self.set,
            self.seed,
            self.games_per_matchup,
            self.max_actions,
            self.candidate,
            self.reference,
            rules_label(&self.rules)
        );
        text += &table(RESULT_HEADERS, &results);
        text += "\n";
        text += &table(BEHAVIOUR_HEADERS, &behaviour);
        text
    }

    /// Checks the counter invariants of every matchup and the policy names.
    pub fn check(&self) -> Result<(), String> {
        for name in [&self.candidate, &self.reference] {
            let valid = !name.is_empty()
                && name.bytes().all(|b| {
                    b.is_ascii_lowercase() || b.is_ascii_digit() || b == b'_' || b == b'-'
                });
            if !valid {
                return Err(format!("invalid policy name \"{name}\""));
            }
        }
        for matchup in &self.matchups {
            let t = &matchup.tally;
            let name = matchup.name();
            let fail = |what: &str| Err(format!("{name}: {what}"));
            if t.candidate.wins + t.reference.wins + t.interrupted != t.games {
                return fail("candidate wins + reference wins + interrupted != games");
            }
            if t.durations.values().sum::<u64>() != t.finished() {
                return fail("the durations do not add up to the finished games");
            }
            if t.durations
                .iter()
                .map(|(&a, &n)| u64::from(a) * n)
                .sum::<u64>()
                != t.total_actions
            {
                return fail("the durations do not add up to the total of actions");
            }
            if t.candidate.wins_as_one + t.reference.wins_as_one != t.seat_one_wins {
                return fail("wins as One do not add up to the wins of seat One");
            }
            if t.seat_one_wins > t.finished() || t.deck_a_wins > t.finished() {
                return fail("more wins of a seat or a deck than finished games");
            }
            if matchup.kind == Kind::Mirror && t.deck_a_wins != t.seat_one_wins {
                return fail("in a mirror, deck A is seat One");
            }
            for (side, label) in [(&t.candidate, "candidate"), (&t.reference, "reference")] {
                if side.hero_attacks > side.attacks {
                    return fail(&format!("{label}: more attacks on the hero than attacks"));
                }
                if side.wins_as_one > side.wins {
                    return fail(&format!("{label}: more wins as One than wins"));
                }
            }
        }
        Ok(())
    }
}

/// Plays `config.games` games of every matchup of `contenders` between `candidate` and
/// `reference` and reports them. `Err` when `config.games` is not a multiple of 4 (the seats
/// are crossed over four games).
pub fn run_versus(
    contenders: &[Contender],
    config: &BatchConfig,
    candidate: NamedPolicy,
    reference: NamedPolicy,
) -> Result<VersusReport, String> {
    run_versus_observed(contenders, config, candidate, reference, |_| {})
}

/// [`run_versus`], showing `on_game` every game once it is over.
pub fn run_versus_observed(
    contenders: &[Contender],
    config: &BatchConfig,
    candidate: NamedPolicy,
    reference: NamedPolicy,
    mut on_game: impl FnMut(&VersusGame),
) -> Result<VersusReport, String> {
    if !config.games.is_multiple_of(4) {
        return Err(format!(
            "a confrontation needs a number of games per matchup that is a multiple of 4 (got {}): the seats of the decks and of the policies are crossed every four games",
            config.games
        ));
    }
    let mut report = VersusReport {
        set: config.data_versions.join(", "),
        seed: config.seed,
        games_per_matchup: config.games,
        max_actions: config.max_actions,
        rules: config.rules,
        candidate: candidate.name.to_owned(),
        reference: reference.name.to_owned(),
        matchups: Vec::new(),
    };
    for (matchup, (a, b)) in matchups(contenders.len()).into_iter().enumerate() {
        let mut tally = VersusTally {
            games: config.games,
            ..VersusTally::default()
        };
        for index in 0..config.games {
            // The decks cross seats as in `run_batch`; the policies cross every two games.
            let a_is_one = a == b || index % 2 == 0;
            let candidate_seat = if (index / 2) % 2 == 0 {
                PlayerId::One
            } else {
                PlayerId::Two
            };
            let (one, two) = if a_is_one { (a, b) } else { (b, a) };
            let (policy_one, policy_two) = if candidate_seat == PlayerId::One {
                (candidate.policy, reference.policy)
            } else {
                (reference.policy, candidate.policy)
            };
            // Indexed by seat: the counts of the policy on seat One, then on seat Two.
            let mut seats = [VersusSide::default(), VersusSide::default()];
            let setup = game_setup(contenders, config, one, two);
            let mut record = play_game_seats(
                setup,
                game_seed(config.seed, matchup, index),
                policy_one,
                policy_two,
                config.max_actions,
                |_, events| {
                    let counts = step_counts(events);
                    if let Some(player) = counts.attack_by {
                        let side = &mut seats[seat_index(player)];
                        side.attacks += 1;
                        side.hero_attacks += u64::from(counts.hero_hit);
                    }
                    if let Some(player) = counts.hand_over_by {
                        seats[seat_index(player)].hand_overs += 1;
                    }
                },
            );
            record.matchup = matchup;
            record.index = index;
            record.seat_of_a = if a_is_one {
                PlayerId::One
            } else {
                PlayerId::Two
            };
            let (mut mine, mut theirs) = if candidate_seat == PlayerId::One {
                (seats[0].clone(), seats[1].clone())
            } else {
                (seats[1].clone(), seats[0].clone())
            };
            if let Some((winner, reason)) = record.winner {
                debug_assert!(reason != WinReason::Concede, "a policy never concedes");
                let winning = if winner == candidate_seat {
                    &mut mine
                } else {
                    &mut theirs
                };
                winning.wins += 1;
                winning.wins_as_one += u64::from(winner == PlayerId::One);
                if winner == record.seat_of_a {
                    tally.deck_a_wins += 1;
                }
                tally.seat_one_wins += u64::from(winner == PlayerId::One);
                tally.total_actions += u64::from(record.actions);
                *tally.durations.entry(record.actions).or_default() += 1;
            } else {
                tally.interrupted += 1;
            }
            for (side, game_side) in [
                (&mut tally.candidate, &mine),
                (&mut tally.reference, &theirs),
            ] {
                side.wins += game_side.wins;
                side.wins_as_one += game_side.wins_as_one;
                side.attacks += game_side.attacks;
                side.hero_attacks += game_side.hero_attacks;
                side.hand_overs += game_side.hand_overs;
            }
            on_game(&VersusGame {
                record,
                candidate_seat,
            });
        }
        report.matchups.push(VersusMatchup {
            deck_a: contenders[a].id.clone(),
            deck_b: contenders[b].id.clone(),
            kind: if a == b { Kind::Mirror } else { Kind::Cross },
            tally,
        });
    }
    debug_assert!(report.check().is_ok(), "{:?}", report.check());
    Ok(report)
}

/// 0 for seat One, 1 for seat Two.
fn seat_index(player: PlayerId) -> usize {
    match player {
        PlayerId::One => 0,
        PlayerId::Two => 1,
    }
}
