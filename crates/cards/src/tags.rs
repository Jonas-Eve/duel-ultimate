//! Optional `tags.json` of a set: the closed list of tags (display texts such as "bâtiment")
//! a card of the set may carry. Pure validation, no I/O; the engine never reads a tag.
//!
//! The text rules are written once ([`validate_tag_text`]) and shared by [`parse_tags`] and the
//! tags of a card ([`validate_card_tags`]). Tags are untrusted text, bounded in number, length
//! and control characters; a tag is never interpreted nor used as a key.

use std::collections::BTreeSet;

use crate::CardsError;

/// Largest accepted `tags.json` document, checked before parsing.
pub const MAX_TAGS_DOC_BYTES: usize = 8 * 1024;
/// Most entries in a data set's tag list.
pub const MAX_DATASET_TAGS: usize = 32;
/// Longest tag, in characters (`chars().count()`).
pub const MAX_TAG_LEN: usize = 24;
/// Most tags on one card.
pub const MAX_TAGS_PER_CARD: usize = 4;

#[derive(Debug, thiserror::Error, PartialEq, Eq)]
pub enum TagsError {
    #[error("invalid JSON (an array of strings is expected): {0}")]
    Json(String),
    #[error("tags document is {len} bytes, above the maximum of {max}")]
    DocTooLarge { len: usize, max: usize },
    #[error("a tag list has at most {max} entries, found {found}")]
    TooManyEntries { found: usize, max: usize },
    #[error("invalid tag: {0}")]
    InvalidTag(CardsError),
    #[error("duplicate tag `{0}`")]
    DuplicateTag(String),
}

/// The text rules of one tag: not empty, at most [`MAX_TAG_LEN`] characters, no control
/// character, no leading or trailing whitespace.
pub(crate) fn validate_tag_text(tag: &str) -> Result<(), CardsError> {
    if tag.is_empty() {
        return Err(CardsError::TagEmpty);
    }
    let len = tag.chars().count();
    if len > MAX_TAG_LEN {
        return Err(CardsError::TagTooLong {
            len,
            max: MAX_TAG_LEN,
        });
    }
    if tag.chars().any(char::is_control) {
        return Err(CardsError::TagControlChar);
    }
    if tag.trim() != tag {
        return Err(CardsError::TagUntrimmed);
    }
    Ok(())
}

/// The tags of one card: at most [`MAX_TAGS_PER_CARD`], each valid text, no duplicate. Whether
/// they belong to the data set's list is checked by [`parse_card_set`](crate::parse_card_set).
pub(crate) fn validate_card_tags(tags: &[String]) -> Result<(), CardsError> {
    if tags.len() > MAX_TAGS_PER_CARD {
        return Err(CardsError::TooManyTags {
            found: tags.len(),
            max: MAX_TAGS_PER_CARD,
        });
    }
    let mut seen = BTreeSet::new();
    for tag in tags {
        validate_tag_text(tag)?;
        if !seen.insert(tag.as_str()) {
            return Err(CardsError::DuplicateTag(tag.clone()));
        }
    }
    Ok(())
}

/// Parses and validates a `tags.json` document: an array of at most [`MAX_DATASET_TAGS`]
/// distinct tags.
pub fn parse_tags(json: &str) -> Result<Vec<String>, TagsError> {
    if json.len() > MAX_TAGS_DOC_BYTES {
        return Err(TagsError::DocTooLarge {
            len: json.len(),
            max: MAX_TAGS_DOC_BYTES,
        });
    }
    let tags: Vec<String> =
        serde_json::from_str(json).map_err(|e| TagsError::Json(e.to_string()))?;
    if tags.len() > MAX_DATASET_TAGS {
        return Err(TagsError::TooManyEntries {
            found: tags.len(),
            max: MAX_DATASET_TAGS,
        });
    }
    let mut seen = BTreeSet::new();
    for tag in &tags {
        validate_tag_text(tag).map_err(TagsError::InvalidTag)?;
        if !seen.insert(tag.as_str()) {
            return Err(TagsError::DuplicateTag(tag.clone()));
        }
    }
    Ok(tags)
}
