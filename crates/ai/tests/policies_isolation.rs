//! Architecture guard: a policy sees the game only through its `Observation` argument. The
//! signature cannot be bypassed from another crate, but a `use` inside this one could, so the
//! sources of `src/policies/` must never name the state or the engine's entry points. The
//! harness (`choose`) lives outside that folder.

use std::fs;
use std::path::Path;

/// The state and the engine's entry points (a policy only sees its `Observation`), then what
/// would put a card-specific rule or floating point (native and WASM could then disagree) into
/// a policy.
const FORBIDDEN: [&str; 11] = [
    "GameState",
    "apply",
    "legal_actions",
    "preview_legal_actions",
    ".name",
    "f32",
    "f64",
    "powi",
    "sqrt",
    "ln(",
    "exp(",
];

fn sources(dir: &Path, found: &mut Vec<(String, String)>) {
    for entry in fs::read_dir(dir).expect("policies/ is readable") {
        let path = entry.expect("a directory entry").path();
        if path.is_dir() {
            sources(&path, found);
        } else if path.extension().is_some_and(|ext| ext == "rs") {
            found.push((
                path.display().to_string(),
                fs::read_to_string(&path).expect("a readable source"),
            ));
        }
    }
}

#[test]
fn no_policy_source_names_the_state_or_the_engine_entry_points() {
    let dir = Path::new(env!("CARGO_MANIFEST_DIR")).join("src/policies");
    let mut files = Vec::new();
    sources(&dir, &mut files);
    assert!(!files.is_empty(), "policies/ holds at least random.rs");
    for (path, text) in files {
        for word in FORBIDDEN {
            assert!(!text.contains(word), "{path} mentions {word}");
        }
    }
}
