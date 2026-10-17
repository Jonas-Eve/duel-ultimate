//! The library reads no clock: "same batch, same report" depends on it. The one clock of the
//! crate is `Instant` in `src/main.rs` (the `--timing` line on stderr, never in the report).

use std::fs;
use std::path::PathBuf;

/// What a source of the library must not contain.
const CLOCK_WORDS: [&str; 4] = ["std::time", "Instant", "SystemTime", "chrono"];

/// The clock words found in `source`.
fn clock_words_in(source: &str) -> Vec<&'static str> {
    CLOCK_WORDS
        .into_iter()
        .filter(|word| source.contains(word))
        .collect()
}

fn src_dir() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("src")
}

#[test]
fn the_library_reads_no_clock() {
    let mut scanned = 0;
    for entry in fs::read_dir(src_dir()).expect("the src directory is readable") {
        let path = entry.expect("a directory entry").path();
        if path.extension().is_none_or(|ext| ext != "rs") || path.file_name().unwrap() == "main.rs"
        {
            continue;
        }
        let source = fs::read_to_string(&path).expect("a readable source");
        assert!(
            clock_words_in(&source).is_empty(),
            "{} mentions a clock: {:?}",
            path.display(),
            clock_words_in(&source)
        );
        scanned += 1;
    }
    assert!(scanned >= 3, "the scan saw only {scanned} library files");
}

#[test]
fn the_scan_detects_a_clock() {
    for sample in [
        "use std::time::Duration;",
        "let t = Instant::now();",
        "let t = SystemTime::now();",
        "use chrono::Utc;",
    ] {
        assert!(!clock_words_in(sample).is_empty(), "{sample}");
    }
    assert!(clock_words_in("let games = 3; // no clock here").is_empty());
}

#[test]
fn the_clock_lives_in_the_binary() {
    let main = fs::read_to_string(src_dir().join("main.rs")).expect("main.rs is readable");
    assert!(main.contains("Instant"));
}
