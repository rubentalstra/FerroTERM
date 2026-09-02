//! Identity, versions, and the default-version rule.

use std::sync::Arc;

use ferroterm_terminology::registry::{RegisterError, ResolveError};

use crate::fixture::{FLAT_URL, Fixture, URL, registry};

#[test]
fn the_default_is_the_configured_version_else_the_greatest() {
    let mut registry = registry();
    let resolved = registry.resolve(URL, None).expect("resolves");
    assert_eq!(resolved.provider.identity().version, "2025");
    assert!(resolved.defaulted);
    registry.set_default(URL, "2024").expect("sets default");
    let resolved = registry.resolve(URL, None).expect("resolves");
    assert_eq!(resolved.provider.identity().version, "2024");
    assert_eq!(registry.default_version(URL), Some("2024"));
    let explicit = registry.resolve(URL, Some("2025")).expect("resolves");
    assert_eq!(explicit.provider.identity().version, "2025");
    assert!(!explicit.defaulted);
}

#[test]
fn unknown_systems_versions_and_duplicates_are_typed_errors() {
    let mut registry = registry();
    assert_eq!(
        registry.resolve("http://example.org/nowhere", None).err(),
        Some(ResolveError::UnknownSystem(String::from(
            "http://example.org/nowhere"
        )))
    );
    assert_eq!(
        registry.resolve(URL, Some("1999")).err(),
        Some(ResolveError::UnknownVersion {
            url: String::from(URL),
            version: String::from("1999"),
        })
    );
    assert_eq!(
        registry.set_default(FLAT_URL, "2").err(),
        Some(ResolveError::UnknownVersion {
            url: String::from(FLAT_URL),
            version: String::from("2"),
        })
    );
    assert_eq!(
        registry.register(Arc::new(Fixture::flat())).err(),
        Some(RegisterError::Duplicate {
            url: String::from(FLAT_URL),
            version: String::from("1"),
        })
    );
    assert_eq!(registry.systems().collect::<Vec<_>>(), vec![URL, FLAT_URL]);
    assert_eq!(registry.versions(URL).count(), 2);
}
