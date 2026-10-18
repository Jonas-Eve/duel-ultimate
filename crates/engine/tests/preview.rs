//! `preview_legal_actions` through the public API (T3 of #52): for every legal action, what it
//! costs and what it causes, obtained by a dry run on a clone.
//!
//! The differential test compares the preview with the observable change of a real copy of the
//! state (public accessors), not with the events the preview itself reads; the boundary table
//! uses values computed by hand on built states.

use duel_cards::{CreatureStats, Effect, Hero, HeroEffect};
use duel_engine::{
    Action, ActionPreview, Event, GameResult, GameSetup, GameState, PlayerId, PlayerSetup, RuleSet,
    Target, WinReason, apply, game_setup_from_data, legal_actions, new_game, preview_legal_actions,
};
use serde_json::Value;

const FIXTURE_CARDS: &str = include_str!("../../../data/fixture/1.0.0/cards.json");
const FIXTURE_DECKS: &str = include_str!("../../../data/fixture/1.0.0/decks.json");
const STARTER_CARDS: &str = include_str!("../../../data/starter/1.0.0/cards.json");
const STARTER_DECKS: &str = include_str!("../../../data/starter/1.0.0/decks.json");

struct Xorshift(u64);

impl Xorshift {
    fn below(&mut self, bound: usize) -> usize {
        self.0 ^= self.0 << 13;
        self.0 ^= self.0 >> 7;
        self.0 ^= self.0 << 17;
        usize::try_from(self.0 % u64::try_from(bound).unwrap()).unwrap()
    }
}

fn matchups() -> Vec<GameSetup> {
    let of = |cards: &str, decks: &str, one: &str, two: &str| {
        game_setup_from_data(cards, decks, one, two, vec!["test@1".into()]).unwrap()
    };
    let s = (STARTER_CARDS, STARTER_DECKS);
    vec![
        of(s.0, s.1, "rush", "bulwark"),
        of(s.0, s.1, "bulwark", "rush"),
        of(s.0, s.1, "rush", "rush"),
        of(s.0, s.1, "bulwark", "bulwark"),
        of(FIXTURE_CARDS, FIXTURE_DECKS, "aggro", "control"),
    ]
}

fn result_of(state: &GameState) -> Option<GameResult> {
    state
        .winner()
        .map(|(winner, reason)| GameResult { winner, reason })
}

// ---------------------------------------------------------------------------------------
// 1. Differential: the preview against the observable change of a real copy
// ---------------------------------------------------------------------------------------

/// The cost the events of a real application report for `action`, found by variant.
fn event_cost(events: &[Event]) -> Option<u32> {
    events.iter().find_map(|e| match e {
        Event::Played { cost, .. }
        | Event::EffectActivated { cost, .. }
        | Event::Attacked { cost, .. } => Some(*cost),
        _ => None,
    })
}

fn check_step(state: &GameState) {
    let before = state.clone();
    let legal = legal_actions(state);
    let previews = preview_legal_actions(state);
    assert_eq!(*state, before, "the preview must leave the state intact");
    assert_eq!(
        previews
            .iter()
            .map(|p| p.action.clone())
            .collect::<Vec<_>>(),
        legal,
        "same actions, same order as legal_actions"
    );
    for preview in &previews {
        let mut copy = state.clone();
        let events = apply(&mut copy, preview.action.clone()).unwrap();
        assert_eq!(
            preview.hands_over,
            copy.active() != state.active(),
            "{preview:?}"
        );
        assert_eq!(preview.ends_game, result_of(&copy), "{preview:?}");
        match preview.budget_after {
            Some(budget) => assert_eq!(budget, copy.budget(), "{preview:?}"),
            None => assert!(
                matches!(
                    preview.ends_game,
                    Some(GameResult {
                        reason: WinReason::HeroDown | WinReason::Concede,
                        ..
                    })
                ),
                "no budget only when the game ends without a token move: {preview:?}"
            ),
        }
        assert_eq!(preview.cost, event_cost(&events), "{preview:?}");
    }
}

#[test]
fn the_preview_matches_a_real_application_at_every_step_of_whole_games() {
    let mut steps = 0_usize;
    for (m, setup) in matchups().into_iter().enumerate() {
        for seed in 0..100_u64 {
            let (mut state, _) = new_game(setup.clone(), seed).unwrap();
            let mut rng = Xorshift(seed.wrapping_mul(0x9E37_79B9_7F4A_7C15) ^ (m as u64 + 1));
            for _ in 0..400 {
                check_step(&state);
                steps += 1;
                // The walk never concedes (it would end every game at once); Concede is still
                // previewed and checked at each step above.
                let actions: Vec<Action> = legal_actions(&state)
                    .into_iter()
                    .filter(|a| *a != Action::Concede)
                    .collect();
                if actions.is_empty() {
                    break;
                }
                apply(&mut state, actions[rng.below(actions.len())].clone()).unwrap();
            }
        }
    }
    assert!(steps > 5_000, "the walk must be long enough: {steps}");
}

#[test]
fn a_finished_game_has_no_preview() {
    let (mut state, _) = new_game(matchups().remove(0), 1).unwrap();
    apply(&mut state, Action::Concede).unwrap();
    assert_eq!(preview_legal_actions(&state), vec![]);
}

// ---------------------------------------------------------------------------------------
// 2. Boundary table on built states (expected values computed by hand)
// ---------------------------------------------------------------------------------------

fn creature(cost: u32, force: u32, attack_cost: u32, health: u32, riposte: u32) -> CreatureStats {
    CreatureStats {
        cost,
        force,
        attack_cost,
        health,
        provocation: false,
        riposte,
    }
}

fn draw(cost: u32, count: u32) -> HeroEffect {
    HeroEffect {
        name: format!("draw {count}"),
        cost,
        effect: Effect::Draw { count },
    }
}

fn hero(
    initiative: u32,
    start_budget: u32,
    max_time: u32,
    starting_hand: u32,
    health: u32,
    effects: Vec<HeroEffect>,
) -> Hero {
    Hero {
        name: format!("hero-{initiative}"),
        max_time,
        starting_hand,
        initiative,
        start_budget,
        health,
        effects,
    }
}

fn side(hero: Hero, card: CreatureStats) -> PlayerSetup {
    PlayerSetup {
        hero,
        cards: vec![card; 30].into_iter().map(Into::into).collect(),
        card_names: Vec::new(),
    }
}

/// One starts (initiative 2) with `start_budget` 3 and `max_time` 8; Two has `max_time` 8 too
/// unless overridden by the caller on the returned setup. Every card of a side is identical.
fn built(one: PlayerSetup, two: PlayerSetup) -> GameState {
    new_game(
        GameSetup {
            one,
            two,
            data_versions: vec![],
            rules: RuleSet::default(),
        },
        7,
    )
    .unwrap()
    .0
}

fn one_hero(start_budget: u32, max_time: u32, effects: Vec<HeroEffect>) -> Hero {
    hero(2, start_budget, max_time, 4, 10, effects)
}

fn two_hero(max_time: u32) -> Hero {
    hero(1, 1, max_time, 4, 10, vec![draw(1, 1)])
}

fn preview_of(state: &GameState, action: Action) -> ActionPreview {
    preview_legal_actions(state)
        .into_iter()
        .find(|p| p.action == action)
        .unwrap_or_else(|| panic!("{action:?} is not legal"))
}

fn first_play(state: &GameState) -> Action {
    legal_actions(state)
        .into_iter()
        .find(|a| matches!(a, Action::Play { .. }))
        .unwrap()
}

fn expect(
    action: Action,
    cost: Option<u32>,
    hands_over: bool,
    budget_after: Option<u32>,
    ends_game: Option<GameResult>,
) -> ActionPreview {
    ActionPreview {
        action,
        cost,
        hands_over,
        budget_after,
        ends_game,
        attacker_destroyed: false,
    }
}

fn won(winner: PlayerId, reason: WinReason) -> Option<GameResult> {
    Some(GameResult { winner, reason })
}

/// A summoning of cost `cost` by One (budget 3, max_time 8) against a Two of `two_max`.
fn summoning(cost: u32, two_max: u32) -> ActionPreview {
    let state = built(
        side(one_hero(3, 8, vec![draw(1, 1)]), creature(cost, 1, 1, 3, 0)),
        side(two_hero(two_max), creature(1, 1, 1, 3, 0)),
    );
    let play = first_play(&state);
    let preview = preview_of(&state, play.clone());
    assert_eq!(preview.action, play);
    preview
}

#[test]
fn a_cost_below_the_budget_stays_with_what_is_left() {
    // b = 3, c = 2 = b - 1: One keeps the token with 1.
    let p = summoning(2, 8);
    assert_eq!(p, expect(p.action.clone(), Some(2), false, Some(1), None));
}

#[test]
fn a_cost_equal_to_the_budget_hands_over_and_the_receiver_gets_one() {
    // b = 3, c = 3: Two receives 3 - 3 + 1 = 1.
    let p = summoning(3, 8);
    assert_eq!(p, expect(p.action.clone(), Some(3), true, Some(1), None));
}

#[test]
fn a_cost_above_the_budget_gives_the_excess_plus_one() {
    // b = 3, c = 5: Two receives 5 - 3 + 1 = 3.
    let p = summoning(5, 8);
    assert_eq!(p, expect(p.action.clone(), Some(5), true, Some(3), None));
}

#[test]
fn the_receivers_max_time_caps_the_budget_after() {
    // b = 3, c = 9: 9 - 3 + 1 = 7, capped by Two's max_time 5 (One's own is 8).
    let p = summoning(9, 5);
    assert_eq!(p, expect(p.action.clone(), Some(9), true, Some(5), None));
}

#[test]
fn a_cost_above_the_senders_own_max_time_is_not_capped_by_it() {
    // One: max_time 4, budget 3, cost 7: Two (max_time 10) receives 7 - 3 + 1 = 5 > 4.
    let state = built(
        side(one_hero(3, 4, vec![draw(1, 1)]), creature(7, 1, 1, 3, 0)),
        side(two_hero(10), creature(1, 1, 1, 3, 0)),
    );
    let p = preview_of(&state, first_play(&state));
    assert_eq!(p, expect(p.action.clone(), Some(7), true, Some(5), None));
}

#[test]
fn hero_effects_follow_the_same_rule_with_their_own_cost() {
    // b = 3, Two max_time 5: costs 1, 3, 5 and 9 give stay(2), hand(1), hand(3), hand(5).
    let state = built(
        side(
            one_hero(3, 8, vec![draw(1, 1), draw(3, 1), draw(5, 1), draw(9, 1)]),
            creature(4, 1, 1, 3, 0),
        ),
        side(two_hero(5), creature(1, 1, 1, 3, 0)),
    );
    let a = |effect| Action::activate(effect);
    assert_eq!(
        preview_of(&state, a(0)),
        expect(a(0), Some(1), false, Some(2), None)
    );
    assert_eq!(
        preview_of(&state, a(1)),
        expect(a(1), Some(3), true, Some(1), None)
    );
    assert_eq!(
        preview_of(&state, a(2)),
        expect(a(2), Some(5), true, Some(3), None)
    );
    assert_eq!(
        preview_of(&state, a(3)),
        expect(a(3), Some(9), true, Some(5), None)
    );
}

/// One summons a creature (cost 1, budget 5 -> 4) whose attack costs 7; Two's hero has
/// `two_health` health, so a force of 1 is lethal only at 1.
fn attacking_state(two_health: u32) -> (GameState, Action) {
    attacking_state_with(RuleSet::default(), two_health)
}

fn attacking_state_with(rules: RuleSet, two_health: u32) -> (GameState, Action) {
    let mut state = built_with(
        rules,
        side(one_hero(5, 8, vec![draw(1, 1)]), creature(1, 1, 7, 3, 0)),
        side(
            hero(1, 1, 8, 4, two_health, vec![draw(1, 1)]),
            creature(1, 1, 1, 3, 0),
        ),
    );
    let Action::Play { card, .. } = first_play(&state) else {
        unreachable!()
    };
    apply(&mut state, Action::play(card)).unwrap();
    assert_eq!((state.active(), state.budget()), (PlayerId::One, 4));
    (
        state,
        Action::Attack {
            attacker: card,
            target: Target::Hero,
        },
    )
}

#[test]
fn a_lethal_attack_on_the_hero_spends_nothing_and_does_not_hand_over() {
    // Displayed cost 7 (the attack cost), but no time is spent: no token move, hero_down.
    let (state, attack) = attacking_state(1);
    assert_eq!(
        preview_of(&state, attack.clone()),
        expect(
            attack,
            Some(7),
            false,
            None,
            won(PlayerId::One, WinReason::HeroDown)
        )
    );
}

#[test]
fn a_non_lethal_attack_on_the_hero_pays_and_hands_over() {
    // Same attack against a hero with 2 health: b = 4, c = 7 -> Two receives 7 - 4 + 1 = 4.
    // With the options off the attacker lives; under the default (standard) rules the hero
    // hits back and the preview says so.
    let (state, attack) = attacking_state_with(RuleSet::legacy(), 2);
    assert_eq!(
        preview_of(&state, attack.clone()),
        expect(attack, Some(7), true, Some(4), None)
    );
    let (state, attack) = attacking_state(2);
    assert_eq!(
        preview_of(&state, attack.clone()),
        ActionPreview {
            attacker_destroyed: true,
            ..expect(attack, Some(7), true, Some(4), None)
        }
    );
}

#[test]
fn a_handover_to_a_player_with_nothing_to_do_ends_the_game_by_exhaustion() {
    // Two has no effect and an empty hand; One's summoning costs 10 > 3: Two receives
    // min(10 - 3 + 1, 5) = 5 and is exhausted at once.
    let state = built(
        side(one_hero(3, 8, vec![draw(1, 1)]), creature(10, 1, 1, 3, 0)),
        side(hero(1, 1, 5, 0, 10, vec![]), creature(1, 1, 1, 3, 0)),
    );
    let play = first_play(&state);
    assert_eq!(
        preview_of(&state, play.clone()),
        expect(
            play,
            Some(10),
            true,
            Some(5),
            won(PlayerId::One, WinReason::Exhaustion)
        )
    );
}

#[test]
fn an_attacker_killed_by_the_riposte_still_pays_its_cost() {
    // One's creatures: cost 3, attack cost 2, health 1. Two's: cost 1, health 5, riposte 2.
    // One summons (3 = b: Two receives 1), Two summons (1 = b: One receives 1). One's attack
    // on that creature costs 2 > 1: Two receives 2 - 1 + 1 = 2, whether or not the attacker
    // dies of the riposte (it does: 1 health against 2).
    let mut state = built(
        side(one_hero(3, 8, vec![draw(1, 1)]), creature(3, 1, 2, 1, 0)),
        side(two_hero(8), creature(1, 1, 1, 5, 2)),
    );
    let play = first_play(&state);
    apply(&mut state, play).unwrap();
    assert_eq!(state.active(), PlayerId::Two);
    let play = first_play(&state);
    apply(&mut state, play).unwrap();
    assert_eq!((state.active(), state.budget()), (PlayerId::One, 1));
    let attacker = state.board(PlayerId::One)[0];
    let target = state.board(PlayerId::Two)[0];
    let attack = Action::Attack {
        attacker,
        target: Target::Creature(target),
    };
    assert_eq!(
        preview_of(&state, attack.clone()),
        ActionPreview {
            attacker_destroyed: true,
            ..expect(attack.clone(), Some(2), true, Some(2), None)
        }
    );
    // Control through the real application: the attacker did die.
    let mut copy = state.clone();
    apply(&mut copy, attack).unwrap();
    assert_eq!(copy.board(PlayerId::One), vec![]);
}

#[test]
fn conceding_costs_nothing_and_the_opponent_wins() {
    let (state, _) = attacking_state(1);
    assert_eq!(
        preview_of(&state, Action::Concede),
        expect(
            Action::Concede,
            None,
            false,
            None,
            won(PlayerId::Two, WinReason::Concede)
        )
    );
}

// ---------------------------------------------------------------------------------------
// 3. No leak of the hidden information
// ---------------------------------------------------------------------------------------

fn json_of(state: &GameState) -> Value {
    serde_json::to_value(preview_legal_actions(state)).unwrap()
}

#[test]
fn an_entry_has_exactly_six_fields_and_no_draw_appears() {
    for setup in matchups() {
        for seed in 0..20 {
            let (state, _) = new_game(setup.clone(), seed).unwrap();
            let value = json_of(&state);
            for entry in value.as_array().unwrap() {
                let mut keys: Vec<&str> = entry
                    .as_object()
                    .unwrap()
                    .keys()
                    .map(String::as_str)
                    .collect();
                keys.sort_unstable();
                assert_eq!(
                    keys,
                    [
                        "action",
                        "attacker_destroyed",
                        "budget_after",
                        "cost",
                        "ends_game",
                        "hands_over"
                    ]
                );
            }
            assert!(!value.to_string().contains("drew"), "{value}");
        }
    }
}

#[test]
fn drawing_several_cards_changes_only_the_cost_and_the_handover() {
    // Two heroes with the same effect cost and different counts (1 against 5): identical
    // previews, so nothing of what is drawn can be read.
    let state_with = |count| {
        built(
            side(
                one_hero(3, 8, vec![draw(2, count)]),
                creature(4, 1, 1, 3, 0),
            ),
            side(two_hero(8), creature(1, 1, 1, 3, 0)),
        )
    };
    let previews = |count| {
        let state = state_with(count);
        preview_legal_actions(&state)
            .into_iter()
            .filter(|p| matches!(p.action, Action::Activate { .. }))
            .collect::<Vec<_>>()
    };
    assert_eq!(previews(1), previews(5));
    assert_eq!(previews(1).len(), 1);
}

// ---------------------------------------------------------------------------------------
// 4. Purity: no effect on the state or on its generator
// ---------------------------------------------------------------------------------------

#[test]
fn previewing_does_not_change_how_the_game_continues() {
    for (m, setup) in matchups().into_iter().enumerate() {
        for seed in 0..10_u64 {
            let (mut plain, _) = new_game(setup.clone(), seed).unwrap();
            let (mut observed, _) = new_game(setup.clone(), seed).unwrap();
            let mut rng = Xorshift(seed.wrapping_mul(0xD134_2543_DE82_EF95) ^ (m as u64 + 3));
            for _ in 0..200 {
                let before = observed.clone();
                let _ = preview_legal_actions(&observed);
                assert_eq!(observed, before, "state and generator intact");
                let actions: Vec<Action> = legal_actions(&plain)
                    .into_iter()
                    .filter(|a| *a != Action::Concede)
                    .collect();
                if actions.is_empty() {
                    break;
                }
                let action = actions[rng.below(actions.len())].clone();
                let a = serde_json::to_string(&apply(&mut plain, action.clone()).unwrap()).unwrap();
                let b = serde_json::to_string(&apply(&mut observed, action).unwrap()).unwrap();
                assert_eq!(a, b, "same events byte for byte");
            }
            assert_eq!(plain, observed);
        }
    }
}

// ---------------------------------------------------------------------------------------
// 5. `attacker_destroyed` (#170): read from the facts of the dry run (a `Died` of the attacker)
// ---------------------------------------------------------------------------------------

const HERO_RIPOSTE: RuleSet = RuleSet {
    hero_riposte: true,
    ..RuleSet::legacy()
};

/// `built` with explicit rules.
fn built_with(rules: RuleSet, one: PlayerSetup, two: PlayerSetup) -> GameState {
    new_game(
        GameSetup {
            one,
            two,
            data_versions: vec![],
            rules,
        },
        7,
    )
    .unwrap()
    .0
}

/// The `attacker_destroyed` key of the JSON of the preview of `action`: the contract the
/// clients read (a boolean, always present).
fn destroyed(state: &GameState, action: Action) -> bool {
    let value = serde_json::to_value(preview_of(state, action)).unwrap();
    value["attacker_destroyed"]
        .as_bool()
        .unwrap_or_else(|| panic!("no boolean attacker_destroyed in {value}"))
}

/// One summons a creature (cost 1, force 1, attack cost 1, health 3) against a hero of
/// `two_health`, budget 5 -> 4, under `rules`.
fn hitting_state(rules: RuleSet, two_health: u32) -> (GameState, Action) {
    let mut state = built_with(
        rules,
        side(one_hero(5, 8, vec![draw(1, 1)]), creature(1, 1, 1, 3, 0)),
        side(
            hero(1, 1, 8, 4, two_health, vec![draw(1, 1)]),
            creature(1, 1, 1, 3, 0),
        ),
    );
    let Action::Play { card, .. } = first_play(&state) else {
        unreachable!()
    };
    apply(&mut state, Action::play(card)).unwrap();
    (
        state,
        Action::Attack {
            attacker: card,
            target: Target::Hero,
        },
    )
}

#[test]
fn a_non_lethal_hit_on_the_hero_destroys_the_attacker_when_the_option_is_on() {
    let (state, attack) = hitting_state(HERO_RIPOSTE, 2);
    assert!(destroyed(&state, attack));
}

#[test]
fn a_lethal_hit_on_the_hero_destroys_nothing() {
    let (state, attack) = hitting_state(HERO_RIPOSTE, 1);
    assert!(!destroyed(&state, attack));
}

#[test]
fn with_the_option_off_a_hit_on_the_hero_destroys_nothing() {
    let (state, attack) = hitting_state(RuleSet::legacy(), 2);
    assert!(!destroyed(&state, attack));
}

/// One's creature (cost 3, force 1, attack cost 2, `health`) attacks Two's (health 5, riposte
/// 2) with One holding 1 time unit.
fn creature_duel(health: u32) -> (GameState, Action) {
    let mut state = built(
        side(
            one_hero(3, 8, vec![draw(1, 1)]),
            creature(3, 1, 2, health, 0),
        ),
        side(two_hero(8), creature(1, 1, 1, 5, 2)),
    );
    for _ in 0..2 {
        let play = first_play(&state);
        apply(&mut state, play).unwrap();
    }
    let attack = Action::Attack {
        attacker: state.board(PlayerId::One)[0],
        target: Target::Creature(state.board(PlayerId::Two)[0]),
    };
    (state, attack)
}

#[test]
fn an_attacker_killed_by_the_riposte_of_a_creature_is_destroyed() {
    let (state, attack) = creature_duel(1);
    assert!(destroyed(&state, attack));
}

#[test]
fn an_attacker_that_survives_the_riposte_is_not_destroyed() {
    let (state, attack) = creature_duel(3);
    assert!(!destroyed(&state, attack));
}

#[test]
fn only_attacks_can_destroy_the_attacker() {
    let (state, _) = hitting_state(HERO_RIPOSTE, 2);
    for preview in preview_legal_actions(&state) {
        let value = serde_json::to_value(&preview).unwrap();
        let expected = matches!(preview.action, Action::Attack { .. });
        assert!(value["attacker_destroyed"].is_boolean(), "{value}");
        if !expected {
            assert_eq!(value["attacker_destroyed"], false, "{value}");
        }
    }
    let fresh = built_with(
        HERO_RIPOSTE,
        side(one_hero(3, 8, vec![draw(1, 1)]), creature(1, 1, 1, 3, 0)),
        side(two_hero(8), creature(1, 1, 1, 3, 0)),
    );
    for preview in preview_legal_actions(&fresh) {
        assert_eq!(
            serde_json::to_value(&preview).unwrap()["attacker_destroyed"],
            false,
            "{preview:?}"
        );
    }
}

#[test]
fn attacker_destroyed_is_the_death_of_the_attacker_in_the_real_journal() {
    let mut destroyed_seen = 0_usize;
    let mut survived_seen = 0_usize;
    for hero_riposte in [false, true] {
        for board_limit in [None, Some(5)] {
            let rules = RuleSet {
                hero_riposte,
                board_limit,
            };
            for (m, setup) in matchups().into_iter().enumerate() {
                for seed in 0..30_u64 {
                    let mut setup = setup.clone();
                    setup.rules = rules;
                    let (mut state, _) = new_game(setup, seed).unwrap();
                    let mut rng =
                        Xorshift(seed.wrapping_mul(0x9E37_79B9_7F4A_7C15) ^ (m as u64 + 9));
                    for _ in 0..400 {
                        for preview in preview_legal_actions(&state) {
                            let mut copy = state.clone();
                            let events = apply(&mut copy, preview.action.clone()).unwrap();
                            let expected = match preview.action {
                                Action::Attack { attacker, .. } => events.iter().any(|e| {
                                    matches!(e, Event::Died { creature, .. } if *creature == attacker)
                                }),
                                _ => false,
                            };
                            let value = serde_json::to_value(&preview).unwrap();
                            assert_eq!(
                                value["attacker_destroyed"].as_bool(),
                                Some(expected),
                                "{rules:?}, seed {seed}: {preview:?}"
                            );
                            if matches!(preview.action, Action::Attack { .. }) {
                                if expected {
                                    destroyed_seen += 1;
                                } else {
                                    survived_seen += 1;
                                }
                            }
                        }
                        let actions: Vec<Action> = legal_actions(&state)
                            .into_iter()
                            .filter(|a| *a != Action::Concede)
                            .collect();
                        if actions.is_empty() {
                            break;
                        }
                        apply(&mut state, actions[rng.below(actions.len())].clone()).unwrap();
                    }
                }
            }
        }
    }
    assert!(
        destroyed_seen > 100,
        "destroyed attackers: {destroyed_seen}"
    );
    assert!(survived_seen > 100, "surviving attackers: {survived_seen}");
}
