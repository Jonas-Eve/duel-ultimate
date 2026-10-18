//! S2b-T2 through the JSON boundary: an attack the provocation forbids is refused with the
//! stable code `target_forbidden_by_provocation`, and `legal_actions` does not offer it.

use duel_cards::{parse_cards, parse_decks, resolve_deck};
use duel_engine::{GameSetup, PlayerSetup, RuleSet};
use duel_engine_wasm::Duel;
use serde_json::{Value, json};

const CARDS: &str = include_str!("../../../data/fixture/1.0.0/cards.json");
const DECKS: &str = include_str!("../../../data/fixture/1.0.0/decks.json");

/// The fixture duel where every card costs 1 (force 1, health 3) and every card of Two has
/// provocation. One starts with 3 time units.
fn setup() -> Value {
    let cards = parse_cards(CARDS).unwrap();
    let decks = parse_decks(DECKS).unwrap();
    let side = |i: usize| PlayerSetup {
        hero: decks[i].hero.clone(),
        cards: resolve_deck(&decks[i], &cards).unwrap(),
        card_names: Vec::new(),
    };
    let mut setup = serde_json::to_value(GameSetup {
        one: side(0),
        two: side(1),
        data_versions: vec!["fixture@1.0.0".into()],
        rules: RuleSet::default(),
    })
    .unwrap();
    for (side, provocation) in [("one", false), ("two", true)] {
        let stats = json!({"cost": 1, "force": 1, "attack_cost": 1, "health": 3,
                           "provocation": provocation});
        setup[side]["cards"] = Value::Array(vec![stats; 30]);
    }
    setup["one"]["hero"]["initiative"] = 9.into();
    setup["one"]["hero"]["start_budget"] = 3.into();
    setup["one"]["hero"]["max_time"] = 8.into();
    setup["two"]["hero"]["initiative"] = 1.into();
    setup["two"]["hero"]["max_time"] = 8.into();
    setup
}

fn legal(game: &Duel) -> Vec<Value> {
    serde_json::from_str(&game.legal_actions_json().unwrap()).unwrap()
}

fn first_play(game: &Duel) -> Value {
    legal(game)
        .into_iter()
        .find(|a| a.get("play").is_some())
        .expect("a card to play")
}

fn code(error: &str) -> String {
    let v: Value = serde_json::from_str(error).expect("errors are JSON");
    v["code"].as_str().expect("a code").to_owned()
}

#[test]
fn a_forbidden_attack_is_refused_with_its_code_and_is_not_offered() {
    let mut game = Duel::from_json(&setup().to_string(), 5).unwrap();
    // One summons three creatures (3 time units), then Two summons one provocateur.
    for _ in 0..3 {
        let action = first_play(&game);
        game.apply_json(&action.to_string()).unwrap();
    }
    let action = first_play(&game);
    game.apply_json(&action.to_string()).unwrap();

    let offered = legal(&game);
    let attacks: Vec<&Value> = offered
        .iter()
        .filter(|a| a.get("attack").is_some())
        .collect();
    assert_eq!(
        attacks.len(),
        3,
        "each of One's creatures, the provocateur only"
    );
    assert!(
        attacks
            .iter()
            .all(|a| a["attack"]["target"].get("creature").is_some()),
        "no attack on the hero is offered: {attacks:?}"
    );

    let attacker = attacks[0]["attack"]["attacker"].clone();
    let forbidden = json!({"attack": {"attacker": attacker, "target": "hero"}});
    let error = game.apply_json(&forbidden.to_string()).unwrap_err();
    assert_eq!(code(&error), "target_forbidden_by_provocation");
    // The game goes on exactly as if nothing had been asked.
    assert_eq!(legal(&game), offered);
}
