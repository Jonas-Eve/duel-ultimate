//! `CreatureStats` travels as JSON in the engine setup (engine-wasm), so it is strict data too.
use duel_cards::CreatureStats;

#[test]
fn creature_stats_round_trip_through_json() {
    let stats = CreatureStats {
        cost: 2,
        force: 3,
        attack_cost: 4,
        health: 5,
    };
    let json = serde_json::to_string(&stats).unwrap();
    assert_eq!(serde_json::from_str::<CreatureStats>(&json).unwrap(), stats);
}

#[test]
fn creature_stats_reject_unknown_fields() {
    let json = r#"{"cost":1,"force":1,"attack_cost":1,"health":1,"bonus":9}"#;
    let err = serde_json::from_str::<CreatureStats>(json).unwrap_err();
    assert!(err.to_string().contains("bonus"), "{err}");
}
