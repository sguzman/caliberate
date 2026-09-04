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

#[test]
fn pane_width_validation_accepts_runtime_minimum_and_legacy_values() {
    assert!(save_with_pane_widths("runtime-minimum", 200.0, 200.0).is_ok());
    assert!(save_with_pane_widths("legacy-high", 1600.0, 900.0).is_ok());
}

#[test]
fn pane_width_validation_rejects_values_below_runtime_minimum() {
    assert!(save_with_pane_widths("left-too-small", 199.0, 400.0).is_err());
    assert!(save_with_pane_widths("right-too-small", 400.0, 199.0).is_err());
}

fn save_with_pane_widths(
    name: &str,
    left: f32,
    right: f32,
) -> Result<(), caliberate_core::error::CoreError> {
    let mut config = ControlPlane::load_from_path(fixture_path("control-plane.toml"))?;
    config.gui.pane_left_width = left;
    config.gui.pane_right_width = right;
    let path = std::env::temp_dir().join(format!(
        "caliberate-config-{name}-{}.toml",
        std::process::id()
    ));
    let result = config.save_to_path(&path);
    let _ = std::fs::remove_file(path);
    result
}

fn fixture_path(name: &str) -> PathBuf {
    let root = PathBuf::from(env!("CARGO_MANIFEST_DIR"));
    root.join("tests").join("fixtures").join(name)
}
