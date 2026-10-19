//! `player_view` through the public API of the engine (S2-T2 of #52): at every step of whole
//! games, for both seats, the view agrees with the public accessors, with a zone tracker
//! written here from the events only, and with the conservation of cards. The leak tests
//! (which need the identifiers of the piles) live inside the crate, in `view.rs`.

use duel_cards::{
    Op, PickKind, PickRole, PickSlot, Polarity, Selector, Side, Trigger, parse_cards, parse_decks,
};
use duel_engine::{
    Action, CardInstanceId, CardKindView, CardView, EffectLine, Event, GameSetup, GameState,
    OpLine, PlayerId, WinReason, apply, game_setup_from_data, legal_actions, new_game, player_view,
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

/// The normal attack of a plain unit, written out from the stats alone (the tracker never reads
/// the engine): an activated effect of the attack cost, one `attack` of `force` on the opposing
/// hero or unit, a bonus, one target to pick.
fn attack_line(force: u32, attack_cost: u32) -> EffectLine {
    EffectLine {
        index: None,
        trigger: Trigger::Activated,
        cost: Some(attack_cost),
        ops: vec![OpLine {
            op: Op::Attack {
                target: Selector::HeroOrUnit {
                    side: Side::Opposing,
                },
                amount: force,
                ignores_provocation: false,
                scales_with_force: false,
                attackers: 1,
                mode: None,
            },
            polarity: Polarity::Bonus,
        }],
        picks: vec![PickSlot {
            role: PickRole::Target,
            kind: PickKind::HeroOrUnit,
            side: Side::Opposing,
        }],
    }
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
        side.cards[i].creature().unwrap().clone()
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
            kind: CardKindView::Unit,
            immobile: false,
            tags: Vec::new(),
            effects: vec![attack_line(stats.force, stats.attack_cost)],
            unavailable: None,
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
                Event::HeroRiposted { creature, .. } => {
                    // The attacker is destroyed whatever its health.
                    *self.health.get_mut(&creature).unwrap() = 0;
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
        // The tracker knows the cards, not the reason a card cannot be played (a full board
        // under the standard rules): that reason is pinned by the `unavailable` module below.
        let mut hand: Vec<_> = view
            .you
            .hand
            .iter()
            .map(|c| CardView {
                unavailable: None,
                ..c.clone()
            })
            .collect();
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
        let events = apply(&mut state, actions[rng.below(actions.len())].clone()).unwrap();
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
            let action = actions[rng.below(actions.len())].clone();
            assert_eq!(
                apply(&mut watched, action.clone()),
                apply(&mut blind, action)
            );
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

// ---- #171: `unavailable`, the reason a card of the hand cannot be played --------------------

mod unavailable {
    use super::*;
    use duel_cards::{CreatureStats, Hero};
    use duel_engine::{Observation, PlayerSetup, RuleSet, Target};
    use serde_json::Value;

    fn unit(cost: u32, health: u32) -> CreatureStats {
        CreatureStats {
            cost,
            force: 2,
            attack_cost: 1,
            health,
            provocation: false,
            riposte: 0,
        }
    }

    fn hero(initiative: u32, start_budget: u32) -> Hero {
        Hero {
            name: format!("hero-{initiative}"),
            max_time: 30,
            starting_hand: 30,
            initiative,
            start_budget,
            health: 10,
            effects: vec![],
        }
    }

    /// One (active, 20 time units) and Two have their whole deck in hand. `two_health` makes the
    /// hidden hand of Two differ without changing anything public.
    fn setup(rules: RuleSet, two_health: u32) -> GameSetup {
        GameSetup {
            one: PlayerSetup {
                hero: hero(2, 20),
                cards: vec![unit(1, 3); 30].into_iter().map(Into::into).collect(),
                card_names: Vec::new(),
            },
            two: PlayerSetup {
                hero: hero(1, 1),
                cards: vec![unit(1, two_health); 30]
                    .into_iter()
                    .map(Into::into)
                    .collect(),
                card_names: Vec::new(),
            },
            data_versions: Vec::new(),
            rules,
        }
    }

    fn limited(n: u32) -> RuleSet {
        RuleSet {
            board_limit: Some(n),
            ..RuleSet::legacy()
        }
    }

    fn play(state: &mut GameState, n: u32) {
        apply(state, Action::play(CardInstanceId(n))).unwrap();
    }

    fn json(state: &GameState, seat: PlayerId) -> Value {
        serde_json::to_value(player_view(state, seat)).unwrap()
    }

    fn cards(value: &Value) -> Vec<&Value> {
        value.as_array().unwrap().iter().collect()
    }

    /// Whether the key appears anywhere in the document.
    fn mentions(value: &Value, key: &str) -> bool {
        match value {
            Value::Object(map) => map.contains_key(key) || map.values().any(|v| mentions(v, key)),
            Value::Array(items) => items.iter().any(|v| mentions(v, key)),
            _ => false,
        }
    }

    #[test]
    fn every_card_of_the_hand_of_a_seat_with_a_full_board_says_why() {
        let (mut state, _) = new_game(setup(limited(2), 3), 5).unwrap();
        play(&mut state, 0);
        // One place left: nothing is blocked yet.
        assert!(!mentions(&json(&state, PlayerId::One), "unavailable"));
        play(&mut state, 1);
        let view = json(&state, PlayerId::One);
        let hand = cards(&view["you"]["hand"]);
        assert_eq!(hand.len(), 28);
        for card in hand {
            assert_eq!(card["unavailable"], "board_full", "{card}");
        }
    }

    #[test]
    fn the_key_is_last_and_only_there() {
        let (mut state, _) = new_game(setup(limited(1), 3), 5).unwrap();
        play(&mut state, 0);
        let text = serde_json::to_string(&player_view(&state, PlayerId::One)).unwrap();
        assert!(
            text.contains(concat!(
                r#""riposte":0,"kind":"unit","effects":[{"trigger":"activated","cost":1,"#,
                r#""ops":[{"op":"attack","target":{"select":"hero_or_unit","side":"opposing"},"#,
                r#""amount":2,"polarity":"bonus"}],"#,
                r#""picks":[{"role":"target","kind":"hero_or_unit","side":"opposing"}]}],"#,
                r#""unavailable":"board_full"}"#
            )),
            "{text}"
        );
    }

    #[test]
    fn the_other_seat_the_opponent_boards_and_discards_never_have_the_key() {
        let (mut state, _) = new_game(setup(limited(1), 3), 5).unwrap();
        play(&mut state, 0);
        // One is full; Two has an empty board, so its hand is playable, and it never reads the
        // board of One.
        let two = json(&state, PlayerId::Two);
        assert!(!mentions(&two, "unavailable"), "{two}");
        let one = json(&state, PlayerId::One);
        assert!(mentions(&one["you"]["hand"], "unavailable"), "control");
        assert!(!mentions(&one["opponent"], "unavailable"));
        assert!(!mentions(&one["you"]["board"], "unavailable"));
        assert!(!mentions(&one["you"]["discard"], "unavailable"));
        // The same holds with a dead creature in a discard.
        let (mut state, _) = new_game(
            setup(
                RuleSet {
                    hero_riposte: true,
                    board_limit: Some(1),
                },
                3,
            ),
            5,
        )
        .unwrap();
        play(&mut state, 0);
        apply(
            &mut state,
            Action::Attack {
                attacker: CardInstanceId(0),
                target: Target::Hero,
            },
        )
        .unwrap();
        assert_eq!(state.discard(PlayerId::One), vec![CardInstanceId(0)]);
        for seat in SEATS {
            let view = json(&state, seat);
            assert!(!mentions(&view["opponent"], "unavailable"));
            assert!(!mentions(&view["you"]["discard"], "unavailable"));
        }
    }

    #[test]
    fn a_finished_game_blocks_nothing() {
        let (mut state, _) = new_game(setup(limited(1), 3), 5).unwrap();
        play(&mut state, 0);
        assert!(mentions(&json(&state, PlayerId::One), "unavailable"));
        apply(&mut state, Action::Concede).unwrap();
        for seat in SEATS {
            assert!(!mentions(&json(&state, seat), "unavailable"), "{seat:?}");
        }
    }

    #[test]
    fn a_view_without_a_full_board_is_the_view_it_always_was() {
        // A limit that is never reached writes the very same bytes as no limit at all.
        for seed in 0..10 {
            let (mut plain, _) = new_game(setup(RuleSet::legacy(), 3), seed).unwrap();
            let (mut roomy, _) = new_game(setup(limited(30), 3), seed).unwrap();
            for step in 0..6 {
                for seat in SEATS {
                    let (a, b) = (
                        serde_json::to_string(&player_view(&plain, seat)).unwrap(),
                        serde_json::to_string(&player_view(&roomy, seat)).unwrap(),
                    );
                    assert_eq!(a, b, "seed {seed} step {step} {seat:?}");
                    assert!(!a.contains("unavailable"), "{a}");
                }
                if plain.active() != PlayerId::One {
                    break;
                }
                play(&mut plain, step);
                play(&mut roomy, step);
            }
        }
    }

    #[test]
    fn the_reason_never_reads_the_hidden_hand_of_the_other_seat() {
        // Two states that only differ by what Two holds: the view of One is the same, but for
        // the card counts, which are equal here too.
        let (mut a, _) = new_game(setup(limited(1), 1), 5).unwrap();
        let (mut b, _) = new_game(setup(limited(1), 4), 5).unwrap();
        play(&mut a, 0);
        play(&mut b, 0);
        assert!(mentions(&json(&a, PlayerId::One), "unavailable"), "control");
        assert_eq!(json(&a, PlayerId::One), json(&b, PlayerId::One));
    }

    #[test]
    fn the_known_hand_of_an_outcome_never_has_the_key() {
        let (mut state, _) = new_game(setup(limited(1), 3), 5).unwrap();
        play(&mut state, 0);
        let observation = Observation::of(&state).unwrap();
        assert!(!observation.actions().is_empty());
        for i in 0..observation.actions().len() {
            let outcome = observation.outcome(i).unwrap();
            assert!(
                outcome
                    .view_after
                    .you
                    .hand_known
                    .iter()
                    .all(|c| c.unavailable.is_none()),
                "action {i}"
            );
            let text = serde_json::to_string(&outcome.view_after).unwrap();
            assert!(!text.contains("unavailable"), "{text}");
        }
        // Control: the view of the same state does carry the reason.
        assert!(
            observation
                .view()
                .you
                .hand
                .iter()
                .all(|c| c.unavailable == Some("board_full"))
        );
    }
}

// ---- #209: `kind`, `immobile`, `tags`, `effects`, `unavailable` per card ----------------------

mod new_kinds {
    use super::*;
    use duel_engine::RuleSet;
    use serde_json::{Value, json};

    const ATTACK_ON_HERO_OR_UNIT: &str =
        r#"{"op":"attack","target":{"select":"hero_or_unit","side":"opposing"},"amount":3}"#;

    fn hero(initiative: u32, start_budget: u32, starting_hand: u32) -> Value {
        json!({"name": format!("hero-{initiative}"), "max_time": 30, "starting_hand": starting_hand,
               "initiative": initiative, "start_budget": start_budget, "health": 20, "effects": []})
    }

    fn plain(cost: u32, force: u32, attack_cost: u32, health: u32) -> Value {
        json!({"cost": cost, "force": force, "attack_cost": attack_cost, "health": health,
               "provocation": true, "riposte": 1})
    }

    fn unit_with(effects: Value) -> Value {
        let mut card = plain(2, 2, 1, 4);
        card["effects"] = effects;
        card
    }

    fn special_attack(cost: u32, extra: &str) -> Value {
        let op: Value = serde_json::from_str(ATTACK_ON_HERO_OR_UNIT).unwrap();
        let mut op = op;
        if !extra.is_empty() {
            for (k, v) in serde_json::from_str::<Value>(extra)
                .unwrap()
                .as_object()
                .unwrap()
            {
                op[k] = v.clone();
            }
        }
        json!({"trigger": "activated", "cost": cost, "ops": [op]})
    }

    /// A setup from the JSON of the cards of each side (named `c<position>` unless given).
    fn setup_json(one: Vec<Value>, two: Vec<Value>, hands: (u32, u32), budget: u32) -> GameSetup {
        let side = |cards: Vec<Value>, initiative: u32, hand: u32| {
            let names: Vec<String> = (0..cards.len()).map(|n| format!("c{n}")).collect();
            json!({"hero": hero(initiative, budget, hand), "cards": cards, "card_names": names})
        };
        serde_json::from_value(json!({
            "one": side(one, 2, hands.0),
            "two": side(two, 1, hands.1),
            "data_versions": [],
        }))
        .unwrap()
    }

    fn filler(n: usize) -> Vec<Value> {
        (0..n).map(|_| plain(1, 1, 1, 3)).collect()
    }

    /// `first` then plain units up to 30 cards.
    fn deck_of(first: Vec<Value>) -> Vec<Value> {
        let n = 30 - first.len();
        first.into_iter().chain(filler(n)).collect()
    }

    fn view_text(state: &GameState, seat: PlayerId) -> String {
        serde_json::to_string(&player_view(state, seat)).unwrap()
    }

    fn view_json(state: &GameState, seat: PlayerId) -> Value {
        serde_json::from_str(&view_text(state, seat)).unwrap()
    }

    /// The card of the hand of `seat` named `name`.
    fn hand_card(state: &GameState, seat: PlayerId, name: &str) -> Value {
        view_json(state, seat)["you"]["hand"]
            .as_array()
            .unwrap()
            .iter()
            .find(|c| c["name"] == name)
            .unwrap_or_else(|| panic!("{name} is in the hand"))
            .clone()
    }

    fn keys(value: &Value) -> BTreeSet<String> {
        value.as_object().unwrap().keys().cloned().collect()
    }

    fn game(one: Vec<Value>, two: Vec<Value>) -> GameState {
        new_game(setup_json(one, two, (30, 30), 20), 1).unwrap().0
    }

    #[test]
    fn a_plain_unit_view_gains_only_kind_and_effects() {
        let mut card = plain(3, 4, 2, 5);
        card["riposte"] = json!(1);
        let state = game(deck_of(vec![card.clone()]), filler(30));
        for seat in SEATS {
            let json = view_text(&state, seat);
            let value = view_json(&state, seat);
            let own = &value["you"]["hand"];
            for c in own.as_array().unwrap() {
                // Old keys, then the two new ones, in this order.
                assert_eq!(
                    keys(c),
                    BTreeSet::from(
                        [
                            "id",
                            "name",
                            "cost",
                            "force",
                            "attack_cost",
                            "health",
                            "provocation",
                            "riposte",
                            "kind",
                            "effects"
                        ]
                        .map(String::from)
                    ),
                    "{c}"
                );
                assert_eq!(c["kind"], "unit");
            }
            assert!(
                json.contains(r#""riposte":1,"kind":"unit","effects":[{"trigger":"activated""#)
            );
        }
        // The exact JSON of one card: the old part verbatim, then kind and the attack line.
        let c0 = hand_card(&state, PlayerId::One, "c0");
        let id = c0["id"].as_u64().unwrap();
        let expected = format!(
            concat!(
                r#"{{"id":{id},"name":"c0","cost":3,"force":4,"attack_cost":2,"health":5,"#,
                r#""provocation":true,"riposte":1,"kind":"unit","effects":["#,
                r#"{{"trigger":"activated","cost":2,"ops":[{{"op":"attack","target":"#,
                r#"{{"select":"hero_or_unit","side":"opposing"}},"amount":4,"polarity":"bonus"}}],"#,
                r#""picks":[{{"role":"target","kind":"hero_or_unit","side":"opposing"}}]}}]}}"#
            ),
            id = id
        );
        assert!(
            view_text(&state, PlayerId::One).contains(&expected),
            "{expected}"
        );
        assert!(c0.get("index").is_none());
        assert!(c0["effects"][0].get("index").is_none());
    }

    #[test]
    fn the_normal_attack_line_and_a_special_attack_line_have_the_same_shape() {
        let special = special_attack(3, r#"{"amount": 5}"#);
        let state = game(deck_of(vec![unit_with(json!([special]))]), filler(30));
        let card = hand_card(&state, PlayerId::One, "c0");
        let lines = card["effects"].as_array().unwrap();
        assert_eq!(lines.len(), 2, "{card}");
        let (normal, declared) = (&lines[0], &lines[1]);
        assert!(normal.get("index").is_none());
        assert_eq!(declared["index"], 0);
        let mut without_index = keys(declared);
        without_index.remove("index");
        assert_eq!(keys(normal), without_index);
        assert_eq!(keys(&normal["ops"][0]), keys(&declared["ops"][0]));
        assert_eq!(normal["trigger"], declared["trigger"]);
        assert_eq!(normal["ops"][0]["op"], "attack");
        assert_eq!(normal["ops"][0]["target"], declared["ops"][0]["target"]);
        assert_eq!(normal["ops"][0]["polarity"], declared["ops"][0]["polarity"]);
        assert_eq!(normal["picks"], declared["picks"]);
        // Only the values differ.
        assert_eq!(normal["ops"][0]["amount"], 2);
        assert_eq!(declared["ops"][0]["amount"], 5);
        assert_eq!(normal["cost"], 1);
        assert_eq!(declared["cost"], 3);
    }

    #[test]
    fn an_immobile_unit_has_no_attack_line_and_a_spell_has_none() {
        let immobile = json!({"cost": 1, "health": 4, "immobile": true});
        let spell = json!({"kind": "spell", "cost": 1, "effects": [
            {"trigger": "on_play", "ops": [{"op": "draw", "target": {"select": "hero", "side": "own"}, "count": 1}]}]});
        let state = game(deck_of(vec![immobile, spell]), filler(30));
        let immobile = hand_card(&state, PlayerId::One, "c0");
        assert_eq!(immobile["effects"], json!([]), "always written, empty");
        assert_eq!(immobile["force"], 0);
        assert_eq!(immobile["attack_cost"], 0);
        let spell = hand_card(&state, PlayerId::One, "c1");
        let lines = spell["effects"].as_array().unwrap();
        assert_eq!(lines.len(), 1, "only the declared line: {spell}");
        assert_eq!(lines[0]["index"], 0);
        assert_eq!(lines[0]["trigger"], "on_play");
        assert!(
            lines[0].get("cost").is_none(),
            "no cost for an on_play effect"
        );
        assert_eq!(spell["health"], 0);
        assert_eq!(spell["force"], 0);
    }

    #[test]
    fn declared_effects_follow_the_attack_line_with_their_index() {
        let draw = json!({"trigger": "on_play", "ops": [
            {"op": "draw", "target": {"select": "hero", "side": "own"}, "count": 2}]});
        let cont = json!({"trigger": "continuous", "ops": [
            {"op": "modify_stat", "target": {"select": "self"}, "stat": "force", "delta": 1}]});
        let state = game(
            deck_of(vec![unit_with(json!([draw, special_attack(2, ""), cont]))]),
            filler(30),
        );
        let card = hand_card(&state, PlayerId::One, "c0");
        let lines = card["effects"].as_array().unwrap();
        let shape: Vec<(Option<u64>, &str)> = lines
            .iter()
            .map(|l| (l["index"].as_u64(), l["trigger"].as_str().unwrap()))
            .collect();
        assert_eq!(
            shape,
            vec![
                (None, "activated"),
                (Some(0), "on_play"),
                (Some(1), "activated"),
                (Some(2), "continuous")
            ]
        );
        assert_eq!(lines[1]["ops"][0]["count"], 2);
        assert_eq!(lines[3]["ops"][0]["delta"], 1);
        assert!(lines[1].get("cost").is_none());
        assert_eq!(lines[2]["cost"], 2);
        assert!(lines[3].get("cost").is_none());
    }

    #[test]
    fn the_attack_line_reads_effective_force_and_cost() {
        let state = game(
            deck_of(vec![plain(6, 7, 3, 2), plain(1, 1, 5, 9)]),
            filler(30),
        );
        for (name, force, cost) in [("c0", 7, 3), ("c1", 1, 5)] {
            let card = hand_card(&state, PlayerId::One, name);
            assert_eq!(
                (card["force"].as_u64(), card["attack_cost"].as_u64()),
                (Some(force), Some(cost))
            );
            let line = &card["effects"][0];
            assert_eq!(line["ops"][0]["amount"].as_u64(), Some(force), "{card}");
            assert_eq!(line["cost"].as_u64(), Some(cost), "{card}");
        }
    }

    #[test]
    fn kind_immobile_and_tags_are_written_or_omitted_as_specified() {
        let mut immobile = json!({"cost": 1, "health": 4, "immobile": true});
        immobile["tags"] = json!(["bâtiment", "machine"]);
        let mut spell = json!({"kind": "spell", "cost": 2, "effects": [
            {"trigger": "on_play", "ops": [{"op": "draw", "target": {"select": "hero", "side": "own"}, "count": 1}]}]});
        spell["tags"] = json!(["rituel"]);
        let state = game(
            deck_of(vec![immobile, spell, plain(1, 1, 1, 1)]),
            filler(30),
        );
        let text = view_text(&state, PlayerId::One);

        let imm = hand_card(&state, PlayerId::One, "c0");
        assert_eq!(imm["kind"], "unit");
        assert_eq!(imm["immobile"], true);
        assert_eq!(imm["tags"], json!(["bâtiment", "machine"]));
        let sp = hand_card(&state, PlayerId::One, "c1");
        assert_eq!(sp["kind"], "spell");
        assert!(sp.get("immobile").is_none(), "omitted when false: {sp}");
        assert_eq!(sp["tags"], json!(["rituel"]));
        let pl = hand_card(&state, PlayerId::One, "c2");
        assert_eq!(pl["kind"], "unit");
        assert!(pl.get("immobile").is_none());
        assert!(pl.get("tags").is_none(), "omitted when empty: {pl}");
        assert!(pl.get("effects").is_some(), "always written");

        // Order of the keys of the tagged immobile card: riposte, kind, immobile, tags, effects.
        let card = &text[text.find(r#""name":"c0""#).unwrap()..];
        let at = |needle: &str| {
            card.find(needle)
                .unwrap_or_else(|| panic!("{needle} in {card}"))
        };
        let order = [
            at(r#""riposte":0"#),
            at(r#""kind":"unit""#),
            at(r#""immobile":true"#),
            at(r#""tags":["bâtiment","machine"]"#),
            at(r#""effects":[]"#),
        ];
        assert!(order.windows(2).all(|w| w[0] < w[1]), "{card}");
        // And the spell (kind, tags, effects; no immobile key) and the plain unit (kind, effects).
        let spell = &text[text.find(r#""name":"c1""#).unwrap()..];
        assert!(spell.contains(r#""riposte":0,"kind":"spell","tags":["rituel"],"effects":["#));
        let plain = &text[text.find(r#""name":"c2""#).unwrap()..];
        assert!(plain.contains(r#""riposte":1,"kind":"unit","effects":["#));
    }

    #[test]
    fn picks_are_the_expected_picks_of_the_data() {
        let spell = json!({"kind": "spell", "cost": 2, "effects": [
            {"trigger": "on_play", "ops": [{"op": "attack",
                "target": {"select": "hero_or_unit", "side": "opposing"},
                "amount": 3, "attackers": 2, "mode": "simultaneous"}]}]});
        let sweeper = unit_with(json!([
            {"trigger": "on_play", "ops": [{"op": "deal_damage",
                "target": {"select": "all_units", "side": "both"}, "amount": 1}]}]));
        let state = game(deck_of(vec![spell, sweeper]), filler(30));
        let spell = hand_card(&state, PlayerId::One, "c0");
        assert_eq!(
            spell["effects"][0]["picks"],
            json!([
                {"role": "attacker", "kind": "unit", "side": "own"},
                {"role": "attacker", "kind": "unit", "side": "own"},
                {"role": "target", "kind": "hero_or_unit", "side": "opposing"}
            ])
        );
        assert_eq!(spell["effects"][0]["ops"][0]["attackers"], 2);
        assert_eq!(spell["effects"][0]["ops"][0]["mode"], "simultaneous");
        let sweeper = hand_card(&state, PlayerId::One, "c1");
        assert_eq!(
            sweeper["effects"][1]["picks"],
            json!([]),
            "all_units: no pick"
        );
        assert!(sweeper["effects"][1].get("index").is_some());
    }

    #[test]
    fn polarity_is_the_derived_one() {
        let own_draw =
            json!({"op": "draw", "target": {"select": "hero", "side": "own"}, "count": 1});
        let hit_them = json!({"op": "deal_damage", "target": {"select": "hero_or_unit", "side": "opposing"}, "amount": 1});
        let hit_us = json!({"op": "deal_damage", "target": {"select": "all_units", "side": "own"}, "amount": 1});
        let hit_all = json!({"op": "deal_damage", "target": {"select": "all_units", "side": "both"}, "amount": 1});
        let two_effects = json!([
            {"trigger": "on_play", "ops": [own_draw, hit_them, hit_us]},
            {"trigger": "on_play", "ops": [hit_all]}
        ]);
        let state = game(deck_of(vec![unit_with(two_effects)]), filler(30));
        let card = hand_card(&state, PlayerId::One, "c0");
        let polarities = |line: usize| -> Vec<String> {
            card["effects"][line]["ops"]
                .as_array()
                .unwrap()
                .iter()
                .map(|o| o["polarity"].as_str().unwrap().to_owned())
                .collect()
        };
        assert_eq!(polarities(1), ["bonus", "bonus", "malus"]);
        assert_eq!(polarities(2), ["neutral"]);
        // A cost that goes up is a malus for its owner (inverted stats).
        let cost_up = json!([{"trigger": "continuous", "ops": [
            {"op": "modify_stat", "target": {"select": "self"}, "stat": "cost", "delta": 1},
            {"op": "modify_stat", "target": {"select": "self"}, "stat": "force", "delta": -1},
            {"op": "grant_keyword", "target": {"select": "self"}, "keyword": "ranged"}]}]);
        let state = game(deck_of(vec![unit_with(cost_up)]), filler(30));
        let card = hand_card(&state, PlayerId::One, "c0");
        let polarities: Vec<&str> = card["effects"][1]["ops"]
            .as_array()
            .unwrap()
            .iter()
            .map(|o| o["polarity"].as_str().unwrap())
            .collect();
        assert_eq!(polarities, ["malus", "malus", "bonus"]);
    }

    fn limited(state_rules: u32) -> RuleSet {
        RuleSet {
            board_limit: Some(state_rules),
            ..RuleSet::legacy()
        }
    }

    fn spell_card() -> Value {
        json!({"kind": "spell", "cost": 1, "effects": [
            {"trigger": "on_play", "ops": [{"op": "draw", "target": {"select": "hero", "side": "own"}, "count": 1}]}]})
    }

    fn limited_game(limit: u32) -> GameState {
        let mut setup = setup_json(
            deck_of(vec![spell_card(), plain(1, 2, 1, 3)]),
            filler(30),
            (30, 30),
            20,
        );
        setup.rules = limited(limit);
        new_game(setup, 1).unwrap().0
    }

    fn reason(card: &Value) -> Option<&str> {
        card.get("unavailable").map(|v| v.as_str().unwrap())
    }

    #[test]
    fn hand_unavailable_is_per_card() {
        let mut state = limited_game(1);
        // Board not full: the unit has no reason, the spell has the temporary one.
        assert_eq!(reason(&hand_card(&state, PlayerId::One, "c1")), None);
        assert_eq!(
            reason(&hand_card(&state, PlayerId::One, "c0")),
            Some("unsupported_effect")
        );
        // Board full: a unit says board_full, a spell is still not playable for its own reason.
        let unit = hand_card(&state, PlayerId::One, "c1")["id"]
            .as_u64()
            .unwrap();
        apply(
            &mut state,
            Action::play(CardInstanceId(u32::try_from(unit).unwrap())),
        )
        .unwrap();
        assert_eq!(
            state.active(),
            PlayerId::One,
            "still One's turn at this budget"
        );
        for name in ["c2", "c3", "c4"] {
            assert_eq!(
                reason(&hand_card(&state, PlayerId::One, name)),
                Some("board_full"),
                "{name}"
            );
        }
        assert_eq!(
            reason(&hand_card(&state, PlayerId::One, "c0")),
            Some("unsupported_effect")
        );
        // A finished game blocks nothing.
        apply(&mut state, Action::Concede).unwrap();
        for seat in SEATS {
            assert!(!view_text(&state, seat).contains("unavailable"), "{seat:?}");
        }
    }

    #[test]
    fn an_immobile_board_unit_carries_no_unavailable() {
        let immobile = json!({"cost": 1, "health": 4, "immobile": true});
        let mut state = game(deck_of(vec![immobile]), filler(30));
        let id = hand_card(&state, PlayerId::One, "c0")["id"]
            .as_u64()
            .unwrap();
        apply(
            &mut state,
            Action::play(CardInstanceId(u32::try_from(id).unwrap())),
        )
        .unwrap();
        let board = &view_json(&state, PlayerId::One)["you"]["board"];
        assert_eq!(board.as_array().unwrap().len(), 1);
        assert_eq!(reason(&board[0]), None, "{board}");
        assert_eq!(board[0]["effects"], json!([]));
    }

    #[test]
    fn a_plain_board_unit_with_force_has_no_reason() {
        let mut state = game(deck_of(vec![plain(1, 2, 1, 3)]), filler(30));
        let id = hand_card(&state, PlayerId::One, "c0")["id"]
            .as_u64()
            .unwrap();
        apply(
            &mut state,
            Action::play(CardInstanceId(u32::try_from(id).unwrap())),
        )
        .unwrap();
        for seat in SEATS {
            let view = view_json(&state, seat);
            for side in ["you", "opponent"] {
                for card in view[side]["board"].as_array().unwrap() {
                    assert_eq!(reason(card), None, "{seat:?} {side}");
                }
            }
        }
    }

    #[test]
    fn unavailable_is_the_predicate_usable_uses() {
        // A mixed game with a full board of 2: hand cards and board units are checked against the
        // refusal `apply` gives to the action each one stands for.
        let immobile = json!({"cost": 1, "health": 4, "immobile": true});
        let mut setup = setup_json(
            deck_of(vec![
                spell_card(),
                immobile,
                plain(1, 2, 1, 3),
                plain(1, 3, 1, 3),
            ]),
            filler(30),
            (30, 30),
            20,
        );
        setup.rules = limited(2);
        let (mut state, _) = new_game(setup, 1).unwrap();
        for step in 0..3 {
            let view = view_json(&state, PlayerId::One);
            for card in view["you"]["hand"].as_array().unwrap() {
                let id = CardInstanceId(u32::try_from(card["id"].as_u64().unwrap()).unwrap());
                let mut copy = state.clone();
                let refused = apply(&mut copy, Action::play(id)).err().map(|e| e.code());
                assert_eq!(refused, reason(card), "step {step} hand {card}");
            }
            for card in view["you"]["board"].as_array().unwrap() {
                if card["immobile"] == true {
                    assert_eq!(reason(card), None);
                    continue;
                }
                let id = CardInstanceId(u32::try_from(card["id"].as_u64().unwrap()).unwrap());
                let mut copy = state.clone();
                let refused = apply(
                    &mut copy,
                    Action::Attack {
                        attacker: id,
                        target: duel_engine::Target::Hero,
                    },
                )
                .err()
                .map(|e| e.code());
                assert_eq!(refused, reason(card), "step {step} board {card}");
            }
            // Play the first playable card to grow the board.
            let next = legal_actions(&state)
                .into_iter()
                .find(|a| matches!(a, Action::Play { .. }));
            let Some(next) = next else { break };
            apply(&mut state, next).unwrap();
        }
        assert!(
            !state.board(PlayerId::One).is_empty(),
            "the check ran on a real board"
        );
    }

    // ---- no leak ---------------------------------------------------------------------------

    /// The deck of a seat: every card carries the tag `<prefix>-<instance id>` (the id is the
    /// position in the setup, offset 0 for One and 30 for Two), and the spells a unique amount.
    fn tagged(prefix: &str, offset: usize, amount: u32) -> Vec<Value> {
        (0..30)
            .map(|n| {
                let tag = json!([format!("{prefix}-{}", offset + n)]);
                match n % 4 {
                    0 => json!({"kind": "spell", "cost": 2, "tags": tag, "effects": [
                        {"trigger": "on_play", "ops": [{"op": "attack",
                            "target": {"select": "hero_or_unit", "side": "opposing"},
                            "amount": amount}]}]}),
                    1 => json!({"cost": 1, "health": 4, "immobile": true, "tags": tag}),
                    2 => {
                        let mut c = unit_with(json!([special_attack(2, "")]));
                        c["tags"] = tag;
                        c
                    }
                    _ => {
                        let mut c = plain(1, 2, 1, 3);
                        c["tags"] = tag;
                        c
                    }
                }
            })
            .collect()
    }

    fn secret_ids(text: &str, prefix: &str) -> BTreeSet<u32> {
        text.split(&format!("{prefix}-"))
            .skip(1)
            .map(|rest| {
                rest.chars()
                    .take_while(char::is_ascii_digit)
                    .collect::<String>()
                    .parse()
                    .unwrap()
            })
            .collect()
    }

    fn ids(cards: Vec<CardInstanceId>) -> BTreeSet<u32> {
        cards.into_iter().map(|c| c.0).collect()
    }

    /// One summons a card (the budget of 1 hands the token over), then Two summons one.
    fn played_game(two_tags: &str) -> GameState {
        let setup = setup_json(
            tagged("PUBLIC", 0, 4001),
            tagged(two_tags, 30, 4242),
            (30, 30),
            1,
        );
        let (mut state, _) = new_game(setup, 5).unwrap();
        assert_eq!(state.active(), PlayerId::One);
        for _ in 0..2 {
            let play = legal_actions(&state)
                .into_iter()
                .find(|a| matches!(a, Action::Play { .. }))
                .expect("a summonable card");
            apply(&mut state, play).unwrap();
        }
        state
    }

    #[test]
    fn a_secret_tag_never_reaches_the_other_seat() {
        let state = played_game("SECRET");
        let public: BTreeSet<u32> =
            ids([state.board(PlayerId::Two), state.discard(PlayerId::Two)].concat());
        assert!(!public.is_empty(), "Two has a public card: {public:?}");
        let one = view_text(&state, PlayerId::One);
        // Only the tags of the public cards of Two (board, discard) reach One.
        assert_eq!(secret_ids(&one, "SECRET"), public, "{one}");
        // The unique amount of the spells of Two lives in its hand only.
        assert!(!one.contains("4242"), "{one}");
        // Positive witness: Two sees its own hand, board and discard.
        let two = view_text(&state, PlayerId::Two);
        let own: BTreeSet<u32> = ids([
            state.hand(PlayerId::Two),
            state.board(PlayerId::Two),
            state.discard(PlayerId::Two),
        ]
        .concat());
        assert_eq!(secret_ids(&two, "SECRET"), own);
        assert!(two.contains("4242"), "a spell of Two is in its hand");
        // And the pile never: no tag of a card in the draw pile appears in either view.
        let piles: BTreeSet<u32> = (30..60).filter(|id| !own.contains(id)).collect();
        for text in [&one, &two] {
            assert!(secret_ids(text, "SECRET").is_disjoint(&piles));
        }
        // The other way round, One's tags are public to Two as the board and discard of One.
        let one_public: BTreeSet<u32> =
            ids([state.board(PlayerId::One), state.discard(PlayerId::One)].concat());
        assert_eq!(
            secret_ids(&two, "PUBLIC"),
            one_public,
            "Two sees the tags of the public cards of One only"
        );
    }

    #[test]
    fn view_of_the_other_seat_is_independent_of_the_hidden_hand_tags() {
        // Two setups that only differ by the tags of the hand of Two (and the spells' amount).
        let (a, b) = (played_game("SECRET"), played_game("OTHER"));
        assert_eq!(a.board(PlayerId::Two), b.board(PlayerId::Two));
        let (va, vb) = (view_text(&a, PlayerId::One), view_text(&b, PlayerId::One));
        // The tags of the two public cards differ by name, the rest is the same byte for byte.
        let normalise = |t: &str, prefix: &str| t.replace(&format!("{prefix}-"), "T-");
        assert_eq!(normalise(&va, "SECRET"), normalise(&vb, "OTHER"));
        // Control: the view of Two does depend on its own tags.
        assert_ne!(view_text(&a, PlayerId::Two), view_text(&b, PlayerId::Two));
    }
}
