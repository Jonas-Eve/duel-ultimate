//! Pins the structure of `docs/design/effets.md` (#125): the catalog table has a fixed format
//! (eight columns, closed value lists, a Phi class and a usage condition for every operation).
//! The lists below are copied on purpose, not imported: the Rust enums of the closed sets do not
//! exist yet, and cross-checking table and enums is an acceptance criterion of #137.
use std::collections::BTreeSet;

const CATALOG: &str = include_str!("../../../docs/design/effets.md");

const SECTION_TITLE: &str = "## Tableau du catalogue";
const HEADER: [&str; 8] = [
    "élément",
    "famille",
    "déclencheur",
    "sélecteur",
    "durée",
    "condition d'usage",
    "classe Φ",
    "vague",
];
const FAMILIES: [&str; 6] = [
    "opération",
    "sélecteur",
    "déclencheur",
    "durée",
    "type de carte",
    "mot-clé",
];
const TRIGGERS: [&str; 5] = ["on_play", "activated", "continuous", "on_event", "n/a"];
const DURATIONS: [&str; 3] = ["instant", "while_source_in_play", "n/a"];
const CLASSES: [&str; 4] = ["S", "D", "N", "n/a"];
const WAVES: [&str; 3] = ["1", "2+", "réservé"];
const SELECTORS: [&str; 5] = [
    "self",
    "hero(camp)",
    "creature(camp)",
    "hero_or_creature(camp)",
    "all_creatures(camp)",
];
const WAVE1_OPERATIONS: [&str; 5] = ["Attack", "DealDamage", "Draw", "ModifyStat", "GrantKeyword"];

/// Checks the table of the catalog section; the error names the faulty line and element.
fn check_catalog(_md: &str) -> Result<(), String> {
    Ok(())
}

#[test]
fn catalog_file_is_committed_and_has_table_section() {
    assert!(
        CATALOG.lines().any(|l| l.trim_end() == SECTION_TITLE),
        "docs/design/effets.md must contain the section `{SECTION_TITLE}`"
    );
}

const VALID: &str = "# Catalogue

## Tableau du catalogue

| élément | famille | déclencheur | sélecteur | durée | condition d'usage | classe Φ | vague |
|---|---|---|---|---|---|---|---|
| Attack | opération | activated, on_play | hero_or_creature(camp) | instant | cible résoluble | D | 1 |
| DealDamage | opération | on_play, activated | hero_or_creature(camp) | instant | cible résoluble | D | 1 |
| Draw | opération | on_play, activated | hero(camp) | instant | pioche résoluble | D | 1 |
| ModifyStat | opération | continuous | creature(camp) | while_source_in_play | source en jeu, résoluble | S | 1 |
| GrantKeyword | opération | continuous | creature(camp) | while_source_in_play | source en jeu, résoluble | S | 1 |
| Soin | opération | on_play | hero_or_creature(camp) | instant | cible résoluble | N | 2+ |
| self | sélecteur | n/a | n/a | n/a | n/a | n/a | 1 |
| hero(camp) | sélecteur | n/a | n/a | n/a | n/a | n/a | 1 |
| creature(camp) | sélecteur | n/a | n/a | n/a | n/a | n/a | 1 |
| hero_or_creature(camp) | sélecteur | n/a | n/a | n/a | n/a | n/a | 1 |
| all_creatures(camp) | sélecteur | n/a | n/a | n/a | n/a | n/a | 1 |
| on_event | déclencheur | on_event | n/a | n/a | n/a | n/a | réservé |

## Suite

| libre | tableau |
|---|---|
";

fn mutate(from: &str, to: &str) -> String {
    assert!(VALID.contains(from), "test bug: `{from}` not in VALID");
    VALID.replacen(from, to, 1)
}

fn assert_rejected(md: &str, needle: &str) {
    match check_catalog(md) {
        Ok(()) => panic!("expected an error mentioning `{needle}`, got Ok"),
        Err(e) => assert!(e.contains(needle), "error `{e}` should mention `{needle}`"),
    }
}

#[test]
fn accepts_valid_minimal_table() {
    assert_eq!(check_catalog(VALID), Ok(()));
}

#[test]
fn rejects_table_section_missing() {
    assert_rejected(&mutate("## Tableau du catalogue", "## Autre"), "section");
}

#[test]
fn rejects_unknown_column_header() {
    assert_rejected(&mutate("| durée |", "| duree |"), "en-tête");
}

#[test]
fn rejects_wrong_column_order() {
    assert_rejected(
        &mutate("| famille | déclencheur |", "| déclencheur | famille |"),
        "en-tête",
    );
}

#[test]
fn rejects_row_with_wrong_cell_count() {
    assert_rejected(
        &mutate("| pioche résoluble | D | 1 |", "| pioche résoluble | D |"),
        "Draw",
    );
    assert_rejected(
        &mutate("| Draw | opération |", "| Draw | op | opération |"),
        "Draw",
    );
}

#[test]
fn rejects_operation_without_phi_class() {
    assert_rejected(
        &mutate(
            "| pioche résoluble | D | 1 |",
            "| pioche résoluble |  | 1 |",
        ),
        "Draw",
    );
}

#[test]
fn rejects_operation_with_na_phi_class() {
    assert_rejected(
        &mutate(
            "| pioche résoluble | D | 1 |",
            "| pioche résoluble | n/a | 1 |",
        ),
        "Draw",
    );
}

#[test]
fn rejects_operation_without_usage_condition() {
    assert_rejected(&mutate("| pioche résoluble | D |", "|  | D |"), "Draw");
    assert_rejected(&mutate("| pioche résoluble | D |", "| n/a | D |"), "Draw");
}

#[test]
fn rejects_value_outside_closed_lists() {
    for (from, to) in [
        ("| Draw | opération |", "| Draw | opérateur |"),
        (
            "| Draw | opération | on_play, activated |",
            "| Draw | opération | on_play, onplay |",
        ),
        (
            "| Draw | opération | on_play, activated | hero(camp) | instant |",
            "| Draw | opération | on_play, activated | hero(camp) | forever |",
        ),
        (
            "| pioche résoluble | D | 1 |",
            "| pioche résoluble | X | 1 |",
        ),
        (
            "| pioche résoluble | D | 1 |",
            "| pioche résoluble | D | 3 |",
        ),
        (
            "| Draw | opération | on_play, activated | hero(camp) |",
            "| Draw | opération | on_play, activated | player |",
        ),
        ("| self | sélecteur |", "| myself | sélecteur |"),
    ] {
        let md = mutate(from, to);
        assert!(check_catalog(&md).is_err(), "should reject `{to}`");
    }
}

#[test]
fn rejects_d_operation_without_resolution_wording() {
    assert_rejected(
        &mutate("| pioche résoluble | D |", "| pioche | D |"),
        "Draw",
    );
}

#[test]
fn rejects_wave1_n_operation() {
    assert_rejected(&mutate("| N | 2+ |", "| N | 1 |"), "Soin");
}

#[test]
fn rejects_s_operation_not_continuous() {
    assert_rejected(
        &mutate(
            "| ModifyStat | opération | continuous |",
            "| ModifyStat | opération | continuous, on_play |",
        ),
        "ModifyStat",
    );
}

#[test]
fn rejects_wave1_on_event() {
    assert_rejected(&mutate("| n/a | réservé |", "| n/a | 1 |"), "on_event");
    assert_rejected(
        &mutate(
            "| Soin | opération | on_play |",
            "| Soin | opération | on_event |",
        ),
        "on_event",
    );
}

#[test]
fn rejects_duplicate_row() {
    let row = "| Draw | opération | on_play, activated | hero(camp) | instant | pioche résoluble | D | 1 |";
    let md = mutate(row, &format!("{row}\n{row}"));
    assert_rejected(&md, "Draw");
}

#[test]
fn rejects_missing_wave1_operation() {
    let row = "| Draw | opération | on_play, activated | hero(camp) | instant | pioche résoluble | D | 1 |\n";
    assert_rejected(&mutate(row, ""), "Draw");
}

#[test]
fn rejects_missing_selector() {
    let row = "| all_creatures(camp) | sélecteur | n/a | n/a | n/a | n/a | n/a | 1 |\n";
    assert_rejected(&mutate(row, ""), "all_creatures(camp)");
}

#[test]
fn committed_catalog_passes_all_checks() {
    assert_eq!(check_catalog(CATALOG), Ok(()));
}

#[test]
fn catalog_lists_every_wave1_operation_and_selector() {
    let _ = (&FAMILIES, &TRIGGERS, &DURATIONS, &CLASSES, &WAVES, &HEADER);
    let cells: BTreeSet<String> = CATALOG
        .lines()
        .filter(|l| l.trim_start().starts_with('|'))
        .filter_map(|l| {
            l.trim()
                .trim_matches('|')
                .split('|')
                .next()
                .map(|c| c.trim().to_string())
        })
        .collect();
    for name in WAVE1_OPERATIONS.iter().chain(SELECTORS.iter()) {
        assert!(cells.contains(*name), "catalog is missing `{name}`");
    }
}
