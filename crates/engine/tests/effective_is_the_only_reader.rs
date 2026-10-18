//! Guard of #185: `effects::effective` is the only reader of the effective values of a card
//! (cost, force, attack cost, riposte, provocation). A source scan of production code, so that
//! #131 cannot forget a reader: a new `.stats.force` read fails here, silently wrong otherwise.
//! The current health (`stats.health`) is not an effective value and stays read directly.
//!
//! Scanned: `crates/engine/src/*.rs`, the text before the first `#[cfg(test)]` of each file.
//! Skipped: `testing.rs` (compiled under `cfg(test)` only), and, in `effects.rs`, the body of
//! `effective` alone.

use std::fs;
use std::path::PathBuf;

const SKIPPED_FILES: [&str; 1] = ["testing.rs"];
/// The files that carry the `CreatureStats` type: the only ones that may name it as a pattern.
const STATS_TYPE_FILES: [&str; 2] = ["setup.rs", "state.rs"];
const EFFECTIVE_SIGNATURE: &str = "pub(crate) fn effective(";

/// The production text of a file: everything before the first `#[cfg(test)]`.
fn production_text(source: &str) -> &str {
    source
        .find("#[cfg(test)]")
        .map_or(source, |end| &source[..end])
}

/// `effects.rs` without the function `effective` (from its signature to the first closing brace
/// at the start of a line): the one place allowed to read the raw fields.
fn without_effective(text: &str) -> String {
    let Some(start) = text.find(EFFECTIVE_SIGNATURE) else {
        return text.to_owned();
    };
    let end = text[start..]
        .find("\n}\n")
        .map_or(text.len(), |offset| start + offset + 3);
    format!("{}{}", &text[..start], &text[end..])
}

/// The offending lines of one file's production text, as `file:line: text`. The text is scanned
/// as a whole (the access may be split over lines by rustfmt), after dropping `//` comments.
fn offences(file: &str, text: &str) -> Vec<String> {
    let text = if file == "effects.rs" {
        without_effective(text)
    } else {
        text.to_owned()
    };
    let code: String = text
        .lines()
        .map(|line| line.split("//").next().unwrap_or(""))
        .collect::<Vec<_>>()
        .join("\n");
    let may_name_the_type = STATS_TYPE_FILES.contains(&file);
    let mut at = raw_field_reads(&code);
    if !may_name_the_type {
        at.extend(code.match_indices("CreatureStats {").map(|(i, _)| i));
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

/// Byte offsets of `.stats . (force|attack_cost|riposte|provocation|cost)` at a word boundary,
/// whitespace (newlines included) allowed around the dot.
fn raw_field_reads(code: &str) -> Vec<usize> {
    let bytes = code.as_bytes();
    let skip = |mut i: usize| {
        while i < bytes.len() && bytes[i].is_ascii_whitespace() {
            i += 1;
        }
        i
    };
    let mut found = Vec::new();
    for (start, _) in code.match_indices(".stats") {
        let i = skip(start + ".stats".len());
        if bytes.get(i) != Some(&b'.') {
            continue;
        }
        let i = skip(i + 1);
        let word: String = code[i..]
            .chars()
            .take_while(|c| c.is_alphanumeric() || *c == '_')
            .collect();
        if ["force", "attack_cost", "riposte", "provocation", "cost"].contains(&word.as_str()) {
            found.push(start);
        }
    }
    found
}

#[test]
fn effective_is_the_only_reader() {
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
        "raw reads of effective values (go through effects::effective):\n{}",
        found.join("\n")
    );
}

#[test]
fn the_scan_itself_reads_what_it_should() {
    // A fabricated source with every forbidden spelling fails the scan.
    for bad in [
        "let f = card.stats.force;",
        "let f = card.stats . attack_cost;",
        "x.stats\n    .riposte",
        "c.stats.provocation",
        "c.stats.cost + 1",
    ] {
        assert!(!offences("rules.rs", bad).is_empty(), "missed: {bad}");
    }
    assert!(!offences("rules.rs", "let s = CreatureStats { cost: 1 };").is_empty());
    // Allowed: the health, the type in the files that carry it, `effective` itself.
    assert!(offences("rules.rs", "c.stats.health").is_empty());
    assert!(offences("rules.rs", "c.stats.force_field").is_empty());
    assert!(offences("state.rs", "CreatureStats { cost: 1 }").is_empty());
    let effects = "fn a() {}\npub(crate) fn effective(s: &S, c: &C) -> E {\n    E { force: c.stats.force }\n}\nfn b() {}\n";
    assert!(offences("effects.rs", effects).is_empty());
    // ...but only `effective`: a read next to it in the same file fails, and so does any other file.
    let leaking = format!("{effects}fn c() {{ c.stats.cost }}\n");
    assert_eq!(offences("effects.rs", &leaking).len(), 1);
    assert!(!offences("view.rs", effects).is_empty());
    // The text of tests is cut off.
    assert_eq!(production_text("a\n#[cfg(test)]\nc.stats.force"), "a\n");
}
