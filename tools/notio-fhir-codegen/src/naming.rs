//! FHIR names to Rust identifiers.
//!
//! Type names keep FHIR's spelling in `PascalCase` (`ValueSet`, `Coding`,
//! `Boolean` for the primitive `boolean`); a backbone element is named by its
//! path (`ValueSet.compose.include` becomes `ValueSetComposeInclude`); fields
//! are `snake_case`, and a Rust keyword becomes a raw identifier (`r#type`).

/// Rust keywords that need the raw-identifier form when used as a field name.
const KEYWORDS: [&str; 51] = [
    "abstract", "as", "async", "await", "become", "box", "break", "const", "continue", "do", "dyn",
    "else", "enum", "extern", "false", "final", "fn", "for", "gen", "if", "impl", "in", "let",
    "loop", "macro", "match", "mod", "move", "mut", "override", "priv", "pub", "ref", "return",
    "static", "struct", "trait", "true", "try", "type", "typeof", "unsafe", "unsized", "use",
    "virtual", "where", "while", "yield", "union", "safe", "raw",
];

/// The `PascalCase` Rust type name for a FHIR type name.
///
/// Complex types and resources are already `PascalCase`; primitives are
/// lowercase (`dateTime`) and get their first letter raised (`DateTime`).
#[must_use]
pub fn type_name(fhir_name: &str) -> String {
    let mut chars = fhir_name.chars();
    match chars.next() {
        Some(first) => first.to_uppercase().chain(chars).collect(),
        None => String::new(),
    }
}

/// The struct name for a backbone element at `path`, for example
/// `ValueSet.compose.include` to `ValueSetComposeInclude`.
#[must_use]
pub fn backbone_name(path: &str) -> String {
    path.split('.')
        .map(|segment| type_name(segment.trim_end_matches("[x]")))
        .collect()
}

/// The Rust field name for a FHIR element name.
///
/// `camelCase` becomes `snake_case`, a choice suffix `[x]` is dropped, and a
/// keyword is written as a raw identifier.
#[must_use]
pub fn field_name(element_name: &str) -> String {
    let stem = element_name.trim_end_matches("[x]");
    let mut out = String::with_capacity(stem.len() + 4);
    for (index, ch) in stem.chars().enumerate() {
        if ch.is_uppercase() {
            if index > 0 {
                out.push('_');
            }
            out.extend(ch.to_lowercase());
        } else {
            out.push(ch);
        }
    }
    if KEYWORDS.contains(&out.as_str()) {
        format!("r#{out}")
    } else {
        out
    }
}

/// The `snake_case` module (file) name for a Rust type name.
#[must_use]
pub fn module_name(type_name: &str) -> String {
    let mut out = String::with_capacity(type_name.len() + 4);
    let chars: Vec<char> = type_name.chars().collect();
    for (index, ch) in chars.iter().enumerate() {
        if ch.is_uppercase() {
            let prev_lower = index > 0
                && chars
                    .get(index - 1)
                    .is_some_and(|p| p.is_lowercase() || p.is_ascii_digit());
            let next_lower = chars.get(index + 1).is_some_and(|n| n.is_lowercase());
            let prev_upper = index > 0 && chars.get(index - 1).is_some_and(|p| p.is_uppercase());
            if index > 0 && (prev_lower || (prev_upper && next_lower)) {
                out.push('_');
            }
            out.extend(ch.to_lowercase());
        } else {
            out.push(*ch);
        }
    }
    out
}

#[cfg(test)]
mod tests {
    use super::{backbone_name, field_name, module_name, type_name};

    #[test]
    fn type_names_are_pascal_case() {
        assert_eq!(type_name("ValueSet"), "ValueSet");
        assert_eq!(type_name("dateTime"), "DateTime");
        assert_eq!(type_name("string"), "String");
        assert_eq!(type_name("base64Binary"), "Base64Binary");
    }

    #[test]
    fn backbone_names_follow_the_path() {
        assert_eq!(
            backbone_name("ValueSet.compose.include"),
            "ValueSetComposeInclude"
        );
        assert_eq!(backbone_name("Parameters.parameter"), "ParametersParameter");
    }

    #[test]
    fn field_names_are_snake_case_with_raw_keywords() {
        assert_eq!(field_name("lockedDate"), "locked_date");
        assert_eq!(field_name("value[x]"), "value");
        assert_eq!(field_name("type"), "r#type");
        assert_eq!(field_name("use"), "r#use");
        assert_eq!(field_name("abstract"), "r#abstract");
        assert_eq!(field_name("modifierExtension"), "modifier_extension");
    }

    #[test]
    fn module_names_split_acronyms() {
        assert_eq!(module_name("ValueSet"), "value_set");
        assert_eq!(module_name("CodeableConcept"), "codeable_concept");
        assert_eq!(module_name("Base64Binary"), "base64_binary");
        assert_eq!(module_name("HumanName"), "human_name");
        assert_eq!(module_name("DateTime"), "date_time");
    }
}
