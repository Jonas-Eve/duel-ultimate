//! What one seat may see of a game: [`player_view`], a pure projection of the state.
//!
//! The two sides have two distinct types on purpose. [`OwnSide`] carries the hand of the
//! seat the view is for; [`OpponentSide`] has no field that could hold the other hand, only
//! its size: the leak is impossible by construction, the tests are a second line of defence.
//! No type here carries a draw pile other than as a count, nor the generator, the seed or
//! the data versions.
//!
//! The view does not authenticate the seat it is asked for: whoever calls [`player_view`]
//! decides which seat to ask. A local adapter (the WASM binding) trusts its client; a server
//! must call it with the seat of the connection and never expose a way to choose another.
//!
//! The JSON shape is a contract (the client reads it): snake_case keys, seats and `active`
//! as `"One"` / `"Two"` (as [`PlayerId`] serializes everywhere else), reasons as
//! `hero_down`, `exhaustion`, `concede`.
//!
//! A card carries, after its eight original keys, `kind`, `immobile`, `tags`, `effects` and
//! `unavailable` (#209, see [`CardView`]). Tags and effects are public for every card the view
//! shows; the hidden hand of the other seat is not in the view, so neither are its tags.

use crate::effects::effective;
use crate::rules;
use crate::state::{
    CardInstance, CardInstanceId, CardKind, GameState, PlayerId, PlayerState, WinReason,
};
use duel_cards::{CardEffect, HeroEffect, Op, PickSlot, Polarity, Selector, Side, Trigger};
use serde::Serialize;

/// How a finished game ended.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
pub struct GameResult {
    pub winner: PlayerId,
    pub reason: WinReason,
}

/// A hero as shown to everyone: `health` is its current health.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct HeroView {
    pub name: String,
    pub health: u32,
    pub max_time: u32,
    pub effects: Vec<HeroEffect>,
}

/// The kind of a card as shown (`unit` | `spell`).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum CardKindView {
    Unit,
    Spell,
}

/// An operation of an effect as the data writes it (`Op` flattened), plus its `polarity` for the
/// owner of the card (`Op::polarity`, the single derivation).
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct OpLine {
    #[serde(flatten)]
    pub op: Op,
    pub polarity: Polarity,
}

/// One line of effect of a card, the same shape for the normal attack and for any declared effect
/// (the display does not tell them apart): `index` is the position in the **declared** `effects`
/// of the card (the `effect` field of a future `Activate`) and is absent for the normal attack,
/// which is synthesised at read time and never written in the data; `cost` is present for an
/// activated effect only; `picks` are the choices the effect expects (`CardEffect::expected_picks`).
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct EffectLine {
    #[serde(skip_serializing_if = "Option::is_none")]
    pub index: Option<u32>,
    pub trigger: Trigger,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub cost: Option<u32>,
    pub ops: Vec<OpLine>,
    pub picks: Vec<PickSlot>,
}

/// A card as shown: `health` is the instance's current health (0 in a discard); `name` is
/// `null` when the setup gave no names. The keys keep this order (a contract): the original eight,
/// then `kind` (always), `immobile` (`true`, omitted when false), `tags` (omitted when empty),
/// `effects` (always, possibly empty), then `unavailable` (omitted when absent, always last).
///
/// The numbers are the **effective** values (0 for the force and the attack cost of a spell or of
/// an immobile unit, 0 for the health of a spell): none is optional. A card of the hand of the seat
/// asked, or of its board, is the only one that may carry `unavailable`; the hidden hand of the
/// other seat is not in the view at all, so neither are its tags nor its effects.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct CardView {
    pub id: CardInstanceId,
    pub name: Option<String>,
    pub cost: u32,
    pub force: u32,
    pub attack_cost: u32,
    pub health: u32,
    pub provocation: bool,
    pub riposte: u32,
    pub kind: CardKindView,
    #[serde(skip_serializing_if = "std::ops::Not::not")]
    pub immobile: bool,
    /// Display text of the card (the raw strings of the tag list of the set).
    #[serde(skip_serializing_if = "Vec::is_empty")]
    pub tags: Vec<String>,
    pub effects: Vec<EffectLine>,
    /// Why the card cannot be used now for its main purpose (an engine code, the same table as the
    /// errors): to play it for a card of the hand of the seat asked (`rules::play_blocker_of`), to
    /// attack for a unit with an attack of the board of that seat (`rules::attack_blocker`); absent
    /// when it can. Never on the other board, a discard or in an [`OutcomeView`].
    #[serde(skip_serializing_if = "Option::is_none")]
    pub unavailable: Option<&'static str>,
}

/// The seat's own side: its hand is visible, its pile only as a count.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct OwnSide {
    pub hero: HeroView,
    pub hand: Vec<CardView>,
    pub deck_count: usize,
    pub board: Vec<CardView>,
    pub discard: Vec<CardView>,
}

/// The other side: its hand and pile only as counts.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct OpponentSide {
    pub hero: HeroView,
    pub hand_count: usize,
    pub deck_count: usize,
    pub board: Vec<CardView>,
    pub discard: Vec<CardView>,
}

/// The game as `seat` is allowed to see it.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct PlayerView {
    pub seat: PlayerId,
    pub active: PlayerId,
    pub budget: u32,
    pub result: Option<GameResult>,
    pub you: OwnSide,
    pub opponent: OpponentSide,
}

/// The acting seat's own side right after one of its actions (#141): the hand is split by
/// provenance. `hand_known` holds the cards that were already in the hand before the action
/// and still are; `hand_unseen` only counts the cards that came from the draw pile during the
/// action. No field can carry a drawn card (id, name or stats), so the leak is impossible by
/// construction.
///
/// No card of an outcome ever carries `unavailable` (always `None`, unlike the hand and the board
/// of `PlayerView`): that reason is derived from the public board, so nothing leaks by leaving it
/// out, and the AI that reads the outcome sees the playable actions from `Observation::actions`,
/// never an unplayable card. A `hand_known` card keeps its public `kind`, `immobile`, `tags` and
/// `effects` (it was already known); a drawn card shows none of them.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct OutcomeOwnSide {
    pub hero: HeroView,
    pub hand_known: Vec<CardView>,
    pub hand_unseen: usize,
    pub deck_count: usize,
    pub board: Vec<CardView>,
    pub discard: Vec<CardView>,
}

/// What the seat that acted may see of the game right after one of its actions: a type
/// distinct from [`PlayerView`], whose own hand never shows what was drawn. The opposing side
/// is the ordinary [`OpponentSide`].
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct OutcomeView {
    pub seat: PlayerId,
    pub active: PlayerId,
    pub budget: u32,
    pub result: Option<GameResult>,
    pub you: OutcomeOwnSide,
    pub opponent: OpponentSide,
}

/// Projects `state` for `seat`. Pure: reads the state, touches neither it nor the generator.
pub fn player_view(state: &GameState, seat: PlayerId) -> PlayerView {
    let you = state.player(seat);
    PlayerView {
        seat,
        active: state.active,
        budget: state.budget,
        result: result_of(state),
        you: OwnSide {
            hero: hero_view(you),
            hand: hand_views(state, seat),
            deck_count: you.deck.len(),
            board: own_board_views(state, seat),
            discard: card_views(state, &you.discard),
        },
        opponent: opponent_side(state, seat),
    }
}

/// The hand of `seat`, each card with the reason it cannot be played, if there is one: the answer of
/// `rules::play_blocker_of` for that card (the predicate `effects::usable` reads), so a spell and a
/// unit may differ.
fn hand_views(state: &GameState, seat: PlayerId) -> Vec<CardView> {
    state
        .player(seat)
        .hand
        .iter()
        .map(|card| CardView {
            unavailable: rules::play_blocker_of(state, seat, card).map(|e| e.code()),
            ..card_view(state, card)
        })
        .collect()
}

/// The board of `seat`, each unit that has an attack line with the reason it cannot attack, if
/// there is one (`rules::attack_blocker`, the predicate `effects::usable` reads). An immobile unit
/// has no attack line, so nothing to blame (#129 gives it its own reason).
fn own_board_views(state: &GameState, seat: PlayerId) -> Vec<CardView> {
    state
        .player(seat)
        .board
        .iter()
        .map(|card| CardView {
            unavailable: has_attack_line(card)
                .then(|| rules::attack_blocker(state, card))
                .flatten()
                .map(|e| e.code()),
            ..card_view(state, card)
        })
        .collect()
}

/// Projects `state_after`, the state right after an action of `seat`, for that seat. `drawn` are
/// the instance ids that came into the hand from the draw pile during the action (the `Drew`
/// events): provenance, not a difference of hands decides what is masked, so a card back in the
/// hand from the board or a discard would stay public. A drawn card is only counted: its id,
/// name and stats never reach the result.
pub(crate) fn outcome_view(
    state_after: &GameState,
    seat: PlayerId,
    drawn: &[CardInstanceId],
) -> OutcomeView {
    let you = state_after.player(seat);
    let (unseen, known): (Vec<&CardInstance>, Vec<&CardInstance>) =
        you.hand.iter().partition(|card| drawn.contains(&card.id));
    OutcomeView {
        seat,
        active: state_after.active,
        budget: state_after.budget,
        result: result_of(state_after),
        you: OutcomeOwnSide {
            hero: hero_view(you),
            hand_known: known
                .into_iter()
                .map(|card| card_view(state_after, card))
                .collect(),
            hand_unseen: unseen.len(),
            deck_count: you.deck.len(),
            board: card_views(state_after, &you.board),
            discard: card_views(state_after, &you.discard),
        },
        opponent: opponent_side(state_after, seat),
    }
}

fn result_of(state: &GameState) -> Option<GameResult> {
    state
        .winner
        .map(|(winner, reason)| GameResult { winner, reason })
}

fn opponent_side(state: &GameState, seat: PlayerId) -> OpponentSide {
    let opponent = state.player(seat.opponent());
    OpponentSide {
        hero: hero_view(opponent),
        hand_count: opponent.hand.len(),
        deck_count: opponent.deck.len(),
        board: card_views(state, &opponent.board),
        discard: card_views(state, &opponent.discard),
    }
}

fn hero_view(side: &PlayerState) -> HeroView {
    HeroView {
        name: side.hero.name.clone(),
        health: side.health,
        max_time: side.hero.max_time,
        effects: side.hero.effects.clone(),
    }
}

fn card_views(state: &GameState, cards: &[CardInstance]) -> Vec<CardView> {
    cards.iter().map(|card| card_view(state, card)).collect()
}

/// Whether the card shows a normal attack: a unit that is not immobile. A spell has none, and the
/// attack of an immobile unit does not exist.
fn has_attack_line(card: &CardInstance) -> bool {
    card.kind() == CardKind::Unit && !card.immobile()
}

/// The normal attack, synthesised at read time (never written in the data) in the very format of a
/// special attack: an activated effect of cost `attack_cost` with one `attack` operation on the
/// opposing hero or unit of amount `force`, both effective values.
fn normal_attack_line(state: &GameState, card: &CardInstance) -> EffectLine {
    let values = effective(state, card);
    let effect = CardEffect {
        trigger: Trigger::Activated,
        cost: Some(values.attack_cost),
        ops: vec![Op::Attack {
            target: Selector::HeroOrUnit {
                side: Side::Opposing,
            },
            amount: values.force,
            ignores_provocation: false,
            scales_with_force: false,
            attackers: 1,
            mode: None,
        }],
    };
    effect_line(None, &effect, false)
}

fn effect_line(index: Option<u32>, effect: &CardEffect, is_spell: bool) -> EffectLine {
    EffectLine {
        index,
        trigger: effect.trigger,
        cost: effect.cost,
        ops: effect
            .ops
            .iter()
            .map(|op| OpLine {
                op: op.clone(),
                polarity: op.polarity(),
            })
            .collect(),
        picks: effect.expected_picks(is_spell),
    }
}

/// The lines of effect of a card: the normal attack first (a unit that is not immobile, no index),
/// then the declared effects in the order of the data, each with its index. The same for a card of
/// any zone: it is the content of the card, not an action.
fn effect_lines(state: &GameState, card: &CardInstance) -> Vec<EffectLine> {
    let mut lines = Vec::new();
    if has_attack_line(card) {
        lines.push(normal_attack_line(state, card));
    }
    let is_spell = card.is_spell();
    lines.extend(card.effects().iter().enumerate().map(|(i, effect)| {
        // At most `MAX_CARD_EFFECTS` (validated at setup), so the index always fits.
        effect_line(u32::try_from(i).ok(), effect, is_spell)
    }));
    lines
}

fn card_view(state: &GameState, card: &CardInstance) -> CardView {
    let values = effective(state, card);
    CardView {
        id: card.id,
        name: card.name.clone(),
        cost: values.cost,
        force: values.force,
        attack_cost: values.attack_cost,
        health: card.stats.health,
        provocation: values.provocation,
        riposte: values.riposte,
        kind: match card.kind() {
            CardKind::Unit => CardKindView::Unit,
            CardKind::Spell => CardKindView::Spell,
        },
        immobile: card.immobile(),
        tags: card.tags().to_vec(),
        effects: effect_lines(state, card),
        unavailable: None,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::testing::{draw, hero, setup, stats_with};
    use crate::{Action, Target, apply, legal_actions, new_game};
    use serde_json::Value;
    use std::collections::BTreeSet;

    /// A tiny deterministic generator for the tests' random play (no dependency on the
    /// engine's own generator, which `apply` must never consume).
    struct Xorshift(u64);

    impl Xorshift {
        fn next(&mut self) -> u64 {
            self.0 ^= self.0 << 13;
            self.0 ^= self.0 >> 7;
            self.0 ^= self.0 << 17;
            self.0
        }

        fn below(&mut self, bound: usize) -> usize {
            usize::try_from(self.next() % u64::try_from(bound.max(1)).unwrap()).unwrap()
        }
    }

    /// A setup whose cards all carry a name that gives away their identity: `SECRET-<id>`
    /// (the id of an instance is its position in the setup: One's deck, then Two's).
    fn secret_setup() -> crate::GameSetup {
        let mut s = setup(
            hero(2, 8, 8, vec![draw(1, 1), draw(2, 2)]),
            hero(1, 2, 8, vec![draw(1, 1)]),
        );
        for (offset, side) in [(0, &mut s.one), (30, &mut s.two)] {
            side.card_names = (0..30).map(|i| format!("SECRET-{}", offset + i)).collect();
        }
        s
    }

    fn running_game(seed: u64) -> GameState {
        new_game(secret_setup(), seed).unwrap().0
    }

    /// Plays random non-conceding actions until the game ends or `steps` are spent,
    /// handing every intermediate state (the initial one included) to `check`.
    fn play_randomly(seed: u64, steps: usize, mut check: impl FnMut(&GameState)) {
        let mut state = running_game(seed);
        let mut rng = Xorshift(seed.wrapping_mul(0x9E37_79B9_7F4A_7C15) | 1);
        check(&state);
        for _ in 0..steps {
            let actions: Vec<Action> = legal_actions(&state)
                .into_iter()
                .filter(|a| *a != Action::Concede)
                .collect();
            if actions.is_empty() {
                break;
            }
            let action = actions[rng.below(actions.len())].clone();
            apply(&mut state, action).unwrap();
            check(&state);
        }
    }

    fn ids(cards: &[CardView]) -> BTreeSet<u32> {
        cards.iter().map(|c| c.id.0).collect()
    }

    /// Every instance id of the zones in `piles` (the ones a seat must never learn).
    fn pile_ids(p: &PlayerState) -> BTreeSet<u32> {
        p.deck.iter().map(|c| c.id.0).collect()
    }

    fn hand_ids(p: &PlayerState) -> BTreeSet<u32> {
        p.hand.iter().map(|c| c.id.0).collect()
    }

    /// `SECRET-<n>` names found in a text, as ids.
    fn secret_ids(text: &str) -> BTreeSet<u32> {
        text.split("SECRET-")
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

    fn keys(value: &Value) -> BTreeSet<String> {
        value.as_object().unwrap().keys().cloned().collect()
    }

    fn set(names: &[&str]) -> BTreeSet<String> {
        names.iter().map(|s| (*s).to_owned()).collect()
    }

    // ---- leak: the view of a seat never carries the other hand nor any pile -----------

    #[test]
    fn a_view_holds_the_own_hand_the_boards_and_the_discards_and_nothing_hidden() {
        for seed in 0..40 {
            play_randomly(seed, 400, |state| {
                for seat in [PlayerId::One, PlayerId::Two] {
                    let view = player_view(state, seat);
                    let (me, other) = (state.player(seat), state.player(seat.opponent()));
                    let json = serde_json::to_string(&view).unwrap();

                    // The ids the view may mention: own hand, both boards, both discards.
                    let mut allowed = hand_ids(me);
                    for p in [me, other] {
                        allowed.extend(p.board.iter().map(|c| c.id.0));
                        allowed.extend(p.discard.iter().map(|c| c.id.0));
                    }
                    let mut mentioned = ids(&view.you.hand);
                    for cards in [
                        &view.you.board,
                        &view.you.discard,
                        &view.opponent.board,
                        &view.opponent.discard,
                    ] {
                        mentioned.extend(ids(cards));
                    }
                    assert_eq!(mentioned, allowed, "seed {seed} seat {seat:?}");

                    // The names are the identity of a card: the text mentions exactly those.
                    assert_eq!(
                        secret_ids(&json),
                        allowed,
                        "seed {seed} seat {seat:?}: {json}"
                    );

                    // Disjoint from the other hand and from both piles.
                    let hidden: BTreeSet<u32> = hand_ids(other)
                        .into_iter()
                        .chain(pile_ids(other))
                        .chain(pile_ids(me))
                        .collect();
                    assert!(
                        mentioned.is_disjoint(&hidden),
                        "seed {seed} seat {seat:?}: leaked {:?}",
                        mentioned.intersection(&hidden).collect::<Vec<_>>()
                    );
                }
            });
        }
    }

    #[test]
    fn the_sides_have_exactly_the_agreed_keys() {
        let state = running_game(3);
        let value = serde_json::to_value(player_view(&state, PlayerId::One)).unwrap();
        assert_eq!(
            keys(&value),
            set(&["seat", "active", "budget", "result", "you", "opponent"])
        );
        assert_eq!(
            keys(&value["you"]),
            set(&["hero", "hand", "deck_count", "board", "discard"])
        );
        assert_eq!(
            keys(&value["opponent"]),
            set(&["hero", "hand_count", "deck_count", "board", "discard"])
        );
        assert_eq!(
            keys(&value["you"]["hero"]),
            set(&["name", "health", "max_time", "effects"])
        );
        assert_eq!(
            keys(&value["you"]["hand"][0]),
            set(&[
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
            ])
        );
    }

    #[test]
    fn the_view_exposes_neither_the_generator_nor_the_seed_nor_the_versions() {
        let state = running_game(3);
        let json = serde_json::to_string(&player_view(&state, PlayerId::Two)).unwrap();
        for hidden in ["seed", "rng", "version", "fixture@1.0.0"] {
            assert!(!json.contains(hidden), "{hidden} in {json}");
        }
    }

    #[test]
    fn counts_come_from_the_right_sides_and_cards_are_conserved() {
        for seed in 0..40 {
            play_randomly(seed, 400, |state| {
                for seat in [PlayerId::One, PlayerId::Two] {
                    let view = player_view(state, seat);
                    let (me, other) = (state.player(seat), state.player(seat.opponent()));
                    assert_eq!(view.you.hand.len(), me.hand.len());
                    assert_eq!(view.you.deck_count, me.deck.len());
                    assert_eq!(view.opponent.hand_count, other.hand.len());
                    assert_eq!(view.opponent.deck_count, other.deck.len());
                    assert_eq!(
                        view.you.hand.len()
                            + view.you.deck_count
                            + view.you.board.len()
                            + view.you.discard.len(),
                        30
                    );
                    assert_eq!(
                        view.opponent.hand_count
                            + view.opponent.deck_count
                            + view.opponent.board.len()
                            + view.opponent.discard.len(),
                        30
                    );
                }
            });
        }
    }

    // ---- purity ------------------------------------------------------------------------

    #[test]
    fn a_view_changes_neither_the_state_nor_the_generator() {
        for seed in 0..10 {
            play_randomly(seed, 60, |state| {
                let before = state.clone();
                let first = player_view(state, PlayerId::One);
                let second = player_view(state, PlayerId::One);
                let _ = player_view(state, PlayerId::Two);
                assert_eq!(first, second);
                assert_eq!(*state, before, "the state is untouched, generator included");
                let (mut a, mut b) = (state.clone(), before);
                assert_eq!(a.random_below(1_000_000), b.random_below(1_000_000));
            });
        }
    }

    // ---- content and semantics ----------------------------------------------------------

    fn card(id: u32, name: Option<&str>, health: u32) -> CardInstance {
        CardInstance {
            id: CardInstanceId(id),
            stats: stats_with(3, 4, 2, health, true, 1),
            name: name.map(str::to_owned),
            extras: None,
        }
    }

    /// A state whose zones are written by hand, so that the expected JSON can be too.
    fn handmade() -> GameState {
        let mut state = running_game(1);
        let (one, two) = (&mut state.one, &mut state.two);
        one.hand = vec![card(7, Some("Alpha"), 4), card(2, None, 5)];
        one.deck = vec![card(8, Some("SECRET-deck"), 1)];
        one.board = vec![card(5, Some("Beta"), 2)];
        one.discard = vec![card(9, Some("Gamma"), 0), card(1, Some("Delta"), 0)];
        one.health = 6;
        two.hand = vec![card(40, Some("SECRET-hand"), 1)];
        two.deck = vec![card(41, Some("SECRET-pile"), 1), card(42, None, 1)];
        two.board = vec![];
        two.discard = vec![];
        two.health = 9;
        state.active = PlayerId::Two;
        state.budget = 3;
        state
    }

    #[test]
    fn the_json_shape_is_frozen() {
        let state = handmade();
        let json = serde_json::to_string(&player_view(&state, PlayerId::One)).unwrap();
        let c = |id: u32, name: &str, health: u32| {
            format!(
                r#"{{"id":{id},"name":{name},"cost":3,"force":4,"attack_cost":2,"health":{health},"provocation":true,"riposte":1,"kind":"unit","effects":[{{"trigger":"activated","cost":2,"ops":[{{"op":"attack","target":{{"select":"hero_or_unit","side":"opposing"}},"amount":4,"polarity":"bonus"}}],"picks":[{{"role":"target","kind":"hero_or_unit","side":"opposing"}}]}}]}}"#
            )
        };
        let hero = |name: &str, health: u32, max_time: u32, effects: &str| {
            format!(
                r#"{{"name":"{name}","health":{health},"max_time":{max_time},"effects":{effects}}}"#
            )
        };
        let expected = format!(
            concat!(
                r#"{{"seat":"One","active":"Two","budget":3,"result":null,"#,
                r#""you":{{"hero":{},"hand":[{},{}],"deck_count":1,"board":[{}],"discard":[{},{}]}},"#,
                r#""opponent":{{"hero":{},"hand_count":1,"deck_count":2,"board":[],"discard":[]}}}}"#
            ),
            hero(
                "hero-2",
                6,
                8,
                r#"[{"name":"draw 1 for 1","cost":1,"effect":{"kind":"draw","count":1}},{"name":"draw 2 for 2","cost":2,"effect":{"kind":"draw","count":2}}]"#
            ),
            c(7, r#""Alpha""#, 4),
            c(2, "null", 5),
            c(5, r#""Beta""#, 2),
            c(9, r#""Gamma""#, 0),
            c(1, r#""Delta""#, 0),
            hero(
                "hero-1",
                9,
                8,
                r#"[{"name":"draw 1 for 1","cost":1,"effect":{"kind":"draw","count":1}}]"#
            ),
        );
        assert_eq!(json, expected);
    }

    #[test]
    fn the_discard_keeps_the_order_of_death_not_the_order_of_ids() {
        let state = handmade();
        let view = player_view(&state, PlayerId::One);
        let ids: Vec<u32> = view.you.discard.iter().map(|c| c.id.0).collect();
        assert_eq!(ids, vec![9, 1]);
        // The same discard seen from the other seat, as the opponent's.
        let other = player_view(&state, PlayerId::Two);
        let ids: Vec<u32> = other.opponent.discard.iter().map(|c| c.id.0).collect();
        assert_eq!(ids, vec![9, 1]);
    }

    #[test]
    fn a_view_after_two_real_kills_lists_the_dead_in_order_of_death() {
        // Two attacks, the second killing a creature of a *lower* id than the first.
        let mut state = running_game(2);
        state.one.hand.clear();
        state.two.hand.clear();
        let mut strong = card(50, Some("strong"), 9);
        strong.stats = stats_with(1, 5, 1, 9, false, 0);
        state.one.board = vec![strong];
        state.two.board = vec![card(30, Some("late"), 1), card(10, Some("early"), 1)];
        for stats in state.two.board.iter_mut().map(|c| &mut c.stats) {
            stats.provocation = false;
            stats.riposte = 0;
        }
        state.active = PlayerId::One;
        state.budget = 8;
        for victim in [30, 10] {
            apply(
                &mut state,
                Action::Attack {
                    attacker: CardInstanceId(50),
                    target: Target::Creature(CardInstanceId(victim)),
                },
            )
            .unwrap();
        }
        let view = player_view(&state, PlayerId::One);
        let dead: Vec<u32> = view.opponent.discard.iter().map(|c| c.id.0).collect();
        assert_eq!(dead, vec![30, 10]);
        assert!(view.opponent.discard.iter().all(|c| c.health == 0));
    }

    #[test]
    fn the_view_shows_current_health_for_heroes_and_creatures() {
        let mut state = handmade();
        state.one.board[0].stats.health = 1;
        let view = player_view(&state, PlayerId::One);
        assert_eq!(view.you.hero.health, 6);
        assert_eq!(view.opponent.hero.health, 9);
        assert_eq!(view.you.board[0].health, 1);
        assert_eq!(view.you.hero.max_time, 8);
        // And from the other seat the same public blocks.
        let other = player_view(&state, PlayerId::Two);
        assert_eq!(other.opponent.hero, view.you.hero);
        assert_eq!(other.you.hero, view.opponent.hero);
        assert_eq!(other.opponent.board, view.you.board);
        assert_eq!(other.opponent.discard, view.you.discard);
    }

    #[test]
    fn active_budget_and_pile_sizes_are_read_from_the_right_places() {
        let state = handmade();
        let one = player_view(&state, PlayerId::One);
        let two = player_view(&state, PlayerId::Two);
        for view in [&one, &two] {
            assert_eq!((view.active, view.budget), (PlayerId::Two, 3));
        }
        assert_eq!((one.seat, two.seat), (PlayerId::One, PlayerId::Two));
        assert_eq!((one.you.deck_count, one.opponent.deck_count), (1, 2));
        assert_eq!((two.you.deck_count, two.opponent.deck_count), (2, 1));
        assert_eq!((one.opponent.hand_count, two.opponent.hand_count), (1, 2));
        assert_eq!(ids(&two.you.hand), BTreeSet::from([40]));
    }

    // ---- finished games -------------------------------------------------------------------

    #[test]
    fn a_running_game_has_no_result() {
        let state = running_game(1);
        assert_eq!(player_view(&state, PlayerId::One).result, None);
    }

    #[test]
    fn a_conceded_game_names_the_other_player_the_winner() {
        let mut state = running_game(1);
        let conceding = state.active();
        apply(&mut state, Action::Concede).unwrap();
        for seat in [PlayerId::One, PlayerId::Two] {
            assert_eq!(
                player_view(&state, seat).result,
                Some(GameResult {
                    winner: conceding.opponent(),
                    reason: WinReason::Concede
                })
            );
        }
    }

    #[test]
    fn a_game_won_on_the_hero_shows_hero_down_and_a_dead_hero() {
        let mut state = running_game(1);
        let mut hitter = card(50, Some("hitter"), 5);
        hitter.stats = stats_with(1, 50, 1, 5, false, 0);
        let me = state.active();
        state.player_mut(me).board = vec![hitter];
        state.player_mut(me.opponent()).board.clear();
        apply(
            &mut state,
            Action::Attack {
                attacker: CardInstanceId(50),
                target: Target::Hero,
            },
        )
        .unwrap();
        let view = player_view(&state, me.opponent());
        assert_eq!(
            view.result,
            Some(GameResult {
                winner: me,
                reason: WinReason::HeroDown
            })
        );
        assert_eq!(view.you.hero.health, 0);
        assert_eq!(player_view(&state, me).opponent.hero.health, 0);
    }

    #[test]
    fn a_game_lost_by_exhaustion_shows_its_result_and_the_idle_player_as_active() {
        let mut one = hero(2, 3, 8, vec![]);
        one.starting_hand = 0;
        let (state, _) = new_game(setup(one, hero(1, 2, 5, vec![draw(1, 1)])), 3).unwrap();
        let view = player_view(&state, PlayerId::Two);
        assert_eq!(
            view.result,
            Some(GameResult {
                winner: PlayerId::Two,
                reason: WinReason::Exhaustion
            })
        );
        assert_eq!(
            view.active,
            PlayerId::One,
            "the player left without an action"
        );
        assert_eq!(view.budget, state.budget());
        assert_eq!(view.opponent.hand_count, 0);
    }

    // ---- outcome_view: provenance decides what is masked (#141) -----------------------

    #[test]
    fn a_card_back_in_hand_from_the_discard_stays_public_and_a_drawn_one_is_only_counted() {
        let mut state = running_game(5);
        let seat = state.active;
        let side = state.player_mut(seat);
        // A card that came back from a discard (a future operation: the cause is not a draw).
        let returned = side.deck.pop().expect("a pile");
        side.discard.push(returned.clone());
        let back = side.discard.pop().expect("the card just discarded");
        side.hand.push(back);
        // And a card really drawn.
        let drawn = side.deck.pop().expect("a pile");
        let drawn_id = drawn.id;
        side.hand.push(drawn);

        let view = outcome_view(&state, seat, &[drawn_id]);
        assert_eq!(view.you.hand_unseen, 1);
        assert!(view.you.hand_known.iter().any(|c| c.id == returned.id));
        assert!(view.you.hand_known.iter().all(|c| c.id != drawn_id));
        let text = format!("{view:?}");
        assert!(!text.contains(&format!("SECRET-{}\"", drawn_id.0)));
        assert!(text.contains(&format!("SECRET-{}\"", returned.id.0)));
        // Without provenance information nothing is masked (the masking is the events' job).
        assert_eq!(outcome_view(&state, seat, &[]).you.hand_unseen, 0);
    }

    // ---- #209: tags, kind and the reason of a board unit ---------------------------------

    use crate::state::{CardExtras, CardKind};
    use std::sync::Arc;

    /// `card` carrying extras: its kind, the immobile keyword and its tags.
    fn with_extras(
        mut card: CardInstance,
        kind: CardKind,
        immobile: bool,
        tags: &[&str],
    ) -> CardInstance {
        card.extras = Some(Arc::new(CardExtras {
            kind,
            immobile,
            tags: tags.iter().map(|t| (*t).to_owned()).collect(),
            effects: Vec::new(),
        }));
        card
    }

    fn force(mut card: CardInstance, force: u32) -> CardInstance {
        card.stats.force = force;
        card
    }

    fn unavailable_of(value: &Value, zone: &[&str]) -> Vec<Option<String>> {
        let mut cursor = value;
        for key in zone {
            cursor = &cursor[*key];
        }
        cursor
            .as_array()
            .unwrap()
            .iter()
            .map(|c| c.get("unavailable").map(|v| v.as_str().unwrap().to_owned()))
            .collect()
    }

    #[test]
    fn a_board_unit_with_force_zero_says_attack_force_zero_and_only_on_my_board() {
        let mut state = handmade();
        let immobile = with_extras(
            force(card(21, Some("wall"), 3), 0),
            CardKind::Unit,
            true,
            &[],
        );
        state.one.board = vec![
            force(card(20, Some("idle"), 3), 0),
            immobile,
            force(card(22, Some("fine"), 3), 1),
        ];
        state.one.discard = vec![force(card(23, Some("dead"), 0), 0)];
        state.two.board = vec![force(card(43, Some("their-idle"), 3), 0)];
        state.two.discard = vec![force(card(44, Some("their-dead"), 0), 0)];
        let one = serde_json::to_value(player_view(&state, PlayerId::One)).unwrap();
        // Force 0 without the keyword: the reason; the immobile one has no attack to blame; force 1
        // is the boundary: no reason.
        assert_eq!(
            unavailable_of(&one, &["you", "board"]),
            vec![Some("attack_force_zero".to_owned()), None, None]
        );
        assert_eq!(unavailable_of(&one, &["you", "discard"]), vec![None]);
        assert_eq!(unavailable_of(&one, &["opponent", "board"]), vec![None]);
        assert_eq!(unavailable_of(&one, &["opponent", "discard"]), vec![None]);
        // The same cards seen from the other seat: now they are the opponent's, no reason.
        let two = serde_json::to_value(player_view(&state, PlayerId::Two)).unwrap();
        assert_eq!(
            unavailable_of(&two, &["opponent", "board"]),
            vec![None, None, None]
        );
        // It does not depend on whose turn it is (the view of the seat that waits has it too).
        assert_eq!(state.active, PlayerId::Two);
        // A finished game blocks nothing.
        state.winner = Some((PlayerId::One, WinReason::Concede));
        let done = serde_json::to_value(player_view(&state, PlayerId::One)).unwrap();
        assert_eq!(
            unavailable_of(&done, &["you", "board"]),
            vec![None, None, None]
        );
        // The outcome view never carries the reason, hand or board.
        state.winner = None;
        let outcome = serde_json::to_string(&outcome_view(&state, PlayerId::One, &[])).unwrap();
        assert!(!outcome.contains("unavailable"), "{outcome}");
    }

    #[test]
    fn the_tags_of_the_public_zones_are_public_and_the_hidden_hand_ones_are_not() {
        let mut state = handmade();
        let tag = |c: CardInstance, t: &str| with_extras(c, CardKind::Unit, false, &[t]);
        state.one.hand = vec![tag(card(7, Some("Alpha"), 4), "own-hand")];
        state.one.deck = vec![tag(card(8, Some("a"), 1), "own-deck")];
        state.one.board = vec![tag(card(5, Some("Beta"), 2), "own-board")];
        state.one.discard = vec![tag(card(9, Some("Gamma"), 0), "own-discard")];
        state.two.hand = vec![
            tag(card(40, Some("h"), 1), "SECRET-hand"),
            with_extras(
                card(45, Some("s"), 1),
                CardKind::Spell,
                false,
                &["SECRET-spell"],
            ),
        ];
        state.two.deck = vec![tag(card(41, Some("d"), 1), "SECRET-deck")];
        state.two.board = vec![tag(card(42, Some("b"), 1), "public-board")];
        state.two.discard = vec![tag(card(43, Some("x"), 0), "public-discard")];
        let one = serde_json::to_string(&player_view(&state, PlayerId::One)).unwrap();
        for shown in [
            "own-hand",
            "own-board",
            "own-discard",
            "public-board",
            "public-discard",
        ] {
            assert!(one.contains(shown), "{shown} in {one}");
        }
        for hidden in ["own-deck", "SECRET-hand", "SECRET-deck", "SECRET-spell"] {
            assert!(!one.contains(hidden), "{hidden} in {one}");
        }
        assert!(
            !one.contains(r#""kind":"spell""#),
            "the hidden spell: {one}"
        );
        // The witness: the other seat reads its own hand.
        let two = serde_json::to_string(&player_view(&state, PlayerId::Two)).unwrap();
        assert!(two.contains("SECRET-hand") && two.contains("SECRET-spell"));
        assert!(two.contains(r#""kind":"spell""#));
        assert!(!two.contains("SECRET-deck"));
    }

    #[test]
    fn a_drawn_card_hides_its_tags_kind_and_effects_but_a_returned_one_keeps_them() {
        let mut state = running_game(5);
        let seat = state.active;
        let side = state.player_mut(seat);
        let mut returned = side.deck.pop().expect("a pile");
        returned = with_extras(returned, CardKind::Unit, true, &["returned-tag"]);
        side.discard.push(returned.clone());
        let back = side.discard.pop().expect("the card just discarded");
        side.hand.push(back);
        let drawn = with_extras(
            side.deck.pop().expect("a pile"),
            CardKind::Spell,
            false,
            &["drawn-tag"],
        );
        let drawn_id = drawn.id;
        side.hand.push(drawn);
        let view = outcome_view(&state, seat, &[drawn_id]);
        let text = serde_json::to_string(&view).unwrap();
        assert!(!text.contains("drawn-tag"), "{text}");
        assert!(!text.contains(r#""kind":"spell""#), "{text}");
        assert!(text.contains("returned-tag"), "{text}");
        assert!(text.contains(r#""immobile":true"#), "{text}");
        // The ordinary view of the seat shows both (the card is in its hand).
        let own = serde_json::to_string(&player_view(&state, seat)).unwrap();
        assert!(own.contains("drawn-tag") && own.contains("returned-tag"));
    }

    #[test]
    fn swapping_the_top_of_the_pile_with_another_card_changes_no_outcome() {
        use crate::Observation;
        for seed in 0..20 {
            let state = running_game(seed);
            let mut swapped = state.clone();
            let seat = swapped.active;
            let deck = &mut swapped.player_mut(seat).deck;
            let last = deck.len() - 1;
            deck.swap(last, 0);
            let (a, b) = (
                Observation::of(&state).unwrap(),
                Observation::of(&swapped).unwrap(),
            );
            assert_eq!(a.actions(), b.actions());
            for i in 0..a.actions().len() {
                assert_eq!(a.outcome(i), b.outcome(i), "seed {seed}, action {i}");
            }
        }
    }
}
