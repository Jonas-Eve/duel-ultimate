//! Optional `art.json` of a set: presentation metadata (which image, who made it, under
//! which licence) for the cards of `cards.json`. Pure validation, no I/O: the engine never
//! reads it. Files on disk are checked by `tests/art_files.rs`.

use std::collections::BTreeSet;

use serde::{Deserialize, Serialize};

use crate::Card;

/// Largest accepted `art.json` document, checked before parsing.
pub const MAX_ART_DOC_BYTES: usize = 64 * 1024;
/// Longest `author`, in characters.
pub const MAX_ART_AUTHOR_LEN: usize = 100;
/// Longest `source`, in characters.
pub const MAX_ART_SOURCE_LEN: usize = 300;
/// Longest accepted file name (stem and extension), in bytes.
const MAX_ART_STEM_LEN: usize = 64;

/// The closed list of accepted licences, as SPDX identifiers.
pub const ART_LICENSES: [&str; 2] = ["CC0-1.0", "CC-BY-4.0"];

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum ArtLicense {
    #[serde(rename = "CC0-1.0")]
    Cc0,
    #[serde(rename = "CC-BY-4.0")]
    CcBy,
}

impl ArtLicense {
    pub fn spdx(self) -> &'static str {
        match self {
            ArtLicense::Cc0 => "CC0-1.0",
            ArtLicense::CcBy => "CC-BY-4.0",
        }
    }

    fn from_spdx(s: &str) -> Option<Self> {
        match s {
            "CC0-1.0" => Some(ArtLicense::Cc0),
            "CC-BY-4.0" => Some(ArtLicense::CcBy),
            _ => None,
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ArtFormat {
    Webp,
    Svg,
}

/// One image of a card. `author` and `source` are free text, never an active link.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ArtEntry {
    pub id: String,
    pub file: String,
    pub author: String,
    pub license: ArtLicense,
    pub source: String,
}

impl ArtEntry {
    /// The image format, from the (already validated) file extension.
    pub fn format(&self) -> ArtFormat {
        if self.file.ends_with(".svg") {
            ArtFormat::Svg
        } else {
            ArtFormat::Webp
        }
    }
}

/// Why an `art.json` document was refused. Distinct from `CardsError`: the engine never
/// sees it.
#[derive(Debug, Clone, PartialEq, Eq, thiserror::Error)]
pub enum ArtError {
    #[error("invalid JSON: {0}")]
    Json(String),
    #[error("art document is {len} bytes but the maximum is {max}")]
    DocumentTooLarge { len: usize, max: usize },
    #[error("duplicate art entry for card `{0}`")]
    DuplicateArt(String),
    #[error("art entry references unknown card `{0}`")]
    UnknownCard(String),
    #[error("art entry `{id}`: invalid file name `{file}`")]
    InvalidFile { id: String, file: String },
    #[error("file `{0}` is used by two art entries")]
    DuplicateFile(String),
    #[error("art entry `{id}`: licence `{license}` is not accepted")]
    UnknownLicense { id: String, license: String },
    #[error("art entry `{id}`: `{field}` must not be empty")]
    EmptyField { id: String, field: &'static str },
    #[error("art entry `{id}`: `{field}` has {len} characters but the maximum is {max}")]
    FieldTooLong {
        id: String,
        field: &'static str,
        len: usize,
        max: usize,
    },
    #[error("art entry `{id}`: `{field}` contains a control character")]
    ControlCharacter { id: String, field: &'static str },
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct RawEntry {
    id: String,
    file: String,
    author: String,
    license: String,
    source: String,
}

/// `^[a-z0-9][a-z0-9_-]{0,63}\.(webp|svg)$`, written by hand (a regex `$` would accept a
/// trailing newline).
fn valid_file_name(file: &str) -> bool {
    let Some((stem, ext)) = file.rsplit_once('.') else {
        return false;
    };
    if ext != "webp" && ext != "svg" {
        return false;
    }
    let mut bytes = stem.bytes();
    let first_ok = bytes
        .next()
        .is_some_and(|b| b.is_ascii_lowercase() || b.is_ascii_digit());
    first_ok
        && stem.len() <= MAX_ART_STEM_LEN
        && bytes.all(|b| b.is_ascii_lowercase() || b.is_ascii_digit() || b == b'_' || b == b'-')
}

fn check_text(id: &str, field: &'static str, value: &str, max: usize) -> Result<(), ArtError> {
    let len = value.chars().count();
    if len == 0 {
        return Err(ArtError::EmptyField {
            id: id.to_owned(),
            field,
        });
    }
    if len > max {
        return Err(ArtError::FieldTooLong {
            id: id.to_owned(),
            field,
            len,
            max,
        });
    }
    if value.chars().any(char::is_control) {
        return Err(ArtError::ControlCharacter {
            id: id.to_owned(),
            field,
        });
    }
    Ok(())
}

/// Parses and validates an `art.json` document against the cards of the same set.
pub fn parse_art(json: &str, cards: &[Card]) -> Result<Vec<ArtEntry>, ArtError> {
    if json.len() > MAX_ART_DOC_BYTES {
        return Err(ArtError::DocumentTooLarge {
            len: json.len(),
            max: MAX_ART_DOC_BYTES,
        });
    }
    let raw: Vec<RawEntry> =
        serde_json::from_str(json).map_err(|e| ArtError::Json(e.to_string()))?;
    let card_ids: BTreeSet<&str> = cards.iter().map(Card::id).collect();
    let mut ids = BTreeSet::new();
    let mut files = BTreeSet::new();
    let mut out = Vec::with_capacity(raw.len());
    for e in raw {
        if !ids.insert(e.id.clone()) {
            return Err(ArtError::DuplicateArt(e.id));
        }
        if !card_ids.contains(e.id.as_str()) {
            return Err(ArtError::UnknownCard(e.id));
        }
        if !valid_file_name(&e.file) {
            return Err(ArtError::InvalidFile {
                id: e.id,
                file: e.file,
            });
        }
        if !files.insert(e.file.clone()) {
            return Err(ArtError::DuplicateFile(e.file));
        }
        let Some(license) = ArtLicense::from_spdx(&e.license) else {
            return Err(ArtError::UnknownLicense {
                id: e.id,
                license: e.license,
            });
        };
        check_text(&e.id, "author", &e.author, MAX_ART_AUTHOR_LEN)?;
        check_text(&e.id, "source", &e.source, MAX_ART_SOURCE_LEN)?;
        out.push(ArtEntry {
            id: e.id,
            file: e.file,
            author: e.author,
            license,
            source: e.source,
        });
    }
    Ok(out)
}
