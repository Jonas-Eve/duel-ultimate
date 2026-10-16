//! `player_view` through the public API of the engine (S2-T2 of #52): at every step of whole
//! games, for both seats, the view agrees with the public accessors, with a zone tracker
//! written here from the events only, and with the conservation of cards. The leak tests
//! (which need the identifiers of the piles) live inside the crate, in `view.rs`.

use duel_cards::{parse_cards, parse_decks};
use duel_engine::{
    Action, CardInstanceId, CardView, Event, GameSetup, GameState, PlayerId, WinReason, apply,
    game_setup_from_data, legal_actions, new_game, player_view,
};
use std::collections::{BTreeMap, BTreeSet};

const FIXTURE_CARDS: &str = include_str!("../../../data/fixture/1.0.0/cards.json");
const FIXTURE_DECKS: &str = include_str!("../../../data/fixture/1.0.0/decks.json");
const STARTER_CARDS: &str = include_str!("../../../data/starter/1.0.0/cards.json");
const STARTER_DECKS: &str = include_str!("../../../data/starter/1.0.0/decks.json");

const SEATS: [PlayerId; 2] = [PlayerId::One, PlayerId::Two];

/// (cards, decks, deck of One, deck of Two): rush and bulwark in both seats, mirrors included,
/// and the fixture's two decks.
fn matchups() -> Vec<(&'static str, &'static str, &'static str, &'static str)> {
    let s = (STARTER_CARDS, STARTER_DECKS);
    vec![
        (s.0, s.1, "rush", "bulwark"),
        (s.0, s.1, "bulwark", "rush"),
        (s.0, s.1, "rush", "rush"),
        (s.0, s.1, "bulwark", "bulwark"),
        (FIXTURE_CARDS, FIXTURE_DECKS, "aggro", "control"),
    ]
}

fn setup_of(m: (&str, &str, &str, &str)) -> GameSetup {
    game_setup_from_data(m.0, m.1, m.2, m.3, vec!["test@1".into()]).unwrap()
}

struct Xorshift(u64);

impl Xorshift {
    fn below(&mut self, bound: usize) -> usize {
        self.0 ^= self.0 << 13;
        self.0 ^= self.0 >> 7;
        self.0 ^= self.0 << 17;
        usize::try_from(self.0 % u64::try_from(bound).unwrap()).unwrap()
    }
}

fn ids(cards: &[CardView]) -> Vec<CardInstanceId> {
    cards.iter().map(|c| c.id).collect()
}

/// Independent tracker of what the events say about the zones of a game. The initial hands
/// are read from the accessors (the start events do not list cards); everything after comes
/// from the events, and the stats and names of a card from the setup alone (the instance
/// with id `n` is card `n` of One's deck, then of Two's).
struct Tracker {
    setup: GameSetup,
    hero_health: [u32; 2],
    hand: [BTreeSet<CardInstanceId>; 2],
    deck_count: [usize; 2],
    board: [Vec<CardInstanceId>; 2],
    discard: [Vec<CardInstanceId>; 2],
    health: BTreeMap<CardInstanceId, u32>,
    active: PlayerId,
    budget: u32,
    result: Option<(PlayerId, WinReason)>,
}

fn slot(p: PlayerId) -> usize {
    usize::from(p == PlayerId::Two)
}

impl Tracker {
    fn new(setup: GameSetup, state: &GameState, start: &[Event]) -> Self {
        let mut t = Tracker {
            hero_health: [setup.one.hero.health, setup.two.hero.health],
            hand: [
                state.hand(PlayerId::One).into_iter().collect(),
                state.hand(PlayerId::Two).into_iter().collect(),
            ],
            deck_count: [
                30 - state.hand_len(PlayerId::One),
                30 - state.hand_len(PlayerId::Two),
            ],
            board: [vec![], vec![]],
            discard: [vec![], vec![]],
            health: BTreeMap::new(),
            active: PlayerId::One,
            budget: 0,
            result: None,
            setup,
        };
        for n in 0..60_u32 {
            let stats = t.stats(CardInstanceId(n));
            t.health.insert(CardInstanceId(n), stats.health);
        }
        t.absorb(start);
        t
    }

    fn side(&self, id: CardInstanceId) -> (&duel_engine::PlayerSetup, usize) {
        if id.0 < 30 {
            (&self.setup.one, id.0 as usize)
        } else {
            (&self.setup.two, id.0 as usize - 30)
        }
    }

    fn stats(&self, id: CardInstanceId) -> duel_cards::CreatureStats {
        let (side, i) = self.side(id);
        side.cards[i].clone()
    }

    fn name(&self, id: CardInstanceId) -> Option<String> {
        let (side, i) = self.side(id);
        side.card_names.get(i).cloned()
    }

    fn card(&self, id: CardInstanceId) -> CardView {
        let stats = self.stats(id);
        CardView {
            id,
            name: self.name(id),
            cost: stats.cost,
            force: stats.force,
            attack_cost: stats.attack_cost,
            health: self.health[&id],
            provocation: stats.provocation,
            riposte: stats.riposte,
        }
    }

    fn absorb(&mut self, events: &[Event]) {
        for event in events {
            match *event {
                Event::GameStarted { first, budget, .. } => {
                    (self.active, self.budget) = (first, budget)
                }
                Event::Drew { player, card } => {
                    self.deck_count[slot(player)] -= 1;
                    self.hand[slot(player)].insert(card);
                }
                Event::Played { player, card, .. } => {
                    self.hand[slot(player)].remove(&card);
                    self.board[slot(player)].push(card);
                }
                Event::Damaged { player, amount } => {
                    let h = &mut self.hero_health[slot(player)];
                    *h = h.saturating_sub(amount);
                }
                Event::CreatureDamaged {
                    creature, amount, ..
                }
                | Event::Riposted {
                    creature, amount, ..
                } => {
                    let h = self.health.get_mut(&creature).unwrap();
                    *h = h.saturating_sub(amount);
                }
                Event::Died { player, creature } => {
                    self.board[slot(player)].retain(|c| *c != creature);
                    self.discard[slot(player)].push(creature);
                }
                Event::TokenMoved { player, budget } => {
                    (self.active, self.budget) = (player, budget)
                }
                Event::GameWon { winner, reason } => self.result = Some((winner, reason)),
                Event::EffectActivated { .. }
                | Event::Attacked { .. }
                | Event::ActivePlayerChanged { .. } => {}
            }
        }
    }

    fn zone(&self, ids: impl IntoIterator<Item = CardInstanceId>) -> Vec<CardView> {
        ids.into_iter().map(|id| self.card(id)).collect()
    }
}

/// What a view must say, checked against the accessors and the tracker.
fn check_views(state: &GameState, t: &Tracker, context: &str) {
    let views = SEATS.map(|seat| player_view(state, seat));
    let mut seen = BTreeSet::new();
    for (view, seat) in views.iter().zip(SEATS) {
        let opp = seat.opponent();
        // (a) the public accessors
        assert_eq!(view.seat, seat, "{context}");
        assert_eq!(view.active, state.active(), "{context}");
        assert_eq!(view.budget, state.budget(), "{context}");
        assert_eq!(
            view.result.map(|r| (r.winner, r.reason)),
            state.winner(),
            "{context}"
        );
        assert_eq!(
            ids(&view.you.hand),
            state.hand(seat),
            "{context}: hand order"
        );
        assert_eq!(ids(&view.you.board), state.board(seat), "{context}");
        assert_eq!(ids(&view.you.discard), state.discard(seat), "{context}");
        assert_eq!(ids(&view.opponent.board), state.board(opp), "{context}");
        assert_eq!(ids(&view.opponent.discard), state.discard(opp), "{context}");
        assert_eq!(view.you.deck_count, state.deck_len(seat), "{context}");
        assert_eq!(view.opponent.deck_count, state.deck_len(opp), "{context}");
        assert_eq!(view.opponent.hand_count, state.hand_len(opp), "{context}");
        assert_eq!(view.you.hero.health, state.hero_health(seat), "{context}");
        assert_eq!(
            view.opponent.hero.health,
            state.hero_health(opp),
            "{context}"
        );
        for card in view.you.board.iter().chain(&view.opponent.board) {
            assert_eq!(
                Some(card.health),
                state.creature_health(card.id),
                "{context}"
            );
        }
        // (b) the tracker, from the events and the setup only
        let (mine, theirs) = (slot(seat), slot(opp));
        assert_eq!(
            (view.active, view.budget),
            (t.active, t.budget),
            "{context}"
        );
        assert_eq!(
            view.result.map(|r| (r.winner, r.reason)),
            t.result,
            "{context}"
        );
        assert_eq!(view.you.hero.health, t.hero_health[mine], "{context}");
        assert_eq!(
            view.opponent.hero.health, t.hero_health[theirs],
            "{context}"
        );
        let mut hand = view.you.hand.clone();
        hand.sort_by_key(|c| c.id);
        assert_eq!(
            hand,
            t.zone(t.hand[mine].iter().copied()),
            "{context}: hand"
        );
        assert_eq!(view.opponent.hand_count, t.hand[theirs].len(), "{context}");
        assert_eq!(
            (view.you.deck_count, view.opponent.deck_count),
            (t.deck_count[mine], t.deck_count[theirs]),
            "{context}"
        );
        assert_eq!(
            view.you.board,
            t.zone(t.board[mine].iter().copied()),
            "{context}: board"
        );
        assert_eq!(
            view.opponent.board,
            t.zone(t.board[theirs].iter().copied()),
            "{context}"
        );
        let dead = |side: usize| -> Vec<CardView> {
            t.discard[side]
                .iter()
                .map(|id| CardView {
                    health: 0,
                    ..t.card(*id)
                })
                .collect()
        };
        assert_eq!(
            view.you.discard,
            dead(mine),
            "{context}: discard in order of death"
        );
        assert_eq!(view.opponent.discard, dead(theirs), "{context}");
        assert!(view.you.discard.iter().all(|c| c.health == 0), "{context}");
        // heroes: name, max_time and effects come from the setup
        let (own_setup, other_setup) = if seat == PlayerId::One {
            (&t.setup.one, &t.setup.two)
        } else {
            (&t.setup.two, &t.setup.one)
        };
        for (hero, source) in [
            (&view.you.hero, own_setup),
            (&view.opponent.hero, other_setup),
        ] {
            assert_eq!(hero.name, source.hero.name, "{context}");
            assert_eq!(hero.max_time, source.hero.max_time, "{context}");
            assert_eq!(hero.effects, source.hero.effects, "{context}");
        }
        // (c) conservation
        let all: Vec<CardInstanceId> = view
            .you
            .hand
            .iter()
            .chain(&view.you.board)
            .chain(&view.you.discard)
            .chain(&view.opponent.board)
            .chain(&view.opponent.discard)
            .map(|c| c.id)
            .collect();
        let unique: BTreeSet<_> = all.iter().copied().collect();
        assert_eq!(
            unique.len(),
            all.len(),
            "{context}: ids are pairwise distinct"
        );
        assert_eq!(
            view.you.hand.len()
                + view.you.deck_count
                + view.you.board.len()
                + view.you.discard.len(),
            30,
            "{context}"
        );
        assert_eq!(
            view.opponent.hand_count
                + view.opponent.deck_count
                + view.opponent.board.len()
                + view.opponent.discard.len(),
            30,
            "{context}"
        );
        seen.extend(unique);
    }
    // The public blocks of the two views are the same blocks.
    assert_eq!(views[0].you.board, views[1].opponent.board, "{context}");
    assert_eq!(views[0].you.discard, views[1].opponent.discard, "{context}");
    assert_eq!(views[0].you.hero, views[1].opponent.hero, "{context}");
    assert_eq!(views[0].opponent.hero, views[1].you.hero, "{context}");
    assert_eq!(
        views[0].you.deck_count, views[1].opponent.deck_count,
        "{context}"
    );
    assert_eq!(
        views[0].opponent.deck_count, views[1].you.deck_count,
        "{context}"
    );
    assert_eq!(
        views[0].you.hand.len(),
        views[1].opponent.hand_count,
        "{context}"
    );
    assert_eq!(
        views[1].you.hand.len(),
        views[0].opponent.hand_count,
        "{context}"
    );
}

/// Plays a whole game of random non-conceding actions, `check`ing after every step.
fn play(setup: GameSetup, seed: u32, mut after: impl FnMut(&GameState, &[Event])) -> GameState {
    let (mut state, start) = new_game(setup, u64::from(seed)).unwrap();
    after(&state, &start);
    let mut rng = Xorshift(u64::from(seed).wrapping_mul(0x9E37_79B9_7F4A_7C15) | 1);
    for _ in 0..2_000 {
        let actions: Vec<Action> = legal_actions(&state)
            .into_iter()
            .filter(|a| *a != Action::Concede)
            .collect();
        if actions.is_empty() {
            break;
        }
        let events = apply(&mut state, actions[rng.below(actions.len())]).unwrap();
        after(&state, &events);
    }
    state
}

#[test]
fn at_every_step_the_view_of_each_seat_matches_the_accessors_the_events_and_conservation() {
    let mut steps = 0_usize;
    let mut finished = 0;
    for m in matchups() {
        for seed in 0..100 {
            let setup = setup_of(m);
            let (probe, start) = new_game(setup.clone(), u64::from(seed)).unwrap();
            let mut tracker = Tracker::new(setup.clone(), &probe, &start);
            let end = play(setup, seed, |state, events| {
                if events == start.as_slice() {
                    check_views(
                        state,
                        &tracker,
                        &format!("{}-{} seed {seed} start", m.2, m.3),
                    );
                    return;
                }
                tracker.absorb(events);
                steps += 1;
                check_views(
                    state,
                    &tracker,
                    &format!("{}-{} seed {seed} step {steps}", m.2, m.3),
                );
            });
            finished += usize::from(end.winner().is_some());
        }
    }
    assert!(
        steps > 5_000,
        "the games are long enough to mean something: {steps}"
    );
    assert!(finished > 100, "most games end: {finished}");
}

#[test]
fn every_id_an_event_cites_is_found_with_its_name_in_the_views_after_the_apply() {
    let mut cited = 0_usize;
    for m in matchups() {
        for seed in 0..30 {
            let setup = setup_of(m);
            let named: BTreeMap<u32, Option<String>> = (0..60)
                .map(|n| {
                    let side = if n < 30 { &setup.one } else { &setup.two };
                    (n, side.card_names.get(n as usize % 30).cloned())
                })
                .collect();
            play(setup, seed, |state, events| {
                for event in events {
                    let (player, card) = match *event {
                        Event::Drew { player, card } | Event::Played { player, card, .. } => {
                            (player, card)
                        }
                        Event::Died { player, creature }
                        | Event::CreatureDamaged {
                            player, creature, ..
                        }
                        | Event::Riposted {
                            player, creature, ..
                        } => (player, creature),
                        Event::Attacked {
                            player, attacker, ..
                        } => (player, attacker),
                        _ => continue,
                    };
                    // The seat that owns the card sees it in its own view, whatever the zone.
                    let view = player_view(state, player);
                    let found = view
                        .you
                        .hand
                        .iter()
                        .chain(&view.you.board)
                        .chain(&view.you.discard)
                        .find(|c| c.id == card)
                        .unwrap_or_else(|| {
                            panic!("{event:?} cites {card:?}, absent from the view")
                        });
                    assert_eq!(found.name, named[&card.0], "{event:?}");
                    assert!(found.name.is_some(), "the data sets name every card");
                    cited += 1;
                }
            });
        }
    }
    assert!(cited > 1_000, "{cited}");
}

#[test]
fn a_game_lost_by_exhaustion_at_the_setup_has_a_result_and_a_view() {
    let mut setup = setup_of(matchups()[0]);
    setup.one.hero.effects.clear();
    setup.one.hero.starting_hand = 0;
    setup.one.hero.initiative = 99;
    let (state, _) = new_game(setup, 1).unwrap();
    for seat in SEATS {
        let view = player_view(&state, seat);
        let result = view.result.expect("the game is over");
        assert_eq!(
            (result.winner, result.reason),
            (PlayerId::Two, WinReason::Exhaustion)
        );
    }
}

#[test]
fn a_view_never_changes_what_the_game_does_next() {
    // Two twins fed the same actions; one is looked at after every step, the other never.
    for seed in 0..20 {
        let setup = setup_of(matchups()[0]);
        let (mut watched, _) = new_game(setup.clone(), seed).unwrap();
        let (mut blind, _) = new_game(setup, seed).unwrap();
        let mut rng = Xorshift(seed + 1);
        for _ in 0..300 {
            for seat in SEATS {
                let _ = player_view(&watched, seat);
            }
            let actions = legal_actions(&blind);
            assert_eq!(actions, legal_actions(&watched));
            let action = actions[rng.below(actions.len())];
            assert_eq!(apply(&mut watched, action), apply(&mut blind, action));
            assert_eq!(watched, blind);
            if blind.winner().is_some() {
                break;
            }
        }
    }
}

#[test]
fn the_cards_named_by_the_data_sets_appear_in_the_views_with_their_names() {
    let cards = parse_cards(STARTER_CARDS).unwrap();
    let decks = parse_decks(STARTER_DECKS).unwrap();
    assert!(!cards.is_empty() && !decks.is_empty());
    let setup = setup_of(matchups()[0]);
    let (state, _) = new_game(setup.clone(), 5).unwrap();
    let view = player_view(&state, PlayerId::One);
    assert!(!view.you.hand.is_empty());
    for card in &view.you.hand {
        let expected = &setup.one.card_names[card.id.0 as usize];
        assert_eq!(card.name.as_ref(), Some(expected));
    }
}
