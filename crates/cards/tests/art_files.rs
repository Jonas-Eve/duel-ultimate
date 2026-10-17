//! Validates every `data/<set>/<version>/art.json` and its `art/` folder on disk. The only
//! I/O of the art feature lives here; `parse_art` itself is pure.

use std::collections::BTreeSet;
use std::fs;
use std::path::{Path, PathBuf};

use duel_cards::{ArtEntry, ArtFormat, parse_art, parse_cards};
use roxmltree::{Document, Node, ParsingOptions};

const MAX_IMAGE_BYTES: u64 = 40 * 1024;
const MAX_TOTAL_BYTES: u64 = 400 * 1024;
const SVG_NS: &str = "http://www.w3.org/2000/svg";
const XLINK_NS: &str = "http://www.w3.org/1999/xlink";

const ALLOWED_ELEMENTS: &[&str] = &[
    "svg",
    "g",
    "defs",
    "title",
    "desc",
    "style",
    "use",
    "path",
    "rect",
    "circle",
    "ellipse",
    "line",
    "polyline",
    "polygon",
    "linearGradient",
    "radialGradient",
    "stop",
    "clipPath",
    "mask",
];

fn check_url_refs(text: &str) -> Result<(), String> {
    let lower = text.to_ascii_lowercase();
    if lower.contains("javascript:") {
        return Err("javascript: is forbidden".into());
    }
    if lower.contains("data:") {
        return Err("data: is forbidden".into());
    }
    if lower.contains("@import") {
        return Err("@import is forbidden".into());
    }
    let mut rest = lower.as_str();
    while let Some(i) = rest.find("url(") {
        let after = rest.get(i + 4..).unwrap_or("").trim_start();
        let after = after.trim_start_matches(['"', '\'']);
        if !after.starts_with('#') {
            return Err("url() may only reference #fragment".into());
        }
        rest = rest.get(i + 4..).unwrap_or("");
    }
    Ok(())
}

fn check_node(node: Node) -> Result<(), String> {
    let name = node.tag_name();
    if name.namespace() != Some(SVG_NS) {
        return Err(format!(
            "element `{}` is not in the SVG namespace",
            name.name()
        ));
    }
    if !ALLOWED_ELEMENTS.contains(&name.name()) {
        return Err(format!("element `{}` is not allowed", name.name()));
    }
    for attr in node.attributes() {
        let local = attr.name();
        if local.to_ascii_lowercase().starts_with("on") {
            return Err(format!("event attribute `{local}` is forbidden"));
        }
        if local == "href" && matches!(attr.namespace(), None | Some(SVG_NS) | Some(XLINK_NS)) {
            if !attr.value().starts_with('#') {
                return Err("href may only be a #fragment".into());
            }
        } else {
            check_url_refs(attr.value())?;
        }
    }
    if name.name() == "style" {
        for child in node.children() {
            check_url_refs(child.text().unwrap_or(""))?;
        }
    }
    node.children()
        .filter(Node::is_element)
        .try_for_each(check_node)
}

/// Defense in depth: an SVG is always loaded through `<img>`, never inline.
fn check_svg(text: &str) -> Result<(), String> {
    let options = ParsingOptions {
        allow_dtd: false,
        nodes_limit: 10_000,
        ..ParsingOptions::default()
    };
    let doc = Document::parse_with_options(text, options).map_err(|e| e.to_string())?;
    let root = doc.root_element();
    if root.tag_name().name() != "svg" || root.tag_name().namespace() != Some(SVG_NS) {
        return Err("root element must be an SVG `svg`".into());
    }
    if doc.descendants().any(|n| n.is_pi()) {
        return Err("processing instruction is forbidden".into());
    }
    if doc.root().children().filter(Node::is_element).count() != 1 {
        return Err("exactly one root element expected".into());
    }
    check_node(root)
}

fn check_webp(bytes: &[u8]) -> Result<(), String> {
    if bytes.get(0..4) == Some(b"RIFF") && bytes.get(8..12) == Some(b"WEBP") {
        Ok(())
    } else {
        Err("not a WebP file (RIFF....WEBP signature)".into())
    }
}

fn check_set_dir(dir: &Path) -> Result<(), String> {
    let art_json = dir.join("art.json");
    if !art_json.is_file() {
        return if dir.join("art").exists() {
            Err("art/ exists without art.json".into())
        } else {
            Ok(())
        };
    }
    let cards_json = fs::read_to_string(dir.join("cards.json")).map_err(|e| e.to_string())?;
    let cards = parse_cards(&cards_json).map_err(|e| e.to_string())?;
    let text = fs::read_to_string(&art_json).map_err(|e| e.to_string())?;
    let entries: Vec<ArtEntry> = parse_art(&text, &cards).map_err(|e| e.to_string())?;

    let art_dir = dir.join("art");
    let mut total = 0u64;
    let mut cited = BTreeSet::new();
    for entry in &entries {
        let path = art_dir.join(&entry.file);
        let meta = fs::symlink_metadata(&path).map_err(|e| format!("{}: {e}", entry.file))?;
        if !meta.file_type().is_file() {
            return Err(format!("{}: not a regular file", entry.file));
        }
        if meta.len() > MAX_IMAGE_BYTES {
            return Err(format!(
                "{}: larger than {MAX_IMAGE_BYTES} bytes",
                entry.file
            ));
        }
        total += meta.len();
        let bytes = fs::read(&path).map_err(|e| e.to_string())?;
        match entry.format() {
            ArtFormat::Webp => check_webp(&bytes),
            ArtFormat::Svg => {
                let s = std::str::from_utf8(&bytes).map_err(|e| e.to_string())?;
                check_svg(s)
            }
        }
        .map_err(|e| format!("{}: {e}", entry.file))?;
        cited.insert(entry.file.clone());
    }
    if total > MAX_TOTAL_BYTES {
        return Err(format!("art/ totals more than {MAX_TOTAL_BYTES} bytes"));
    }
    if art_dir.exists() {
        for item in fs::read_dir(&art_dir).map_err(|e| e.to_string())? {
            let name = item
                .map_err(|e| e.to_string())?
                .file_name()
                .to_string_lossy()
                .into_owned();
            if !cited.contains(&name) {
                return Err(format!("orphan file in art/: {name}"));
            }
        }
    }
    Ok(())
}

fn data_root() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR")).join("../../data")
}

fn version_dirs(root: &Path) -> Vec<PathBuf> {
    let mut out = Vec::new();
    for set in fs::read_dir(root).unwrap().flatten() {
        if let Ok(versions) = fs::read_dir(set.path()) {
            for v in versions.flatten() {
                if v.path().join("cards.json").is_file() {
                    out.push(v.path());
                }
            }
        }
    }
    out.sort();
    out
}

#[test]
fn every_art_json_in_data_is_valid() {
    let dirs = version_dirs(&data_root());
    assert!(
        !dirs.is_empty(),
        "no data/<set>/<version>/cards.json found: wrong path, the test would be vacuous"
    );
    for dir in dirs {
        check_set_dir(&dir).unwrap_or_else(|e| panic!("{}: {e}", dir.display()));
    }
}

// ---- SVG rules, one crafted document per rule ----

const VALID_SVG: &str = r##"<svg xmlns="http://www.w3.org/2000/svg" viewBox="0 0 10 10"><title>t</title><defs><linearGradient id="g"><stop offset="0"/></linearGradient></defs><style>.a{fill:url(#g)}</style><rect width="10" height="10" fill="url(#g)"/><use href="#g"/></svg>"##;

fn svg_with(body: &str) -> String {
    format!(
        r#"<svg xmlns="http://www.w3.org/2000/svg" xmlns:xlink="http://www.w3.org/1999/xlink">{body}</svg>"#
    )
}

#[test]
fn minimal_valid_svg_passes() {
    assert_eq!(check_svg(VALID_SVG), Ok(()));
}

#[test]
fn svg_rejects_each_forbidden_construct() {
    let cases: Vec<(&str, String)> = vec![
        ("not xml", "<svg".into()),
        ("two roots", format!("{VALID_SVG}<svg xmlns=\"{SVG_NS}\"/>")),
        (
            "wrong root",
            "<html xmlns=\"http://www.w3.org/1999/xhtml\"/>".into(),
        ),
        ("no namespace", "<svg/>".into()),
        ("script", svg_with("<script>alert(1)</script>")),
        ("foreignObject", svg_with("<foreignObject/>")),
        ("image", svg_with("<image href=\"#a\"/>")),
        ("animate", svg_with("<animate attributeName=\"x\"/>")),
        ("animateTransform", svg_with("<animateTransform/>")),
        ("set", svg_with("<set attributeName=\"x\"/>")),
        ("a", svg_with("<a href=\"#x\"/>")),
        ("unknown element", svg_with("<blink/>")),
        ("onload", svg_with("<rect onload=\"x()\"/>")),
        ("onclick", svg_with("<rect onclick=\"x()\"/>")),
        (
            "external href",
            svg_with("<use href=\"http://a/b.svg#c\"/>"),
        ),
        (
            "external xlink:href",
            svg_with("<use xlink:href=\"b.svg#c\"/>"),
        ),
        (
            "javascript href",
            svg_with("<use href=\"JavaScript:alert(1)\"/>"),
        ),
        (
            "external url()",
            svg_with("<rect fill=\"url(http://a/b)\"/>"),
        ),
        (
            "data url()",
            svg_with("<rect fill=\"url(data:image/png;base64,AA)\"/>"),
        ),
        ("data: attr", svg_with("<rect fill=\"DATA:x\"/>")),
        (
            "style external url",
            svg_with("<style>.a{fill:url(http://a/b)}</style>"),
        ),
        (
            "style @import",
            svg_with("<style>@import 'http://a/b.css';</style>"),
        ),
        (
            "style import url",
            svg_with("<style>@IMPORT url(#a);</style>"),
        ),
        ("doctype", format!("<!DOCTYPE svg>{VALID_SVG}")),
        (
            "entity",
            format!("<!DOCTYPE svg [<!ENTITY e \"x\">]>{VALID_SVG}"),
        ),
    ];
    for (label, svg) in cases {
        assert!(check_svg(&svg).is_err(), "should be rejected: {label}");
    }
}

// ---- Folder rules, on a temporary directory (never inside data/) ----

struct TempDir(PathBuf);

impl TempDir {
    fn new(tag: &str) -> Self {
        let dir = std::env::temp_dir().join(format!("duel-art-{tag}-{}", std::process::id()));
        let _ = fs::remove_dir_all(&dir);
        fs::create_dir_all(dir.join("art")).unwrap();
        fs::write(
            dir.join("cards.json"),
            r#"[{"kind":"creature","id":"raider","name":"Raider","cost":1,"force":1,"attack_cost":1,"health":1}]"#,
        )
        .unwrap();
        Self(dir)
    }

    fn art_json(&self, file: &str) {
        let json = format!(
            r#"[{{"id":"raider","file":"{file}","author":"A","license":"CC0-1.0","source":"s"}}]"#
        );
        fs::write(self.0.join("art.json"), json).unwrap();
    }

    fn put(&self, name: &str, bytes: &[u8]) {
        fs::write(self.0.join("art").join(name), bytes).unwrap();
    }
}

impl Drop for TempDir {
    fn drop(&mut self) {
        let _ = fs::remove_dir_all(&self.0);
    }
}

fn webp(len: usize) -> Vec<u8> {
    let mut v = b"RIFF\0\0\0\0WEBP".to_vec();
    v.resize(len.max(12), 0);
    v
}

#[test]
fn folder_with_valid_svg_and_webp_passes() {
    let t = TempDir::new("ok");
    t.art_json("raider.svg");
    t.put("raider.svg", VALID_SVG.as_bytes());
    assert_eq!(check_set_dir(&t.0), Ok(()));
    t.art_json("raider.webp");
    fs::remove_file(t.0.join("art/raider.svg")).unwrap();
    t.put("raider.webp", &webp(100));
    assert_eq!(check_set_dir(&t.0), Ok(()));
}

#[test]
fn folder_rejects_missing_file() {
    let t = TempDir::new("missing");
    t.art_json("raider.svg");
    let err = check_set_dir(&t.0).unwrap_err();
    assert!(
        err.starts_with("raider.svg:") && err.contains("No such file"),
        "{err}"
    );
}

#[test]
fn folder_rejects_orphan() {
    let t = TempDir::new("orphan");
    t.art_json("raider.svg");
    t.put("raider.svg", VALID_SVG.as_bytes());
    t.put("extra.svg", VALID_SVG.as_bytes());
    assert!(check_set_dir(&t.0).unwrap_err().contains("orphan"));
}

#[test]
fn folder_rejects_bad_webp_signature() {
    let t = TempDir::new("sig");
    t.art_json("raider.webp");
    t.put("raider.webp", b"RIFF\0\0\0\0WEBX");
    let err = check_set_dir(&t.0).unwrap_err();
    assert!(err.contains("not a WebP file"), "{err}");
    t.put("raider.webp", b"RIFF");
    let err = check_set_dir(&t.0).unwrap_err();
    assert!(err.contains("not a WebP file"), "{err}");
}

#[test]
fn folder_enforces_image_size_limit() {
    let t = TempDir::new("big");
    t.art_json("raider.webp");
    t.put("raider.webp", &webp(MAX_IMAGE_BYTES as usize));
    assert_eq!(check_set_dir(&t.0), Ok(()));
    t.put("raider.webp", &webp(MAX_IMAGE_BYTES as usize + 1));
    let err = check_set_dir(&t.0).unwrap_err();
    assert!(err.contains("larger than"), "{err}");
}

#[cfg(unix)]
#[test]
fn folder_rejects_symlink_and_subdirectory() {
    let t = TempDir::new("link");
    t.art_json("raider.webp");
    t.put("real.webp", &webp(100));
    std::os::unix::fs::symlink(t.0.join("art/real.webp"), t.0.join("art/raider.webp")).unwrap();
    let err = check_set_dir(&t.0).unwrap_err();
    assert!(err.contains("not a regular file"), "{err}");
    fs::remove_file(t.0.join("art/raider.webp")).unwrap();
    fs::remove_file(t.0.join("art/real.webp")).unwrap();
    fs::create_dir(t.0.join("art/raider.webp")).unwrap();
    let err = check_set_dir(&t.0).unwrap_err();
    assert!(err.contains("not a regular file"), "{err}");
}

#[test]
fn folder_rejects_unsafe_svg_content() {
    let t = TempDir::new("svg");
    t.art_json("raider.svg");
    t.put("raider.svg", svg_with("<script/>").as_bytes());
    let err = check_set_dir(&t.0).unwrap_err();
    assert!(
        err.starts_with("raider.svg:") && err.contains("`script` is not allowed"),
        "{err}"
    );
}

/// A folder with `n` creatures `c0..c{n-1}`, each citing `c{i}.webp` of `sizes[i]` bytes.
fn many_webp_folder(tag: &str, sizes: &[usize]) -> TempDir {
    let t = TempDir::new(tag);
    let cards: Vec<String> = (0..sizes.len())
        .map(|i| {
            format!(
                r#"{{"kind":"creature","id":"c{i}","name":"C{i}","cost":1,"force":1,"attack_cost":1,"health":1}}"#
            )
        })
        .collect();
    fs::write(t.0.join("cards.json"), format!("[{}]", cards.join(","))).unwrap();
    let arts: Vec<String> = (0..sizes.len())
        .map(|i| {
            format!(
                r#"{{"id":"c{i}","file":"c{i}.webp","author":"A","license":"CC0-1.0","source":"s"}}"#
            )
        })
        .collect();
    fs::write(t.0.join("art.json"), format!("[{}]", arts.join(","))).unwrap();
    for (i, len) in sizes.iter().enumerate() {
        t.put(&format!("c{i}.webp"), &webp(*len));
    }
    t
}

#[test]
fn folder_enforces_total_size_limit() {
    let each = MAX_IMAGE_BYTES as usize;
    // 11 x 40 KiB = 440 KiB, every image within its own limit.
    let t = many_webp_folder("total-over", &[each; 11]);
    let err = check_set_dir(&t.0).unwrap_err();
    assert!(err.contains("totals more than"), "{err}");
    // 10 x 40 KiB = exactly 400 KiB passes.
    let t = many_webp_folder("total-exact", &[each; 10]);
    assert_eq!(check_set_dir(&t.0), Ok(()));
    // One byte over the limit fails (a 12-byte eleventh image makes 400 KiB + 12).
    let mut sizes = vec![each; 10];
    sizes.push(12);
    let t = many_webp_folder("total-plus", &sizes);
    let err = check_set_dir(&t.0).unwrap_err();
    assert!(err.contains("totals more than"), "{err}");
}

#[test]
fn folder_rejects_art_dir_without_art_json() {
    let t = TempDir::new("noartjson");
    t.put("raider.svg", VALID_SVG.as_bytes());
    let err = check_set_dir(&t.0).unwrap_err();
    assert!(err.contains("without art.json"), "{err}");
}

#[test]
fn folder_without_art_dir_nor_art_json_passes() {
    let t = TempDir::new("noart");
    fs::remove_dir_all(t.0.join("art")).unwrap();
    assert_eq!(check_set_dir(&t.0), Ok(()));
}

#[test]
fn svg_rejects_nodes_over_limit() {
    let body = "<g/>".repeat(10_001);
    let err = check_svg(&svg_with(&body)).unwrap_err();
    assert!(err.to_lowercase().contains("nodes limit"), "{err}");
    assert_eq!(check_svg(&svg_with(&"<g/>".repeat(100))), Ok(()));
}

#[test]
fn svg_rejects_processing_instructions() {
    for (label, svg) in [
        (
            "stylesheet before root",
            format!("<?xml-stylesheet href=\"http://a/b.css\"?>{VALID_SVG}"),
        ),
        ("inside root", svg_with("<?foo bar?>")),
        ("after root", format!("{VALID_SVG}<?foo bar?>")),
    ] {
        let err = check_svg(&svg).unwrap_err();
        assert!(err.contains("processing instruction"), "{label}: {err}");
    }
    assert_eq!(
        check_svg(&format!("<?xml version=\"1.0\"?>{VALID_SVG}")),
        Ok(())
    );
}
