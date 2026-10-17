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

/// Splits a Markdown table line into trimmed cells (a `|` can never be inside a cell).
fn cells(line: &str) -> Vec<String> {
    let t = line.trim();
    let t = t.strip_prefix('|').unwrap_or(t);
    let t = t.strip_suffix('|').unwrap_or(t);
    t.split('|').map(|c| c.trim().to_string()).collect()
}

fn in_list(value: &str, list: &[&str]) -> bool {
    list.contains(&value)
}

/// Every `, `-separated value of a cell must belong to the closed list.
fn all_in_list(value: &str, list: &[&str]) -> bool {
    value.split(", ").all(|v| in_list(v, list))
}

/// Checks the table of the catalog section; the error names the faulty line and element.
fn check_catalog(md: &str) -> Result<(), String> {
    let lines: Vec<&str> = md.lines().collect();
    let start = lines
        .iter()
        .position(|l| l.trim_end() == SECTION_TITLE)
        .ok_or_else(|| format!("section `{SECTION_TITLE}` manquante"))?;
    // Only the table lines between the title and the next `## ` heading are read.
    let mut table: Vec<(usize, &str)> = Vec::new();
    for (i, l) in lines.iter().enumerate().skip(start + 1) {
        if l.starts_with("## ") {
            break;
        }
        if l.trim_start().starts_with('|') {
            table.push((i + 1, l));
        }
    }
    let Some(&(hline, header)) = table.first() else {
        return Err("section du tableau sans tableau".to_string());
    };
    if cells(header) != HEADER {
        return Err(format!("ligne {hline}: en-tête attendu {HEADER:?}"));
    }
    let Some(&(sline, sep)) = table.get(1) else {
        return Err("ligne de séparation manquante".to_string());
    };
    let sep_cells = cells(sep);
    let sep_ok = sep_cells.len() == HEADER.len()
        && sep_cells
            .iter()
            .all(|c| !c.is_empty() && c.chars().all(|ch| ch == '-' || ch == ':'));
    if !sep_ok {
        return Err(format!("ligne {sline}: séparation à 8 colonnes attendue"));
    }

    let mut seen: BTreeSet<(String, String)> = BTreeSet::new();
    let mut operations: BTreeSet<String> = BTreeSet::new();
    let mut selectors: BTreeSet<String> = BTreeSet::new();
    for &(n, line) in &table[2..] {
        let c = cells(line);
        let name = c.first().cloned().unwrap_or_default();
        let at = format!("ligne {n}, élément `{name}`");
        if c.len() != HEADER.len() {
            return Err(format!("{at}: {} cellules au lieu de 8", c.len()));
        }
        if let Some(i) = c.iter().position(String::is_empty) {
            return Err(format!("{at}: cellule vide (colonne {})", HEADER[i]));
        }
        let [_, family, trigger, selector, duration, usage, class, wave] =
            <[String; 8]>::try_from(c).expect("8 cells checked above");
        if !in_list(&family, &FAMILIES) {
            return Err(format!("{at}: famille `{family}` hors liste fermée"));
        }
        if !all_in_list(&trigger, &TRIGGERS) {
            return Err(format!("{at}: déclencheur `{trigger}` hors liste fermée"));
        }
        if !in_list(&duration, &DURATIONS) {
            return Err(format!("{at}: durée `{duration}` hors liste fermée"));
        }
        if !in_list(&class, &CLASSES) {
            return Err(format!("{at}: classe Φ `{class}` hors liste fermée"));
        }
        if !in_list(&wave, &WAVES) {
            return Err(format!("{at}: vague `{wave}` hors liste fermée"));
        }
        if !seen.insert((name.clone(), family.clone())) {
            return Err(format!("{at}: ligne en double (élément, famille)"));
        }
        // R5: `on_event` is reserved, never wave 1.
        if trigger.split(", ").any(|t| t == "on_event") && wave != "réservé" {
            return Err(format!(
                "{at}: `on_event` n'est permis qu'en vague `réservé`"
            ));
        }
        if wave == "réservé" && trigger != "on_event" {
            return Err(format!(
                "{at}: la vague `réservé` ne vaut que pour `on_event`"
            ));
        }
        match family.as_str() {
            "opération" => {
                // R1
                if !matches!(class.as_str(), "S" | "D" | "N") {
                    return Err(format!("{at}: une opération a une classe Φ S, D ou N"));
                }
                if usage == "n/a" {
                    return Err(format!("{at}: condition d'usage obligatoire"));
                }
                // R2
                if class == "D" && !usage.contains("résol") {
                    return Err(format!(
                        "{at}: une opération D dit quand elle se résout (`résol`)"
                    ));
                }
                // R3
                if class == "N" && wave == "1" {
                    return Err(format!("{at}: opération N interdite en vague 1"));
                }
                // R4
                if class == "S" && trigger != "continuous" {
                    return Err(format!("{at}: une opération S est `continuous` seulement"));
                }
                if selector != "n/a" && !all_in_list(&selector, &SELECTORS) {
                    return Err(format!("{at}: sélecteur `{selector}` hors liste fermée"));
                }
                if wave == "1" {
                    operations.insert(name);
                }
            }
            "sélecteur" => {
                if !in_list(&name, &SELECTORS) {
                    return Err(format!("{at}: sélecteur hors liste fermée"));
                }
                selectors.insert(name);
            }
            _ => {}
        }
    }
    // R6
    for op in WAVE1_OPERATIONS {
        if !operations.contains(op) {
            return Err(format!("opération de vague 1 `{op}` absente du tableau"));
        }
    }
    for sel in SELECTORS {
        if !selectors.contains(sel) {
            return Err(format!("sélecteur `{sel}` absent du tableau"));
        }
    }
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
