//! Cross-check of the catalog table of `docs/design/effets.md` (#125) against the Rust closed
//! sets of `duel-cards` (acceptance criterion 3 of #137): an operation, a selector, a trigger, a
//! duration or a keyword added on one side only breaks this test. The table reader is a copy of
//! the one of `effects_catalog.rs` (which stays untouched); the vocabulary of the document
//! (`créature`, `unité`, `(camp)`, CamelCase operation names) is normalized by `DOC_TO_DATA`, so
//! the test passes whichever word the catalog uses (#204 renames it).
use duel_cards::{
    CardsError, DURATION_NAMES, KEYWORD_NAMES, OP_NAMES, OpKind, PhiClass, SELECT_NAMES,
    SelectorKind, TRIGGER_NAMES, parse_card_set,
};
use std::collections::BTreeSet;

const CATALOG: &str = include_str!("../../../docs/design/effets.md");

const SECTION_TITLE: &str = "## Tableau du catalogue";
const JSON_SECTION: &str = "Écriture JSON (#137)";

/// How a `DOC_TO_DATA` entry applies to a cell.
#[derive(Clone, Copy, PartialEq)]
enum Match {
    /// The cell is exactly this word (a type of card).
    Word,
    /// The word is a fragment of the cell (a selector), longest first.
    Fragment,
}

/// Vocabulary of the document -> name of the data.
const DOC_TO_DATA: [(&str, &str, Match); 13] = [
    ("créature", "unit", Match::Word),
    ("unité", "unit", Match::Word),
    ("bâtiment", "building", Match::Word),
    ("sort", "spell", Match::Word),
    ("équipement", "equipment", Match::Word),
    ("créatures", "units", Match::Fragment),
    ("creatures", "units", Match::Fragment),
    ("unités", "units", Match::Fragment),
    ("créature", "unit", Match::Fragment),
    ("creature", "unit", Match::Fragment),
    ("unité", "unit", Match::Fragment),
    ("(camp)", "", Match::Fragment),
    ("n/a", "n/a", Match::Word),
];

/// Normalizes a cell of the catalog to the vocabulary of the data.
fn normalize(cell: &str) -> String {
    let cell = cell.trim();
    if let Some((_, data, _)) = DOC_TO_DATA
        .iter()
        .find(|(doc, _, how)| *how == Match::Word && *doc == cell)
    {
        return (*data).to_owned();
    }
    DOC_TO_DATA
        .iter()
        .filter(|(_, _, how)| *how == Match::Fragment)
        .fold(cell.to_owned(), |s, (doc, data, _)| s.replace(doc, data))
}

/// `DealDamage` -> `deal_damage`.
fn snake(name: &str) -> String {
    let mut out = String::new();
    for (i, c) in name.chars().enumerate() {
        if c.is_uppercase() && i > 0 {
            out.push('_');
        }
        out.extend(c.to_lowercase());
    }
    out
}

#[derive(Debug, Clone)]
struct Row {
    name: String,
    family: String,
    trigger: String,
    selector: String,
    class: String,
    wave: String,
}

fn cells(line: &str) -> Vec<String> {
    let t = line.trim();
    let t = t.strip_prefix('|').unwrap_or(t);
    let t = t.strip_suffix('|').unwrap_or(t);
    t.split('|').map(|c| c.trim().to_string()).collect()
}

fn rows(md: &str) -> Result<Vec<Row>, String> {
    let lines: Vec<&str> = md.lines().collect();
    let start = lines
        .iter()
        .position(|l| l.trim_end() == SECTION_TITLE)
        .ok_or_else(|| format!("section `{SECTION_TITLE}` manquante"))?;
    let mut out = Vec::new();
    for l in lines.iter().skip(start + 1) {
        if l.starts_with("## ") {
            break;
        }
        if !l.trim_start().starts_with('|') {
            continue;
        }
        let c = cells(l);
        if c.len() != 8 {
            return Err(format!("ligne à {} cellules: {l}", c.len()));
        }
        // Header and separator lines.
        if c[0] == "élément" || c[0].chars().all(|ch| ch == '-' || ch == ':') {
            continue;
        }
        out.push(Row {
            name: c[0].clone(),
            family: c[1].clone(),
            trigger: c[2].clone(),
            selector: c[3].clone(),
            class: c[6].clone(),
            wave: c[7].clone(),
        });
    }
    Ok(out)
}

fn set(values: impl IntoIterator<Item = String>) -> BTreeSet<String> {
    values.into_iter().collect()
}

fn names(list: &[&str]) -> BTreeSet<String> {
    list.iter().map(|s| (*s).to_owned()).collect()
}

fn comma_set(cell: &str) -> BTreeSet<String> {
    cell.split(", ").map(normalize).collect()
}

fn phi_name(class: PhiClass) -> &'static str {
    match class {
        PhiClass::S => "S",
        PhiClass::D => "D",
        PhiClass::N => "N",
    }
}

/// What the catalog calls wave 2 and later, with the wire name a later wave would give it. Every
/// such row must be listed (the owner of a later wave updates this table) and the candidate must
/// be refused by `parse_card_set`.
#[derive(Clone, Copy)]
enum Where {
    Op,
    Trigger,
    EffectField,
    Keyword,
    Kind,
}

const DEFERRED: [(&str, &str, Where); 12] = [
    ("Soin", "heal", Where::Op),
    ("Bouclier", "shield", Where::Op),
    ("AuraDeVie", "life_aura", Where::Op),
    ("Résurrection", "resurrect", Where::Op),
    ("RetourEnMain", "return_to_hand", Where::Op),
    ("Contrôle", "control", Where::Op),
    ("FixerA", "set_to", Where::Op),
    ("Multiplier", "multiply", Where::Op),
    ("on_event", "on_event", Where::Trigger),
    ("when", "when", Where::EffectField),
    ("paratonnerre", "paratonnerre", Where::Keyword),
    ("équipement", "equipment", Where::Kind),
];

fn parse(json: &str) -> Result<(), CardsError> {
    parse_card_set(json, &["bâtiment".to_owned()]).map(|_| ())
}

const DRAW: &str = r#"{"op":"draw","target":{"select":"hero","side":"own"},"count":1}"#;

fn unit_with(effect: &str) -> String {
    format!(
        r#"[{{"id":"a","name":"A","cost":2,"force":3,"attack_cost":2,"health":5,"effects":[{effect}]}}]"#
    )
}

fn candidate_json(candidate: &str, place: Where) -> String {
    match place {
        Where::Op => unit_with(&format!(
            r#"{{"trigger":"on_play","ops":[{{"op":"{candidate}","target":{{"select":"hero","side":"own"}},"amount":1}}]}}"#
        )),
        Where::Trigger => unit_with(&format!(r#"{{"trigger":"{candidate}","ops":[{DRAW}]}}"#)),
        Where::EffectField => unit_with(&format!(
            r#"{{"trigger":"on_play","{candidate}":"x","ops":[{DRAW}]}}"#
        )),
        Where::Keyword => unit_with(&format!(
            r#"{{"trigger":"continuous","ops":[{{"op":"grant_keyword","target":{{"select":"self"}},"keyword":"{candidate}"}}]}}"#
        )),
        Where::Kind => format!(
            r#"[{{"kind":"{candidate}","id":"a","name":"A","cost":2,"force":3,"attack_cost":2,"health":5}}]"#
        ),
    }
}

/// The kinds the catalog may call a type of card, and whether `parse_card_set` accepts them.
const KINDS: [(&str, bool); 4] = [
    ("unit", true),
    ("spell", true),
    ("building", false),
    ("equipment", false),
];

fn kind_json(kind: &str) -> String {
    match kind {
        "spell" => format!(
            r#"[{{"kind":"spell","id":"a","name":"A","cost":2,"effects":[{{"trigger":"on_play","ops":[{DRAW}]}}]}}]"#
        ),
        other => format!(
            r#"[{{"kind":"{other}","id":"a","name":"A","cost":2,"force":3,"attack_cost":2,"health":5}}]"#
        ),
    }
}

/// The cross-check itself; the error names what differs.
fn cross_check(md: &str) -> Result<(), String> {
    let rows = rows(md)?;
    let wave1 = |family: &str| -> Vec<&Row> {
        rows.iter()
            .filter(|r| r.family == family && r.wave == "1")
            .collect()
    };

    // (1) the operations of wave 1 are `OP_NAMES`, in both directions.
    let ops = wave1("opération");
    let doc_ops = set(ops.iter().map(|r| snake(&r.name)));
    if doc_ops != names(&OP_NAMES) {
        return Err(format!(
            "opérations: catalogue {doc_ops:?}, énumération {:?}",
            names(&OP_NAMES)
        ));
    }

    // (2) triggers, selectors and Phi class of each operation.
    for row in &ops {
        let name = snake(&row.name);
        let Some(kind) = OpKind::ALL.iter().find(|k| k.name() == name) else {
            return Err(format!("opération `{name}` sans `OpKind`"));
        };
        let triggers = set(kind.triggers().iter().map(|t| t.name().to_owned()));
        if comma_set(&row.trigger) != triggers {
            return Err(format!(
                "`{name}`: déclencheurs du catalogue {:?}, de l'énumération {triggers:?}",
                comma_set(&row.trigger)
            ));
        }
        let selectors = set(kind.selectors().iter().map(|s| s.name().to_owned()));
        if comma_set(&row.selector) != selectors {
            return Err(format!(
                "`{name}`: sélecteurs du catalogue {:?}, de l'énumération {selectors:?}",
                comma_set(&row.selector)
            ));
        }
        if row.class != phi_name(kind.phi_class()) {
            return Err(format!(
                "`{name}`: classe Φ du catalogue `{}`, de l'énumération `{}`",
                row.class,
                phi_name(kind.phi_class())
            ));
        }
    }
    for kind in SelectorKind::ALL {
        if !SELECT_NAMES.contains(&kind.name()) {
            return Err(format!("`SelectorKind::{kind:?}` hors de SELECT_NAMES"));
        }
    }

    // (3) the selectors, (4) triggers and durations, (5) the keyword.
    for (family, expected, label) in [
        ("sélecteur", names(&SELECT_NAMES), "sélecteurs"),
        ("déclencheur", names(&TRIGGER_NAMES), "déclencheurs"),
        ("durée", names(&DURATION_NAMES), "durées"),
        ("mot-clé", names(&KEYWORD_NAMES), "mots-clés"),
    ] {
        let doc = set(wave1(family).iter().map(|r| normalize(&r.name)));
        if doc != expected {
            return Err(format!(
                "{label}: catalogue {doc:?}, énumération {expected:?}"
            ));
        }
    }

    // (6) every row of wave 2+ or reserved is deferred, and its candidate is refused.
    for row in rows.iter().filter(|r| r.wave != "1") {
        let Some((_, candidate, place)) = DEFERRED.iter().find(|(n, _, _)| *n == row.name) else {
            return Err(format!(
                "`{}` (vague {}) n'est pas dans la table DEFERRED du test",
                row.name, row.wave
            ));
        };
        match parse(&candidate_json(candidate, *place)) {
            Err(CardsError::Json(_)) => {}
            other => {
                return Err(format!(
                    "`{}` (`{candidate}`) devrait être refusé par serde, obtenu {other:?}",
                    row.name
                ));
            }
        }
    }

    // (7) the types of card.
    for row in rows.iter().filter(|r| r.family == "type de carte") {
        let kind = normalize(&row.name);
        let Some((_, accepted)) = KINDS.iter().find(|(k, _)| *k == kind) else {
            return Err(format!("type de carte `{}` inconnu du test", row.name));
        };
        let result = parse(&kind_json(&kind));
        match (accepted, &result) {
            (true, Ok(())) | (false, Err(CardsError::Json(_))) => {}
            _ => {
                return Err(format!(
                    "type de carte `{}` ({kind}): accepté attendu {accepted}, obtenu {result:?}",
                    row.name
                ));
            }
        }
    }
    Ok(())
}

#[test]
fn the_catalog_table_matches_the_enumerations() {
    assert_eq!(cross_check(CATALOG), Ok(()));
}

#[test]
fn the_table_of_the_operations_as_the_test_reads_it() {
    // The values the cross-check compares, spelled out once (pasted in the PR).
    let expected = [
        ("attack", "activated, on_play", "hero_or_unit", "D"),
        (
            "deal_damage",
            "activated, on_play",
            "all_units, hero_or_unit, unit",
            "D",
        ),
        ("draw", "activated, on_play", "hero", "D"),
        ("modify_stat", "continuous", "all_units, self, unit", "S"),
        ("grant_keyword", "continuous", "all_units, self, unit", "S"),
    ];
    for (name, triggers, selectors, class) in expected {
        let kind = OpKind::ALL
            .iter()
            .find(|k| k.name() == name)
            .expect("an OpKind");
        let t: BTreeSet<String> = kind.triggers().iter().map(|t| t.name().to_owned()).collect();
        let s: BTreeSet<String> = kind.selectors().iter().map(|s| s.name().to_owned()).collect();
        assert_eq!(t, set(triggers.split(", ").map(str::to_owned)), "{name}");
        assert_eq!(s, set(selectors.split(", ").map(str::to_owned)), "{name}");
        assert_eq!(phi_name(kind.phi_class()), class, "{name}");
    }
}

#[test]
fn normalize_reads_both_vocabularies() {
    for (doc, data) in [
        ("hero_or_creature(camp)", "hero_or_unit"),
        ("hero_or_unit(camp)", "hero_or_unit"),
        ("all_creatures(camp)", "all_units"),
        ("all_units", "all_units"),
        ("créature", "unit"),
        ("unité", "unit"),
        ("bâtiment", "building"),
        ("sort", "spell"),
        ("équipement", "equipment"),
        ("self", "self"),
    ] {
        assert_eq!(normalize(doc), data, "{doc}");
    }
    assert_eq!(snake("DealDamage"), "deal_damage");
    assert_eq!(snake("Attack"), "attack");
}

#[test]
fn the_cross_check_itself_fails_on_a_doctored_table() {
    // An operation removed from the catalog.
    let draw = "| Draw | opération | on_play, activated | hero(camp) | instant |";
    let start = CATALOG.find(draw).expect("the Draw row");
    let end = start + CATALOG[start..].find('\n').expect("end of row") + 1;
    let removed = format!("{}{}", &CATALOG[..start], &CATALOG[end..]);
    let error = cross_check(&removed).unwrap_err();
    assert!(error.contains("opérations"), "{error}");

    // An operation added to the catalog only.
    let added = CATALOG.replacen(
        draw,
        &format!(
            "| Teleport | opération | on_play | hero(camp) | instant | cible résoluble | D | 1 |\n{draw}"
        ),
        1,
    );
    let error = cross_check(&added).unwrap_err();
    assert!(error.contains("opérations"), "{error}");

    // A trigger, a selector and a Phi class that differ.
    let moved = CATALOG.replacen(
        "| Draw | opération | on_play, activated |",
        "| Draw | opération | on_play |",
        1,
    );
    assert!(cross_check(&moved).unwrap_err().contains("déclencheurs"));
    let selector = CATALOG.replacen(
        "| Draw | opération | on_play, activated | hero(camp) |",
        "| Draw | opération | on_play, activated | hero(camp), self |",
        1,
    );
    assert!(cross_check(&selector).unwrap_err().contains("sélecteurs"));
    let class = CATALOG.replacen(
        "| pioche non vide : au moins une carte résoluble à piocher | D | 1 |",
        "| pioche non vide : au moins une carte résoluble à piocher | S | 1 |",
        1,
    );
    assert!(cross_check(&class).unwrap_err().contains("classe Φ"));

    // A selector row removed, a row of wave 2+ without an entry in DEFERRED.
    let row = "| self | sélecteur |";
    let start = CATALOG.find(row).expect("the self row");
    let end = start + CATALOG[start..].find('\n').expect("end of row") + 1;
    let no_self = format!("{}{}", &CATALOG[..start], &CATALOG[end..]);
    assert!(cross_check(&no_self).unwrap_err().contains("sélecteurs"));
    let unknown = CATALOG.replacen(
        "| Soin | opération |",
        "| Guérison | opération |",
        1,
    );
    assert!(cross_check(&unknown).unwrap_err().contains("DEFERRED"));
    // A card type the test does not know.
    let kind = CATALOG.replacen("| sort | type de carte |", "| piège | type de carte |", 1);
    assert!(cross_check(&kind).unwrap_err().contains("type de carte"));
}

#[test]
fn the_json_examples_of_the_design_doc_parse() {
    let lines: Vec<&str> = CATALOG.lines().collect();
    let start = lines
        .iter()
        .position(|l| l.starts_with("## ") && l.contains(JSON_SECTION))
        .expect("the section `Écriture JSON (#137)`");
    let mut blocks: Vec<String> = Vec::new();
    let mut current: Option<String> = None;
    for line in lines.iter().skip(start + 1) {
        if line.starts_with("## ") {
            break;
        }
        match (&mut current, line.trim_end()) {
            (None, "```json") => current = Some(String::new()),
            (Some(block), "```") => {
                blocks.push(std::mem::take(block));
                current = None;
            }
            (Some(block), text) => {
                block.push_str(text);
                block.push('\n');
            }
            (None, _) => {}
        }
    }
    assert!(current.is_none(), "an unterminated ```json block");
    assert!(blocks.len() >= 3, "{} blocks", blocks.len());
    for block in &blocks {
        let cards = parse_card_set(block, &["bâtiment".to_owned()])
            .unwrap_or_else(|e| panic!("{e}\n{block}"));
        assert!(!cards.is_empty());
    }
}
