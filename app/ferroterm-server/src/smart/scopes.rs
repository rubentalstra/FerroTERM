//! SMART scopes: what a granted scope string permits on a resource type.
//!
//! The grammar is `system/<resource>.<permissions>`, where the permissions are
//! a subset of the in-order string `cruds`
//! (<https://hl7.org/fhir/smart-app-launch/scopes-and-launch-context.html>,
//! §Scopes for requesting FHIR Resources). The version 1 forms `.read`,
//! `.write`, and `.*` are accepted too: the specification maps them to `.rs`,
//! `.cud`, and `.cruds` in the same section.

/// The compartment the write routes are gated on.
///
/// A write on a `CodeSystem`, a `ValueSet`, or a `ConceptMap` carries no
/// patient and no user context, so the system compartment is the one that
/// applies (<https://hl7.org/fhir/smart-app-launch/scopes-and-launch-context.html>,
/// §Scopes for requesting clinical data).
const SYSTEM: &str = "system";

/// The permission letters, in the order the specification fixes them.
const CRUDS: &str = "cruds";

/// The FHIR interaction a request performs, as a SMART permission.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Permission {
    /// Type-level create, the `c` letter.
    Create,
    /// Instance-level update, the `u` letter.
    Update,
    /// Instance-level delete, the `d` letter.
    Delete,
}

impl Permission {
    /// The version 2 permission letter.
    #[must_use]
    pub const fn letter(self) -> char {
        match self {
            Self::Create => 'c',
            Self::Update => 'u',
            Self::Delete => 'd',
        }
    }

    /// The scope this permission reads as, for a diagnostic naming what is
    /// missing.
    #[must_use]
    pub fn scope_for(self, resource: &str) -> String {
        format!("{SYSTEM}/{resource}.{}", self.letter())
    }
}

/// Whether `granted`, one scope string from the token, permits `permission` on
/// `resource`.
///
/// A scope narrowed by search parameters (`system/CodeSystem.cud?url=…`) grants
/// nothing here: this server does not evaluate the restriction, and a scope it
/// cannot evaluate has to refuse rather than widen
/// (<https://hl7.org/fhir/smart-app-launch/scopes-and-launch-context.html>,
/// §Finer-grained resource constraints using search parameters).
#[must_use]
pub fn grants(granted: &str, resource: &str, permission: Permission) -> bool {
    let Some((compartment, rest)) = granted.trim().split_once('/') else {
        return false;
    };
    if compartment != SYSTEM || rest.contains('?') {
        return false;
    }
    let Some((requested, permissions)) = rest.rsplit_once('.') else {
        return false;
    };
    if requested != "*" && requested != resource {
        return false;
    }
    permits(permissions, permission)
}

/// Whether the permission part of a scope covers `permission`.
fn permits(permissions: &str, permission: Permission) -> bool {
    match permissions {
        // The version 1 forms, with the mapping the specification states: this
        // function is asked only about `c`, `u`, and `d`, which `.write` and
        // `.*` both cover and `.read` covers none of.
        "*" | "write" => true,
        "read" => false,
        letters => in_order(letters) && letters.contains(permission.letter()),
    }
}

/// Whether `letters` is a subset of `cruds` taken in order.
///
/// The specification fixes the order, so `.sr` names no permission and grants
/// nothing.
fn in_order(letters: &str) -> bool {
    if letters.is_empty() {
        return false;
    }
    let mut remaining = CRUDS.chars();
    letters
        .chars()
        .all(|letter| remaining.any(|known| known == letter))
}

/// The scopes a token carries, from `scope` and from `scp`.
///
/// RFC 6749 §3.3 makes `scope` a space-delimited string; `scp` is the array
/// form several issuers mint instead, so both are read.
#[must_use]
pub fn granted(scope: Option<&str>, scp: Option<&[String]>) -> Vec<String> {
    let mut out: Vec<String> = scope
        .map(|text| text.split_whitespace().map(str::to_owned).collect())
        .unwrap_or_default();
    if let Some(scp) = scp {
        out.extend(scp.iter().cloned());
    }
    out
}

#[cfg(test)]
mod tests {
    use super::{Permission, granted, grants, in_order};

    #[test]
    fn a_version_two_scope_grants_the_letter_it_carries() {
        for (scope, permission, expected) in [
            ("system/CodeSystem.c", Permission::Create, true),
            ("system/CodeSystem.c", Permission::Update, false),
            ("system/CodeSystem.cud", Permission::Delete, true),
            ("system/CodeSystem.cruds", Permission::Update, true),
            ("system/CodeSystem.rs", Permission::Create, false),
            ("system/*.cud", Permission::Create, true),
            ("system/ValueSet.cud", Permission::Create, false),
        ] {
            assert_eq!(
                grants(scope, "CodeSystem", permission),
                expected,
                "{scope} for {permission:?}"
            );
        }
    }

    // The specification maps `.read` to `.rs`, `.write` to `.cud`, and `.*` to
    // `.cruds` (<https://hl7.org/fhir/smart-app-launch/scopes-and-launch-context.html>).
    #[test]
    fn the_version_one_forms_map_to_the_version_two_letters() {
        assert!(grants(
            "system/CodeSystem.write",
            "CodeSystem",
            Permission::Update
        ));
        assert!(grants(
            "system/CodeSystem.*",
            "CodeSystem",
            Permission::Delete
        ));
        assert!(!grants(
            "system/CodeSystem.read",
            "CodeSystem",
            Permission::Create
        ));
        assert!(grants("system/*.write", "ConceptMap", Permission::Create));
    }

    #[test]
    fn a_scope_of_another_compartment_or_shape_grants_nothing() {
        for scope in [
            "user/CodeSystem.cud",
            "patient/CodeSystem.cud",
            "system/CodeSystem",
            "CodeSystem.cud",
            "system/CodeSystem.cud?url=http://example.org",
            "system/CodeSystem.sr",
            "system/CodeSystem.",
            "openid",
        ] {
            assert!(
                !grants(scope, "CodeSystem", Permission::Create),
                "{scope} grants nothing"
            );
        }
    }

    #[test]
    fn the_permission_letters_are_a_subset_of_cruds_in_order() {
        for letters in ["c", "cu", "cud", "cruds", "rs", "ds"] {
            assert!(in_order(letters), "{letters}");
        }
        for letters in ["", "sr", "dc", "cc", "x", "cudx"] {
            assert!(!in_order(letters), "{letters}");
        }
    }

    #[test]
    fn both_scope_carriers_are_read() {
        assert_eq!(
            granted(Some("system/CodeSystem.cud openid"), None),
            vec![
                String::from("system/CodeSystem.cud"),
                String::from("openid")
            ]
        );
        let scp = vec![String::from("system/ValueSet.cud")];
        assert_eq!(
            granted(None, Some(&scp)),
            vec![String::from("system/ValueSet.cud")]
        );
        assert_eq!(granted(None, None), Vec::<String>::new());
    }
}
