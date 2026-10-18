//! `tags` (#206): a closed, per-data-set list of display texts (`tags.json`), validated by
//! `parse_tags`; a card carries at most four. Fail-closed: without a list, any tag is refused.
use duel_cards::{
    CardsError, MAX_DATASET_TAGS, MAX_TAG_LEN, MAX_TAGS_DOC_BYTES, MAX_TAGS_PER_CARD, TagsError,
    parse_card_set, parse_cards, parse_tags,
};

fn card_with(tags: &str) -> String {
    format!(
        r#"[{{"kind":"unit","id":"a","name":"A","cost":1,"force":1,"attack_cost":1,"health":1,"tags":{tags}}}]"#
    )
}

fn list(tags: &[&str]) -> Vec<String> {
    tags.iter().map(|t| (*t).to_owned()).collect()
}

fn invalid(source: CardsError) -> CardsError {
    CardsError::InvalidCard {
        id: "a".into(),
        source: Box::new(source),
    }
}

#[test]
fn tags_are_optional_and_omitted_when_empty() {
    let plain = r#"[{"kind":"unit","id":"a","name":"A","cost":1,"force":1,"attack_cost":1,"health":1}]"#;
    let cards = parse_cards(plain).unwrap();
    let written = serde_json::to_value(&cards[0]).unwrap();
    assert!(written.get("tags").is_none(), "{written}");
    let explicit = parse_cards(&card_with("[]")).unwrap();
    assert_eq!(cards, explicit);
}

#[test]
fn a_tag_is_text_with_accents_not_a_slug() {
    let tags = list(&["bâtiment", "Pièce rare"]);
    let cards = parse_card_set(&card_with(r#"["bâtiment","Pièce rare"]"#), &tags).unwrap();
    let written = serde_json::to_value(&cards[0]).unwrap();
    assert_eq!(written["tags"], serde_json::json!(["bâtiment", "Pièce rare"]));
}

#[test]
fn parse_cards_refuses_any_tag_without_a_list() {
    // Fail-closed: no list given means no tag allowed.
    assert_eq!(
        parse_cards(&card_with(r#"["bâtiment"]"#)).unwrap_err(),
        invalid(CardsError::UnknownTag("bâtiment".into()))
    );
    assert_eq!(
        parse_card_set(&card_with(r#"["bâtiment"]"#), &[]).unwrap_err(),
        invalid(CardsError::UnknownTag("bâtiment".into()))
    );
}

#[test]
fn a_tag_outside_the_list_is_refused() {
    assert_eq!(
        parse_card_set(&card_with(r#"["autre"]"#), &list(&["bâtiment"])).unwrap_err(),
        invalid(CardsError::UnknownTag("autre".into()))
    );
}

#[test]
fn a_tag_in_the_list_is_accepted() {
    let cards = parse_card_set(&card_with(r#"["bâtiment"]"#), &list(&["bâtiment", "x"])).unwrap();
    assert_eq!(cards.len(), 1);
    assert_eq!(cards[0].spec().creature(), &cards[0].stats());
}

#[test]
fn parse_tags_reads_an_array_of_strings() {
    assert_eq!(
        parse_tags(r#"["bâtiment","machine"]"#).unwrap(),
        list(&["bâtiment", "machine"])
    );
    assert_eq!(parse_tags("[]").unwrap(), Vec::<String>::new());
}

#[test]
fn parse_tags_bounds() {
    // The size is tested before parsing: invalid JSON of that size is `DocTooLarge`.
    let big = "x".repeat(MAX_TAGS_DOC_BYTES + 1);
    assert_eq!(
        parse_tags(&big).unwrap_err(),
        TagsError::DocTooLarge {
            len: MAX_TAGS_DOC_BYTES + 1,
            max: MAX_TAGS_DOC_BYTES
        }
    );
    let many = |n: usize| {
        let items: Vec<String> = (0..n).map(|i| format!("\"t{i}\"")).collect();
        format!("[{}]", items.join(","))
    };
    assert_eq!(parse_tags(&many(MAX_DATASET_TAGS)).unwrap().len(), MAX_DATASET_TAGS);
    assert_eq!(
        parse_tags(&many(MAX_DATASET_TAGS + 1)).unwrap_err(),
        TagsError::TooManyEntries {
            found: MAX_DATASET_TAGS + 1,
            max: MAX_DATASET_TAGS
        }
    );
    // Characters, not bytes: `é` is two bytes.
    let ok = format!("[\"{}\"]", "é".repeat(MAX_TAG_LEN));
    assert!(parse_tags(&ok).is_ok());
    let long = format!("[\"{}\"]", "é".repeat(MAX_TAG_LEN + 1));
    assert_eq!(
        parse_tags(&long).unwrap_err(),
        TagsError::InvalidTag(CardsError::TagTooLong {
            len: MAX_TAG_LEN + 1,
            max: MAX_TAG_LEN
        })
    );
    for (json, expected) in [
        (r#"[""]"#, CardsError::TagEmpty),
        (r#"["a\u0007b"]"#, CardsError::TagControlChar),
        (r#"["a\nb"]"#, CardsError::TagControlChar),
        (r#"[" a"]"#, CardsError::TagUntrimmed),
        (r#"["a "]"#, CardsError::TagUntrimmed),
    ] {
        assert_eq!(
            parse_tags(json).unwrap_err(),
            TagsError::InvalidTag(expected),
            "{json}"
        );
    }
    assert_eq!(
        parse_tags(r#"["a","b","a"]"#).unwrap_err(),
        TagsError::DuplicateTag("a".into())
    );
    for json in [r#"{"a":1}"#, r#"[1]"#, r#"["a",2]"#, r#""a""#, "not json"] {
        assert!(matches!(parse_tags(json), Err(TagsError::Json(_))), "{json}");
    }
}

#[test]
fn a_card_has_at_most_four_distinct_tags() {
    let all = list(&["a", "b", "c", "d", "e"]);
    let four = card_with(r#"["a","b","c","d"]"#);
    assert_eq!(MAX_TAGS_PER_CARD, 4);
    assert!(parse_card_set(&four, &all).is_ok());
    assert_eq!(
        parse_card_set(&card_with(r#"["a","b","c","d","e"]"#), &all).unwrap_err(),
        invalid(CardsError::TooManyTags { found: 5, max: 4 })
    );
    assert_eq!(
        parse_card_set(&card_with(r#"["a","b","a"]"#), &all).unwrap_err(),
        invalid(CardsError::DuplicateTag("a".into()))
    );
}

#[test]
fn a_card_tag_obeys_the_same_text_rules() {
    let all = list(&["a"]);
    for (tag, expected) in [
        (r#""""#, CardsError::TagEmpty),
        (r#""a\u0007""#, CardsError::TagControlChar),
        (r#"" a""#, CardsError::TagUntrimmed),
    ] {
        assert_eq!(
            parse_card_set(&card_with(&format!("[{tag}]")), &all).unwrap_err(),
            invalid(expected),
            "{tag}"
        );
    }
    let long = format!("\"{}\"", "é".repeat(MAX_TAG_LEN + 1));
    assert_eq!(
        parse_card_set(&card_with(&format!("[{long}]")), &all).unwrap_err(),
        invalid(CardsError::TagTooLong {
            len: MAX_TAG_LEN + 1,
            max: MAX_TAG_LEN
        })
    );
}

#[test]
fn a_game_without_tags_json_and_without_tags_is_read_as_before() {
    for json in [
        include_str!("../../../data/fixture/1.0.0/cards.json"),
        include_str!("../../../data/starter/1.1.0/cards.json"),
    ] {
        assert_eq!(parse_cards(json).unwrap(), parse_card_set(json, &[]).unwrap());
    }
}
