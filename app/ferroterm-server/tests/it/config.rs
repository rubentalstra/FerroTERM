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
            instance_id(VERSION),
            String::from("http://snomed.info/sct"),
            String::from(VERSION)
        )]
    );
    assert!(state.instance(&instance_id(VERSION)).is_some());
    assert!(state.instance("nope").is_none());
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
