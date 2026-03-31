use caliberate_core::config::ControlPlane;
use caliberate_core::logging;
use std::path::PathBuf;
use std::sync::Once;

static INIT: Once = Once::new();

#[test]
fn loads_control_plane_fixture() {
    let path = fixture_path("control-plane.toml");
    let config = ControlPlane::load_from_path(&path).expect("config load");
    assert_eq!(config.app.name, "caliberate");
    assert_eq!(config.formats.supported, vec!["epub".to_string()]);
}

#[test]
fn logging_initializes_once() {
    let path = fixture_path("control-plane.toml");
    let config = ControlPlane::load_from_path(&path).expect("config load");

    INIT.call_once(|| {
        let _guard = logging::init(&config).expect("logging init");
    });
}

fn fixture_path(name: &str) -> PathBuf {
    let root = PathBuf::from(env!("CARGO_MANIFEST_DIR"));
    root.join("tests").join("fixtures").join(name)
}
