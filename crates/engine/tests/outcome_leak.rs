//! `Observation::outcome` / `Observation::preview` through the public API (#141): what the seat
//! that acts may know of the game right after one of its legal actions. The hidden information
//! (what a draw brings) never appears: the view after an action counts the drawn cards
//! (`hand_unseen`) and shows the others (`hand_known`).

use duel_cards::{CreatureStats, Effect, Hero, HeroEffect};
use duel_engine::{
    Action, ActionOutcome, CardInstanceId, Event, GameSetup, GameState, Observation, PlayerId,
    PlayerSetup, RuleSet, apply, legal_actions, new_game, player_view, preview_legal_actions,
};
use std::collections::BTreeSet;

const DECK_SIZE: u32 = 30;

struct Xorshift(u64);

impl Xorshift {
    fn below(&mut self, bound: usize) -> usize {
        self.0 ^= self.0 << 13;
        self.0 ^= self.0 >> 7;
        self.0 ^= self.0 << 17;
        usize::try_from(self.0 % u64::try_from(bound).unwrap()).unwrap()
    }
}

fn draw(cost: u32, count: u32) -> HeroEffect {
    HeroEffect {
        name: format!("draw {count}"),
        cost,
        effect: Effect::Draw { count },
    }
}

/// Equal initiatives: the seed decides who starts, so both seats act first across seeds.
fn hero() -> Hero {
    Hero {
        name: "hero".into(),
        max_time: 8,
        starting_hand: 4,
        initiative: 1,
        start_budget: 8,
        health: 10,
        effects: vec![draw(1, 1), draw(2, 2), draw(3, 3)],
    }
}

fn card(n: u32) -> CreatureStats {
    CreatureStats {
        cost: 1 + n % 5,
        force: 1 + n % 3,
        attack_cost: 1 + n % 2,
        health: 1 + n % 4,
        provocation: n.is_multiple_of(7),
        riposte: n % 3,
    }
}

/// Every card is named `SECRET-<id>` (the id of an instance is its position in the setup: One's
/// deck, then Two's), so a name found anywhere gives its id away.
fn secret_setup() -> GameSetup {
    let side = |offset: u32| PlayerSetup {
        hero: hero(),
        cards: (0..DECK_SIZE).map(|i| card(offset + i).into()).collect(),
        card_names: (0..DECK_SIZE)
            .map(|i| format!("SECRET-{}", offset + i))
            .collect(),
    };
    GameSetup {
        one: side(0),
        two: side(DECK_SIZE),
        data_versions: vec!["test@1".into()],
        rules: RuleSet::default(),
    }
}

fn offset(seat: PlayerId) -> u32 {
    match seat {
        PlayerId::One => 0,
        PlayerId::Two => DECK_SIZE,
    }
}

fn setup_side(setup: &mut GameSetup, seat: PlayerId) -> &mut PlayerSetup {
    match seat {
        PlayerId::One => &mut setup.one,
        PlayerId::Two => &mut setup.two,
    }
}

/// Gives the card of `seat` at instance id `id` another name and other stats.
fn replace_card(setup: &mut GameSetup, seat: PlayerId, id: CardInstanceId) {
    let index = usize::try_from(id.0 - offset(seat)).unwrap();
    let side = setup_side(setup, seat);
    side.cards[index] = CreatureStats {
        cost: 9,
        force: 9,
        attack_cost: 9,
        health: 9,
        provocation: true,
        riposte: 9,
    }
    .into();
    side.card_names[index] = format!("OTHER-{}", id.0);
}

fn ids_of(state: &GameState, seat: PlayerId) -> BTreeSet<u32> {
    [state.hand(seat), state.board(seat), state.discard(seat)]
        .concat()
        .iter()
        .map(|c| c.0)
        .collect()
}

/// The ids of the pile of `seat`: all of its instances but the ones in a visible zone.
fn deck_ids(state: &GameState, seat: PlayerId) -> BTreeSet<u32> {
    let visible = ids_of(state, seat);
    (offset(seat)..offset(seat) + DECK_SIZE)
        .filter(|id| !visible.contains(id))
        .collect()
}

/// The ids drawn by one real application of `action` on a copy.
fn drawn_by(state: &GameState, action: Action) -> Vec<CardInstanceId> {
    let mut copy = state.clone();
    apply(&mut copy, action)
        .unwrap()
        .iter()
        .filter_map(|e| match e {
            Event::Drew { card, .. } => Some(*card),
            _ => None,
        })
        .collect()
}

/// Plays `seed`'s game at random and shows every running state to `check`.
fn for_every_state(seed: u64, mut check: impl FnMut(&GameState)) {
    let (mut state, _) = new_game(secret_setup(), seed).unwrap();
    let mut rng = Xorshift(seed.wrapping_mul(0x9E37_79B9_7F4A_7C15) | 1);
    while state.winner().is_none() {
        check(&state);
        let playable: Vec<Action> = legal_actions(&state)
            .into_iter()
            .filter(|a| *a != Action::Concede)
            .collect();
        apply(&mut state, playable[rng.below(playable.len())].clone()).unwrap();
    }
}

/// The numbers that directly follow `marker` in `text` (`CardInstanceId(7)`, `SECRET-7"`).
fn mentioned_ids<'a>(text: &'a str, marker: &'a str) -> impl Iterator<Item = u32> + 'a {
    text.split(marker).skip(1).filter_map(|rest| {
        rest.chars()
            .take_while(char::is_ascii_digit)
            .collect::<String>()
            .parse()
            .ok()
    })
}

/// The cards of a hand without their `unavailable` reason: `PlayerView` computes it for the seat
/// (a full board under the standard rules), `hand_known` is the stable part of the card (id,
/// name, stats), so the comparison is on what was already in the hand, not on the reason.
fn without_availability(cards: &[duel_engine::CardView]) -> Vec<duel_engine::CardView> {
    cards
        .iter()
        .map(|c| duel_engine::CardView {
            unavailable: None,
            ..c.clone()
        })
        .collect()
}

fn outcomes(state: &GameState) -> Vec<ActionOutcome> {
    let obs = Observation::of(state).unwrap();
    (0..obs.actions().len())
        .map(|i| obs.outcome(i).expect("an outcome per legal action"))
        .collect()
}

fn previews(state: &GameState) -> Vec<duel_engine::ActionPreview> {
    let obs = Observation::of(state).unwrap();
    (0..obs.actions().len())
        .map(|i| obs.preview(i).expect("a preview per legal action"))
        .collect()
}

// ---- 1. the story's criterion: two states that differ only by the top of the pile --------

#[test]
fn states_differing_only_by_the_top_of_the_pile_give_equal_outcomes_and_previews() {
    let mut checked_draws = 0;
    for seed in 0..40 {
        let (state_a, _) = new_game(secret_setup(), seed).unwrap();
        let seat = state_a.active();
        let obs = Observation::of(&state_a).unwrap();
        // The cards the draws of the actor would bring: the top of its pile (up to 3).
        let mut top: BTreeSet<CardInstanceId> = BTreeSet::new();
        for action in obs.actions() {
            if matches!(action, Action::Activate { .. }) {
                top.extend(drawn_by(&state_a, action.clone()));
            }
        }
        assert!(!top.is_empty(), "seed {seed}: no draw to compare");
        let mut setup_b = secret_setup();
        for id in &top {
            replace_card(&mut setup_b, seat, *id);
        }
        let (state_b, _) = new_game(setup_b, seed).unwrap();
        // Same seed, same shuffle: everything public or in hand is identical...
        assert_eq!(state_a.hand(seat), state_b.hand(seat), "seed {seed}");
        assert_eq!(state_a.active(), state_b.active());
        // ...and the piles really differ (a real draw brings another card).
        let draw_action = obs
            .actions()
            .iter()
            .find(|a| matches!(a, Action::Activate { effect: 0, .. }))
            .cloned()
            .expect("the first effect draws one card");
        let (mut real_a, mut real_b) = (state_a.clone(), state_b.clone());
        apply(&mut real_a, draw_action.clone()).unwrap();
        apply(&mut real_b, draw_action).unwrap();
        assert_ne!(
            format!("{:?}", player_view(&real_a, seat)),
            format!("{:?}", player_view(&real_b, seat)),
            "seed {seed}: the states must differ by the card drawn"
        );

        assert_eq!(outcomes(&state_a), outcomes(&state_b), "seed {seed}");
        assert_eq!(previews(&state_a), previews(&state_b), "seed {seed}");
        checked_draws += top.len();
    }
    assert!(checked_draws >= 40);
}

// ---- 2. all or nothing: the debug form never mentions a card of the pile -----------------

#[test]
fn no_outcome_mentions_a_card_of_the_actors_pile_or_of_the_opposing_hand_or_pile() {
    for seed in 0..100 {
        for_every_state(seed, |state| {
            let seat = state.active();
            let other = seat.opponent();
            let mut hidden = deck_ids(state, seat);
            hidden.extend(deck_ids(state, other));
            hidden.extend(state.hand(other).iter().map(|c| c.0));
            let obs = Observation::of(state).unwrap();
            for i in 0..obs.actions().len() {
                let text = format!("{:?}", obs.outcome(i).unwrap());
                // One pass over the text: every id and every secret name it mentions.
                let mentioned: BTreeSet<u32> = mentioned_ids(&text, "CardInstanceId(")
                    .chain(mentioned_ids(&text, "SECRET-"))
                    .collect();
                if let Some(id) = mentioned.intersection(&hidden).next() {
                    panic!("seed {seed}: card {id} in {text}");
                }
            }
        });
    }
}

// ---- 3. the outcome's preview is the preview ---------------------------------------------

#[test]
fn the_outcome_preview_is_the_preview_is_the_entry_of_preview_legal_actions() {
    for seed in 0..100 {
        for_every_state(seed, |state| {
            let obs = Observation::of(state).unwrap();
            let all = preview_legal_actions(state);
            let n = obs.actions().len();
            assert_eq!(all.len(), n + 1, "Concede comes last");
            assert_eq!(all.last().map(|p| p.action.clone()), Some(Action::Concede));
            for (i, (action, expected)) in obs.actions().iter().zip(&all).enumerate() {
                let preview = obs.preview(i).unwrap();
                assert_eq!(preview.action, *action);
                assert_eq!(&preview, expected, "seed {seed}, action {i}");
                assert_eq!(obs.outcome(i).unwrap().preview, preview);
            }
        });
    }
}

#[test]
fn an_index_out_of_range_is_a_none_never_a_panic() {
    let (state, _) = new_game(secret_setup(), 1).unwrap();
    let obs = Observation::of(&state).unwrap();
    let n = obs.actions().len();
    for i in [n, n + 1, usize::MAX] {
        assert!(obs.preview(i).is_none());
        assert!(obs.outcome(i).is_none());
    }
}

// ---- 4. hand_known + hand_unseen ---------------------------------------------------------

#[test]
fn the_hand_after_an_action_splits_into_what_was_there_and_what_was_drawn() {
    let (mut plays, mut draws, mut attacks) = (0, 0, 0);
    for seed in 0..60 {
        for_every_state(seed, |state| {
            let obs = Observation::of(state).unwrap();
            let seat = obs.seat();
            let before = obs.view();
            for (i, action) in obs.actions().iter().enumerate() {
                let after = obs.outcome(i).unwrap().view_after;
                let mut real = state.clone();
                apply(&mut real, action.clone()).unwrap();
                let real_hand: Vec<u32> = real.hand(seat).iter().map(|c| c.0).collect();
                let known: Vec<u32> = after.you.hand_known.iter().map(|c| c.id.0).collect();
                // The two parts always account for the real hand, in order.
                assert_eq!(
                    after.you.hand_known.len() + after.you.hand_unseen,
                    real_hand.len()
                );
                assert_eq!(known[..], real_hand[..known.len()]);
                match action {
                    Action::Play { card, .. } => {
                        plays += 1;
                        assert!(!known.contains(&card.0));
                        assert_eq!(after.you.hand_unseen, 0);
                        assert_eq!(after.you.board.len(), before.you.board.len() + 1);
                        assert!(after.you.board.iter().any(|c| c.id == *card));
                        assert_eq!(known.len() + 1, before.you.hand.len());
                    }
                    Action::Activate { .. } => {
                        let drawn = before.you.deck_count - after.you.deck_count;
                        if drawn > 0 {
                            draws += 1;
                        }
                        assert_eq!(after.you.hand_unseen, drawn);
                        assert_eq!(
                            without_availability(&after.you.hand_known),
                            without_availability(&before.you.hand)
                        );
                    }
                    Action::Attack { .. } => {
                        attacks += 1;
                        assert_eq!(after.you.hand_unseen, 0);
                        assert_eq!(
                            without_availability(&after.you.hand_known),
                            without_availability(&before.you.hand)
                        );
                    }
                    Action::Concede => unreachable!("the observation omits Concede"),
                }
            }
        });
    }
    assert!(plays > 0 && draws > 0 && attacks > 0);
}

// ---- 6. known risk: exhaustion after a handover and the opposing hand --------------------

/// Known risk, recorded on purpose: `ends_game: exhaustion` after a handover is evaluated on
/// the hand of the future active player (the opponent). Today the rule only depends on public
/// counts, so two states whose only difference is the CONTENT (not the number) of the opposing
/// hand give the same outcome.
// À rouvrir par #128/#130 (effet non résoluble, sorts sans cible valide)
#[test]
fn the_content_of_the_opposing_hand_does_not_change_the_outcome() {
    for seed in 0..40 {
        let (state_a, _) = new_game(secret_setup(), seed).unwrap();
        let other = state_a.active().opponent();
        let mut setup_b = secret_setup();
        for id in state_a.hand(other) {
            replace_card(&mut setup_b, other, id);
        }
        let (state_b, _) = new_game(setup_b, seed).unwrap();
        assert_eq!(state_a.hand(other).len(), state_b.hand(other).len());
        assert_ne!(
            format!("{:?}", player_view(&state_a, other)),
            format!("{:?}", player_view(&state_b, other)),
            "seed {seed}: the opposing hands must differ"
        );
        assert_eq!(outcomes(&state_a), outcomes(&state_b), "seed {seed}");
    }
}

// ---- 7. with the hero's riposte on, still nothing hidden (#170) --------------------------

#[test]
fn a_suicidal_hit_on_the_hero_reveals_nothing_hidden_in_the_outcome() {
    let mut suicides = 0;
    for seed in 0..60 {
        let mut setup = secret_setup();
        setup.rules = RuleSet {
            hero_riposte: true,
            ..RuleSet::legacy()
        };
        let (mut state, _) = new_game(setup, seed).unwrap();
        let mut rng = Xorshift(seed.wrapping_mul(0x9E37_79B9_7F4A_7C15) | 1);
        while state.winner().is_none() {
            let seat = state.active();
            let other = seat.opponent();
            let mut hidden = deck_ids(&state, seat);
            hidden.extend(deck_ids(&state, other));
            hidden.extend(state.hand(other).iter().map(|c| c.0));
            let obs = Observation::of(&state).unwrap();
            for i in 0..obs.actions().len() {
                let outcome = obs.outcome(i).unwrap();
                let text = format!("{outcome:?}");
                let mentioned: BTreeSet<u32> = mentioned_ids(&text, "CardInstanceId(")
                    .chain(mentioned_ids(&text, "SECRET-"))
                    .collect();
                if let Some(id) = mentioned.intersection(&hidden).next() {
                    panic!("seed {seed}: card {id} in {text}");
                }
                let value = serde_json::to_value(&outcome.preview).unwrap();
                if value["attacker_destroyed"] == true {
                    suicides += 1;
                }
            }
            let playable: Vec<Action> = legal_actions(&state)
                .into_iter()
                .filter(|a| *a != Action::Concede)
                .collect();
            apply(&mut state, playable[rng.below(playable.len())].clone()).unwrap();
        }
    }
    assert!(suicides > 20, "suicidal hits seen: {suicides}");
}

// ---- #209: the tags, the kind and the effects of a drawn card stay hidden ------------------------

/// Every card carries the tag `TAG-<id>`; a third are spells whose effect amount is unique
/// (`5000 + id`), a third immobile units, a third plain units. A drawn card must show none of it.
fn tagged_setup() -> GameSetup {
    let side = |offset: u32| {
        let cards: Vec<serde_json::Value> = (0..DECK_SIZE)
            .map(|i| {
                let id = offset + i;
                let tags = serde_json::json!([format!("TAG-{id}")]);
                match i % 3 {
                    0 => serde_json::json!({"kind": "spell", "cost": 1, "tags": tags, "effects": [
                        {"trigger": "on_play", "ops": [{"op": "attack",
                            "target": {"select": "hero_or_unit", "side": "opposing"},
                            "amount": 5000 + id}]}]}),
                    1 => {
                        serde_json::json!({"cost": 1, "health": 3, "immobile": true, "tags": tags})
                    }
                    _ => serde_json::json!({"cost": 1, "force": 2, "attack_cost": 1, "health": 3,
                                            "tags": tags}),
                }
            })
            .collect();
        serde_json::json!({"hero": hero(), "cards": cards,
            "card_names": (0..DECK_SIZE).map(|i| format!("n{}", offset + i)).collect::<Vec<_>>()})
    };
    serde_json::from_value(serde_json::json!({
        "one": side(0), "two": side(DECK_SIZE), "data_versions": []
    }))
    .unwrap()
}

#[test]
fn a_drawn_card_hides_its_tags_kind_and_effects() {
    let mut checked_draws = 0;
    for seed in 0..30 {
        let (mut state, _) = new_game(tagged_setup(), seed).unwrap();
        let mut rng = Xorshift(seed.wrapping_mul(0x9E37_79B9_7F4A_7C15) | 1);
        while state.winner().is_none() {
            let seat = state.active();
            let obs = Observation::of(&state).unwrap();
            let hidden_from_outcome: BTreeSet<u32> = deck_ids(&state, seat)
                .into_iter()
                .chain(deck_ids(&state, seat.opponent()))
                .chain(state.hand(seat.opponent()).iter().map(|c| c.0))
                .collect();
            for (i, action) in obs.actions().iter().enumerate() {
                let drawn: BTreeSet<u32> = drawn_by(&state, action.clone())
                    .iter()
                    .map(|c| c.0)
                    .collect();
                let outcome = obs.outcome(i).unwrap();
                let text = serde_json::to_string(&outcome.view_after).unwrap();
                let tagged: BTreeSet<u32> = mentioned_ids(&text, "TAG-").collect();
                assert!(
                    tagged.is_disjoint(&drawn),
                    "seed {seed}, action {i}: drawn tag in {text}"
                );
                assert!(
                    tagged.is_disjoint(&hidden_from_outcome),
                    "seed {seed}, action {i}"
                );
                // The unique amount of a spell is found where the spell is public, and a spell
                // is never public here: it only lives in a hand, a pile or nowhere.
                for id in drawn.iter().chain(hidden_from_outcome.iter()) {
                    assert!(
                        !text.contains(&format!("\"amount\":{}", 5000 + id)),
                        "seed {seed}"
                    );
                }
                if !drawn.is_empty() {
                    checked_draws += 1;
                    assert_eq!(outcome.view_after.you.hand_unseen, drawn.len());
                    // What was already known and stays in the hand keeps its public tags.
                    let known: BTreeSet<u32> = outcome
                        .view_after
                        .you
                        .hand_known
                        .iter()
                        .map(|c| c.id.0)
                        .collect();
                    assert!(known.is_disjoint(&drawn));
                    assert!(
                        known.iter().all(|id| tagged.contains(id)),
                        "seed {seed}: a known card lost its tag in {text}"
                    );
                }
            }
            let playable: Vec<Action> = legal_actions(&state)
                .into_iter()
                .filter(|a| *a != Action::Concede)
                .collect();
            apply(&mut state, playable[rng.below(playable.len())].clone()).unwrap();
        }
    }
    assert!(
        checked_draws > 50,
        "the check ran on real draws: {checked_draws}"
    );
}
