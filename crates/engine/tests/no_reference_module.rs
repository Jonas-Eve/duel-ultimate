//! #186: the frozen oracle of #128 (a copy of the pre-refactoring rules, compared against the
//! new code) is gone for good; a differential against it must not come back by oversight.

use std::fs;
use std::path::PathBuf;

#[test]
fn no_reference_module_remains() {
    let src = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("src");
    let needle = ["mod ", "reference"].concat();
    let file = ["reference", ".rs"].concat();
    assert!(!src.join(&file).exists(), "{file} must not exist");
    let mut scanned = 0;
    for entry in fs::read_dir(&src).unwrap() {
        let path = entry.unwrap().path();
        if path.extension().is_some_and(|e| e == "rs") {
            let text = fs::read_to_string(&path).unwrap();
            assert!(!text.contains(&needle), "{}: `{needle}`", path.display());
            scanned += 1;
        }
    }
    assert!(scanned >= 10, "the scan saw only {scanned} files");
}
