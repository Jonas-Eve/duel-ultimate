//! Guard of #208 (criteria 2 and 9 of #137): the engine never reads a card by its name or by its
//! tags. A source scan of production code, on the model of `effective_is_the_only_reader.rs`: a
//! read of `.name` or `.tags` in `crates/engine/src/*.rs` is refused outside an explicit
//! per-field allow-list (presentation only). The spec of a card (`CardSpec`) carries no card
//! identifier either, so `Card.id` never enters the engine.
//!
//! Allow-list (task 4 of #137 reuses it as is):
//! - `.name`: `view.rs` (the name of a card, of a hero), `setup.rs` (pairs the name with the
//!   card) and `data.rs`;
//! - `.tags`: `setup.rs` (copies the tags of the spec into the extras), `state.rs` (the body of
//!   the `tags()` accessor) and `view.rs` (the only caller of that accessor).
//!
//! Scanned: the text before the first `#[cfg(test)]` of each file, `//` comments dropped;
//! `testing.rs` skipped (compiled under `cfg(test)` only).

use std::fs;
use std::path::PathBuf;

const SKIPPED_FILES: [&str; 1] = ["testing.rs"];
const NAME_ALLOWED: [&str; 3] = ["view.rs", "setup.rs", "data.rs"];
const TAGS_ALLOWED: [&str; 3] = ["setup.rs", "state.rs", "view.rs"];

fn production_text(source: &str) -> &str {
    source
        .find("#[cfg(test)]")
        .map_or(source, |end| &source[..end])
}

/// Byte offsets of `.field` followed by a non-identifier character, whitespace (newlines included)
/// allowed between the dot and the word (rustfmt may split an access over lines).
fn reads_of(code: &str, field: &str) -> Vec<usize> {
    let bytes = code.as_bytes();
    let mut found = Vec::new();
    for (start, _) in code.match_indices('.') {
        let mut i = start + 1;
        while i < bytes.len() && bytes[i].is_ascii_whitespace() {
            i += 1;
        }
        let word: String = code[i..]
            .chars()
            .take_while(|c| c.is_alphanumeric() || *c == '_')
            .collect();
        if word == field {
            found.push(start);
        }
    }
    found
}

/// The offending lines of one file's production text, as `file:line: text`.
fn offences(file: &str, text: &str) -> Vec<String> {
    let code: String = text
        .lines()
        .map(|line| line.split("//").next().unwrap_or(""))
        .collect::<Vec<_>>()
        .join("\n");
    let mut at = Vec::new();
    if !NAME_ALLOWED.contains(&file) {
        at.extend(reads_of(&code, "name"));
    }
    if !TAGS_ALLOWED.contains(&file) {
        at.extend(reads_of(&code, "tags"));
    }
    at.sort_unstable();
    at.into_iter()
        .map(|i| {
            let line = code[..i].matches('\n').count();
            format!(
                "{file}:{}: {}",
                line + 1,
                code.lines().nth(line).unwrap_or("").trim()
            )
        })
        .collect()
}

#[test]
fn the_engine_never_reads_names_or_tags() {
    let dir = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("src");
    let mut found = Vec::new();
    let mut scanned = 0;
    for entry in fs::read_dir(&dir).expect("src directory") {
        let path = entry.expect("entry").path();
        let name = path.file_name().unwrap().to_string_lossy().into_owned();
        if !name.ends_with(".rs") || SKIPPED_FILES.contains(&name.as_str()) {
            continue;
        }
        let source = fs::read_to_string(&path).expect("readable source");
        found.extend(offences(&name, production_text(&source)));
        scanned += 1;
    }
    assert!(
        scanned >= 8,
        "the scan must read the engine sources, read {scanned}"
    );
    assert!(
        found.is_empty(),
        "reads of `.name` or `.tags` outside the allow-list (presentation only):\n{}",
        found.join("\n")
    );
}

#[test]
fn the_scan_itself_reads_what_it_should() {
    // A fabricated source that reads the tags or the name in a rule file is rejected...
    for (file, bad) in [
        ("rules.rs", "let t = card.tags;"),
        ("rules.rs", "card.tags.contains(x)"),
        ("effects.rs", "card\n    .tags\n    .len()"),
        ("rules.rs", "let n = card . tags ;"),
        ("combat.rs", "if card.name == x {}"),
        ("rules.rs", "let n = hero.name.clone();"),
        ("state.rs", "card.name.is_some()"),
        ("observation.rs", "card.tags()"),
        ("data.rs", "card.tags"),
    ] {
        assert!(!offences(file, bad).is_empty(), "missed: {file}: {bad}");
    }
    // ...the same read passes in the file the allow-list names for that field.
    assert!(offences("view.rs", "card.tags").is_empty());
    assert!(offences("setup.rs", "spec.tags").is_empty());
    assert!(offences("state.rs", "e.tags").is_empty());
    assert!(offences("view.rs", "card.name").is_empty());
    assert!(offences("data.rs", "x.name").is_empty());
    // The allow-list is per field: a file allowed for one is not allowed for the other.
    assert!(!offences("data.rs", "x.tags").is_empty());
    assert!(!offences("state.rs", "x.name").is_empty());
    // A longer identifier is not the field, and a comment or a test is not production text.
    assert!(offences("rules.rs", "card.tags_len + card.names").is_empty());
    assert!(offences("rules.rs", "// card.tags\nlet x = 1;").is_empty());
    assert_eq!(production_text("a\n#[cfg(test)]\nc.tags"), "a\n");
}
