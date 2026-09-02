//! Loading the artifacts named by the configuration.

use ferroterm_server::config::Config;
use ferroterm_server::state::{AppState, LoadError, instance_id};
use ferroterm_testkit::snomed::VERSION;

#[test]
fn the_state_loads_every_named_artifact_and_names_its_instances() {
    let dir = tempfile::tempdir().expect("tempdir");
    ferroterm_testkit::snomed::write(dir.path()).expect("writes");
    let config = Config {
        index: vec![dir.path().to_path_buf()],
        ..Config::default()
    };
    let state = AppState::load(&config).expect("loads");
    let instances: Vec<(String, String, String)> = state
        .instances()
        .map(|(a, b, c)| (a.to_owned(), b.to_owned(), c.to_owned()))
        .collect();
    assert_eq!(
        instances,
        [(
            instance_id("http://snomed.info/sct", VERSION),
            String::from("http://snomed.info/sct"),
            String::from(VERSION)
        )]
    );
    assert!(
        state
            .instance(&instance_id("http://snomed.info/sct", VERSION))
            .is_some()
    );
    assert!(state.instance("nope").is_none());
    let summaries = state.summaries().expect("summarises");
    assert_eq!(summaries.len(), 1);
    assert_eq!(summaries[0].concepts, Some(8));
    assert_eq!(summaries[0].languages, ["en", "nl"]);
    assert_eq!(summaries[0].path.as_deref(), Some(dir.path()));
}

#[test]
fn a_missing_or_duplicate_artifact_refuses_to_start() {
    let missing = Config {
        index: vec![std::path::PathBuf::from("/nonexistent/ferroterm")],
        ..Config::default()
    };
    assert!(matches!(
        AppState::load(&missing),
        Err(LoadError::Open { .. })
    ));
    let dir = tempfile::tempdir().expect("tempdir");
    ferroterm_testkit::snomed::write(dir.path()).expect("writes");
    let twice = Config {
        index: vec![dir.path().to_path_buf(), dir.path().to_path_buf()],
        ..Config::default()
    };
    assert!(matches!(
        AppState::load(&twice),
        Err(LoadError::Register(_))
    ));
    let empty = AppState::load(&Config::default()).expect("no artifacts is a valid, empty server");
    assert_eq!(empty.instances().count(), 0);
}

#[test]
fn the_environment_fills_the_config() {
    let config = Config::default();
    assert_eq!(config.listen, "127.0.0.1:8080");
    assert_eq!(config.default_language, "en");
    assert!(config.index.is_empty());
}

#[test]
fn code_system_directories_load_and_supplements_apply() {
    use ferroterm_testkit::fhir::{ANIMALS, ANIMALS_NL, COLOURS, SKETCH};

    let dir = tempfile::tempdir().expect("tempdir");
    ferroterm_testkit::fhir::write_code_systems(dir.path()).expect("writes");
    let config = Config {
        code_systems: vec![dir.path().to_path_buf()],
        ..Config::default()
    };
    let state = AppState::load(&config).expect("loads");
    let urls: Vec<&str> = state.instances().map(|(_, url, _)| url).collect();
    assert_eq!(
        urls,
        [ANIMALS, COLOURS, SKETCH],
        "the supplement is no instance"
    );
    assert!(
        state
            .instances()
            .any(|(id, url, version)| url == ANIMALS && id == instance_id(ANIMALS, version)),
        "an instance id carries the system, not only the version"
    );
    let animals = state.provider(ANIMALS).expect("animals");
    let cat = animals.locate("cat").expect("reads").expect("cat").concept;
    assert_eq!(
        animals.display(cat, Some("nl")).expect("reads").as_deref(),
        Some("Kat"),
        "the supplement {ANIMALS_NL} is applied"
    );
    let summaries = state.summaries().expect("summarises");
    let sketch = summaries.iter().find(|s| s.url == SKETCH).expect("sketch");
    assert_eq!(sketch.concepts, None, "example content does not enumerate");
    assert_eq!(sketch.path.as_deref(), Some(dir.path()));
}

#[test]
fn a_supplement_without_its_system_refuses_to_start() {
    let dir = tempfile::tempdir().expect("tempdir");
    let supplement = serde_json::json!({
        "resourceType": "CodeSystem",
        "url": "http://example.org/orphan-nl",
        "status": "active",
        "content": "supplement",
        "supplements": "http://example.org/nowhere",
        "concept": [{"code": "a", "designation": [{"language": "nl", "value": "A"}]}]
    });
    std::fs::write(
        dir.path().join("CodeSystem-orphan.json"),
        supplement.to_string(),
    )
    .expect("writes");
    let config = Config {
        code_systems: vec![dir.path().to_path_buf()],
        ..Config::default()
    };
    assert!(matches!(
        AppState::load(&config),
        Err(LoadError::SupplementTarget { .. })
    ));
}
