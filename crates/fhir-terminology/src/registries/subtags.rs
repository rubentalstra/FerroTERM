//! The IANA Language Subtag Registry (RFC 5646 §3.1), parsed from the
//! vendored record-jar file.

use std::collections::BTreeMap;
use std::sync::LazyLock;

/// The vendored registry text.
const REGISTRY: &str = include_str!("../../data/iana/language-subtag-registry");

/// One registry record.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Record {
    /// `Type`: `language`, `extlang`, `script`, `region`, `variant`,
    /// `grandfathered`, or `redundant`.
    pub kind: Kind,
    /// `Subtag` (or `Tag` for grandfathered and redundant records), as
    /// registered.
    pub subtag: String,
    /// `Description` lines.
    pub descriptions: Vec<String>,
    /// `Deprecated`, when the record is deprecated.
    pub deprecated: Option<String>,
    /// `Preferred-Value`.
    pub preferred: Option<String>,
    /// `Prefix` lines (extlang and variant records).
    pub prefixes: Vec<String>,
    /// `Suppress-Script`.
    pub suppress_script: Option<String>,
    /// `Scope` (`macrolanguage`, `collection`, `special`, `private-use`).
    pub scope: Option<String>,
}

/// A record type.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub enum Kind {
    /// A primary language subtag.
    Language,
    /// An extended language subtag.
    Extlang,
    /// A script subtag.
    Script,
    /// A region subtag.
    Region,
    /// A variant subtag.
    Variant,
    /// A whole grandfathered tag.
    Grandfathered,
    /// A whole redundant tag.
    Redundant,
}

impl Kind {
    fn parse(text: &str) -> Option<Self> {
        Some(match text {
            "language" => Self::Language,
            "extlang" => Self::Extlang,
            "script" => Self::Script,
            "region" => Self::Region,
            "variant" => Self::Variant,
            "grandfathered" => Self::Grandfathered,
            "redundant" => Self::Redundant,
            _ => return None,
        })
    }
}

/// The parsed registry.
#[derive(Debug)]
pub struct Registry {
    /// `File-Date`.
    pub file_date: String,
    records: BTreeMap<(Kind, String), Record>,
}

impl Registry {
    /// The record of `subtag` (compared without case) of `kind`.
    #[must_use]
    pub fn get(&self, kind: Kind, subtag: &str) -> Option<&Record> {
        self.records.get(&(kind, subtag.to_ascii_lowercase()))
    }

    /// Every record, by kind then lowercased subtag.
    pub fn records(&self) -> impl Iterator<Item = &Record> {
        self.records.values()
    }
}

/// The vendored registry, parsed once.
pub static REGISTRY_DATA: LazyLock<Registry> = LazyLock::new(|| parse(REGISTRY));

/// Parses a record-jar registry text.
#[must_use]
pub fn parse(text: &str) -> Registry {
    let mut file_date = String::new();
    let mut records = BTreeMap::new();
    for block in text.split("%%\n") {
        let fields = fields(block);
        if let Some(date) = fields.iter().find(|(k, _)| k == "File-Date") {
            file_date.clone_from(&date.1);
        }
        let Some(kind) = fields
            .iter()
            .find(|(k, _)| k == "Type")
            .and_then(|(_, v)| Kind::parse(v))
        else {
            continue;
        };
        let Some(subtag) = fields
            .iter()
            .find(|(k, _)| k == "Subtag" || k == "Tag")
            .map(|(_, v)| v.clone())
        else {
            continue;
        };
        let first = |name: &str| {
            fields
                .iter()
                .find(|(k, _)| k == name)
                .map(|(_, v)| v.clone())
        };
        let all = |name: &str| -> Vec<String> {
            fields
                .iter()
                .filter(|(k, _)| k == name)
                .map(|(_, v)| v.clone())
                .collect()
        };
        let record = Record {
            kind,
            subtag: subtag.clone(),
            descriptions: all("Description"),
            deprecated: first("Deprecated"),
            preferred: first("Preferred-Value"),
            prefixes: all("Prefix"),
            suppress_script: first("Suppress-Script"),
            scope: first("Scope"),
        };
        records.insert((kind, subtag.to_ascii_lowercase()), record);
    }
    Registry { file_date, records }
}

/// The `Name: value` fields of one record, continuation lines (leading
/// whitespace) folded into the previous value.
fn fields(block: &str) -> Vec<(String, String)> {
    let mut out: Vec<(String, String)> = Vec::new();
    for line in block.lines() {
        if line.starts_with(' ') || line.starts_with('\t') {
            if let Some((_, value)) = out.last_mut() {
                value.push(' ');
                value.push_str(line.trim());
            }
            continue;
        }
        if let Some((name, value)) = line.split_once(':') {
            out.push((name.trim().to_owned(), value.trim().to_owned()));
        }
    }
    out
}

#[cfg(test)]
mod tests {
    use super::{Kind, REGISTRY_DATA, parse};

    #[test]
    fn the_vendored_registry_parses_with_every_kind() {
        let registry = &*REGISTRY_DATA;
        assert!(!registry.file_date.is_empty());
        assert_eq!(
            registry
                .get(Kind::Language, "EN")
                .map(|r| r.descriptions[0].as_str()),
            Some("English")
        );
        assert_eq!(
            registry
                .get(Kind::Region, "gb")
                .map(|r| r.descriptions[0].as_str()),
            Some("United Kingdom")
        );
        assert!(registry.get(Kind::Script, "Latn").is_some());
        assert!(registry.get(Kind::Variant, "1606nict").is_some());
        assert!(registry.get(Kind::Grandfathered, "i-klingon").is_some());
        assert!(registry.get(Kind::Language, "zz").is_none());
    }

    #[test]
    fn continuation_lines_fold_into_the_value() {
        let registry = parse(
            "File-Date: 2026-01-01\n%%\nType: variant\nSubtag: abcde\nDescription: A long\n  description\nPrefix: en\nAdded: 2020-01-01\n",
        );
        let record = registry.get(Kind::Variant, "abcde").expect("record");
        assert_eq!(record.descriptions, ["A long description"]);
        assert_eq!(record.prefixes, ["en"]);
        assert_eq!(registry.file_date, "2026-01-01");
    }
}
