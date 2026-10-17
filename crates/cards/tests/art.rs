use duel_cards::{
    ArtError, ArtFormat, ArtLicense, MAX_ART_AUTHOR_LEN, MAX_ART_DOC_BYTES, MAX_ART_SOURCE_LEN,
    parse_art, parse_cards,
};

const CARDS: &str = r#"[
    {"kind":"creature","id":"raider","name":"Raider","cost":1,"force":1,"attack_cost":1,"health":1},
    {"kind":"creature","id":"wall","name":"Wall","cost":1,"force":1,"attack_cost":1,"health":1}
]"#;

fn cards() -> Vec<duel_cards::Card> {
    parse_cards(CARDS).unwrap()
}

fn entry(id: &str, file: &str, author: &str, license: &str, source: &str) -> String {
    format!(
        r#"{{"id":"{id}","file":"{file}","author":"{author}","license":"{license}","source":"{source}"}}"#
    )
}

fn one(file: &str) -> String {
    format!("[{}]", entry("raider", file, "Ann", "CC0-1.0", "own work"))
}

fn parse(json: &str) -> Result<Vec<duel_cards::ArtEntry>, ArtError> {
    parse_art(json, &cards())
}

#[test]
fn nominal_case() {
    let json = format!(
        "[{},{}]",
        entry("raider", "raider.svg", "Ann", "CC0-1.0", "own work"),
        entry(
            "wall",
            "wall.webp",
            "Bob",
            "CC-BY-4.0",
            "https://example.org/x"
        )
    );
    let art = parse(&json).unwrap();
    assert_eq!(art.len(), 2);
    assert_eq!(art[0].id, "raider");
    assert_eq!(art[0].format(), ArtFormat::Svg);
    assert_eq!(art[0].license, ArtLicense::Cc0);
    assert_eq!(art[0].license.spdx(), "CC0-1.0");
    assert_eq!(art[1].format(), ArtFormat::Webp);
    assert_eq!(art[1].license.spdx(), "CC-BY-4.0");
}

#[test]
fn empty_array_is_valid() {
    assert_eq!(parse("[]"), Ok(vec![]));
}

#[test]
fn invalid_json_and_unknown_fields() {
    assert!(matches!(parse("{"), Err(ArtError::Json(_))));
    assert!(matches!(parse(r#"{"a":1}"#), Err(ArtError::Json(_))));
    let extra =
        r#"[{"id":"raider","file":"r.svg","author":"A","license":"CC0-1.0","source":"s","x":1}]"#;
    assert!(matches!(parse(extra), Err(ArtError::Json(_))));
}

#[test]
fn document_size_limit_is_exact_and_checked_before_parsing() {
    let ok = format!("[]{}", " ".repeat(MAX_ART_DOC_BYTES - 2));
    assert_eq!(ok.len(), MAX_ART_DOC_BYTES);
    assert_eq!(parse(&ok), Ok(vec![]));
    let big = "x".repeat(MAX_ART_DOC_BYTES + 1);
    assert_eq!(
        parse(&big),
        Err(ArtError::DocumentTooLarge {
            len: MAX_ART_DOC_BYTES + 1,
            max: MAX_ART_DOC_BYTES
        })
    );
}

#[test]
fn duplicate_art_id() {
    let json = format!(
        "[{},{}]",
        entry("raider", "a.svg", "A", "CC0-1.0", "s"),
        entry("raider", "b.svg", "A", "CC0-1.0", "s")
    );
    assert_eq!(parse(&json), Err(ArtError::DuplicateArt("raider".into())));
}

#[test]
fn unknown_card() {
    let json = format!("[{}]", entry("ghost", "a.svg", "A", "CC0-1.0", "s"));
    assert_eq!(parse(&json), Err(ArtError::UnknownCard("ghost".into())));
}

#[test]
fn duplicate_file() {
    let json = format!(
        "[{},{}]",
        entry("raider", "a.svg", "A", "CC0-1.0", "s"),
        entry("wall", "a.svg", "A", "CC0-1.0", "s")
    );
    assert_eq!(parse(&json), Err(ArtError::DuplicateFile("a.svg".into())));
}

#[test]
fn file_name_rule() {
    let longest = format!("a{}.svg", "b".repeat(63));
    assert_eq!(longest.len(), 64 + 4);
    assert!(parse(&one(&longest)).is_ok());
    assert!(parse(&one("0_a-b.webp")).is_ok());
    let too_long = format!("a{}.svg", "b".repeat(64));
    for bad in [
        too_long.as_str(),
        "",
        ".svg",
        "a.png",
        "a.SVG",
        "A.svg",
        "-a.svg",
        "_a.svg",
        "a b.svg",
        "a/b.svg",
        "../a.svg",
        "a\\n.svg",
        "a.svg\\n",
        "a.svg.svg",
        "é.svg",
        "a",
        "a.webp.exe",
    ] {
        assert_eq!(
            parse(&one(bad)),
            Err(ArtError::InvalidFile {
                id: "raider".into(),
                file: bad.replace("\\n", "\n")
            }),
            "{bad:?}"
        );
    }
}

#[test]
fn unknown_license() {
    let json = format!("[{}]", entry("raider", "a.svg", "A", "MIT", "s"));
    assert_eq!(
        parse(&json),
        Err(ArtError::UnknownLicense {
            id: "raider".into(),
            license: "MIT".into()
        })
    );
}

#[test]
fn empty_fields() {
    for (author, source, field) in [("", "s", "author"), ("A", "", "source")] {
        let json = format!("[{}]", entry("raider", "a.svg", author, "CC0-1.0", source));
        assert_eq!(
            parse(&json),
            Err(ArtError::EmptyField {
                id: "raider".into(),
                field
            })
        );
    }
}

#[test]
fn field_length_limits_are_exact_and_count_characters() {
    let author = "é".repeat(MAX_ART_AUTHOR_LEN);
    let source = "é".repeat(MAX_ART_SOURCE_LEN);
    let ok = format!(
        "[{}]",
        entry("raider", "a.svg", &author, "CC0-1.0", &source)
    );
    assert!(parse(&ok).is_ok());

    let author = "é".repeat(MAX_ART_AUTHOR_LEN + 1);
    let json = format!("[{}]", entry("raider", "a.svg", &author, "CC0-1.0", "s"));
    assert_eq!(
        parse(&json),
        Err(ArtError::FieldTooLong {
            id: "raider".into(),
            field: "author",
            len: MAX_ART_AUTHOR_LEN + 1,
            max: MAX_ART_AUTHOR_LEN
        })
    );
    let source = "é".repeat(MAX_ART_SOURCE_LEN + 1);
    let json = format!("[{}]", entry("raider", "a.svg", "A", "CC0-1.0", &source));
    assert_eq!(
        parse(&json),
        Err(ArtError::FieldTooLong {
            id: "raider".into(),
            field: "source",
            len: MAX_ART_SOURCE_LEN + 1,
            max: MAX_ART_SOURCE_LEN
        })
    );
}

#[test]
fn control_characters_are_rejected() {
    for (author, source, field) in [
        ("A\\u0007", "s", "author"),
        ("A", "s\\nt", "source"),
        ("A", "s\\u007f", "source"),
    ] {
        let json = format!("[{}]", entry("raider", "a.svg", author, "CC0-1.0", source));
        assert_eq!(
            parse(&json),
            Err(ArtError::ControlCharacter {
                id: "raider".into(),
                field
            }),
            "{author:?} {source:?}"
        );
    }
}
