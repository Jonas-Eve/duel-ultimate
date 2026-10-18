//! Scenario builders for the greedy policy tests: small hand-made games, no useful chance.
//!
//! Every deck has exactly 30 cards (an engine rule). A scenario deck is the listed cards plus
//! "pad" cards, so that `starting_hand = 30` puts the whole deck in the hand: the hand is then
//! fully known and independent of the shuffle. A pad is poison to use: it costs far more than any
//! budget (playing it hands the turn over, which is how a scenario passes the token) and its
//! attack hands over too.
#![allow(dead_code)]

use duel_engine::{
    Action, CardInstanceId, GameSetup, GameState, PlayerId, Target, apply, new_game, player_view,
};
use serde_json::{Value, json};

/// A creature of a scenario: `(name, cost, force, attack_cost, health, provocation, riposte)`.
pub struct Card {
    pub name: &'static str,
    pub cost: u32,
    pub force: u32,
    pub attack_cost: u32,
    pub health: u32,
    pub provocation: bool,
    pub riposte: u32,
}

pub fn card(name: &'static str, cost: u32, force: u32, health: u32) -> Card {
    Card {
        name,
        cost,
        force,
        attack_cost: 1,
        health,
        provocation: false,
        riposte: 0,
    }
}

impl Card {
    pub fn attack_cost(mut self, attack_cost: u32) -> Card {
        self.attack_cost = attack_cost;
        self
    }
    pub fn provocation(mut self) -> Card {
        self.provocation = true;
        self
    }
    pub fn riposte(mut self, riposte: u32) -> Card {
        self.riposte = riposte;
        self
    }
}

pub fn pad() -> Card {
    card("pad", 9000, 1, 1).attack_cost(9000)
}

/// A hero: initiative, start budget, max time, health, effects as `(cost, draw count)`.
pub struct Hero {
    pub initiative: u32,
    pub start_budget: u32,
    pub health: u32,
    pub effects: Vec<(u32, u32)>,
}

impl Hero {
    /// Hero One: starts, plenty of time.
    pub fn first(health: u32) -> Hero {
        Hero {
            initiative: 5,
            start_budget: 8,
            health,
            effects: vec![],
        }
    }
    /// Hero Two.
    pub fn second(health: u32) -> Hero {
        Hero {
            initiative: 1,
            start_budget: 8,
            health,
            effects: vec![],
        }
    }
}

fn card_json(c: &Card) -> Value {
    json!({
        "cost": c.cost, "force": c.force, "attack_cost": c.attack_cost,
        "health": c.health, "provocation": c.provocation, "riposte": c.riposte
    })
}

pub fn side(hero: &Hero, cards: &[Card], starting_hand: usize) -> Value {
    let mut deck: Vec<&Card> = cards.iter().collect();
    let filler = pad();
    while deck.len() < 30 {
        deck.push(&filler);
    }
    let effects: Vec<Value> = hero
        .effects
        .iter()
        .enumerate()
        .map(|(i, (cost, count))| {
            json!({"name": format!("effect-{i}"), "cost": cost, "effect": {"kind": "draw", "count": count}})
        })
        .collect();
    json!({
        "hero": {
            "name": "hero", "max_time": 10, "starting_hand": starting_hand,
            "initiative": hero.initiative, "start_budget": hero.start_budget,
            "health": hero.health, "effects": effects
        },
        "cards": deck.iter().map(|c| card_json(c)).collect::<Vec<_>>(),
        "card_names": deck.iter().map(|c| c.name).collect::<Vec<_>>(),
    })
}

/// A game whose whole decks are in the hands (`starting_hand = 30`), under the engine default
/// rules.
pub fn scenario(one: (Hero, Vec<Card>), two: (Hero, Vec<Card>)) -> GameState {
    let setup: GameSetup = serde_json::from_value(json!({
        "one": side(&one.0, &one.1, 30),
        "two": side(&two.0, &two.1, 30),
        "data_versions": [],
    }))
    .expect("a valid scenario setup");
    let (state, _) = new_game(setup, 1).expect("a valid scenario game");
    state
}

/// [`scenario`] with the rules written out (`{"hero_riposte": ..., "board_limit": ...}`).
pub fn scenario_with_rules(
    one: (Hero, Vec<Card>),
    two: (Hero, Vec<Card>),
    rules: Value,
) -> GameState {
    let setup: GameSetup = serde_json::from_value(json!({
        "one": side(&one.0, &one.1, 30),
        "two": side(&two.0, &two.1, 30),
        "data_versions": [],
        "rules": rules,
    }))
    .expect("a valid scenario setup");
    let (state, _) = new_game(setup, 1).expect("a valid scenario game");
    state
}

/// A game where a side is `copies` of one card with a hand of `starting_hand` of them.
pub fn clones(hero: &Hero, card: &Card, starting_hand: usize) -> Value {
    let mut value = side(hero, &[], starting_hand);
    value["cards"] = json!((0..30).map(|_| card_json(card)).collect::<Vec<_>>());
    value["card_names"] = json!((0..30).map(|_| card.name).collect::<Vec<_>>());
    value
}

pub fn game_from(one: Value, two: Value) -> GameState {
    let setup: GameSetup = serde_json::from_value(json!({
        "one": one, "two": two, "data_versions": [],
    }))
    .expect("a valid scenario setup");
    new_game(setup, 1).expect("a valid scenario game").0
}

/// The id of the first card called `name` in the hand of `seat`.
pub fn in_hand(state: &GameState, seat: PlayerId, name: &str) -> CardInstanceId {
    player_view(state, seat)
        .you
        .hand
        .iter()
        .find(|c| c.name.as_deref() == Some(name))
        .unwrap_or_else(|| panic!("no {name} in hand"))
        .id
}

/// The id of a card called `name` on the board of `seat` (own board seen from `seat`).
pub fn on_board(state: &GameState, seat: PlayerId, name: &str) -> CardInstanceId {
    player_view(state, seat)
        .you
        .board
        .iter()
        .find(|c| c.name.as_deref() == Some(name))
        .unwrap_or_else(|| panic!("no {name} on board"))
        .id
}

/// The active seat plays the card called `name` from its hand.
pub fn play(state: &mut GameState, name: &str) {
    let seat = state.active();
    let card = in_hand(state, seat, name);
    apply(state, Action::Play { card }).expect("a legal play");
}

/// The active seat hands the token over by playing a pad.
pub fn pass(state: &mut GameState) {
    let seat = state.active();
    play(state, "pad");
    assert_ne!(state.active(), seat, "a pad hands the token over");
}

pub fn attack(state: &mut GameState, attacker: CardInstanceId, target: Target) {
    apply(state, Action::Attack { attacker, target }).expect("a legal attack");
}

/// The standard opening: One plays `mine`, passes; Two plays `theirs`, passes. One is to move.
pub fn opening(
    one: (Hero, Vec<Card>),
    two: (Hero, Vec<Card>),
    mine: &[&str],
    theirs: &[&str],
) -> GameState {
    opened(scenario(one, two), mine, theirs)
}

fn opened(mut state: GameState, mine: &[&str], theirs: &[&str]) -> GameState {
    assert_eq!(state.active(), PlayerId::One);
    for name in mine {
        play(&mut state, name);
    }
    pass(&mut state);
    for name in theirs {
        play(&mut state, name);
    }
    pass(&mut state);
    assert_eq!(state.active(), PlayerId::One);
    state
}
