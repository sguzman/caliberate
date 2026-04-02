use caliberate_core::config::ControlPlane;
use caliberate_gui::app::CaliberateApp;

#[test]
fn gui_app_initializes_with_temp_config() {
    let temp_dir = tempfile::Builder::new()
        .prefix("caliberate-test-gui-")
        .tempdir()
        .expect("tempdir");

    let fixture = workspace_root().join("crates/core/tests/fixtures/control-plane.toml");
    let mut config = ControlPlane::load_from_path(&fixture).expect("load fixture");
    let data_dir = temp_dir.path().join("data");
    let cache_dir = temp_dir.path().join("cache");
    let log_dir = temp_dir.path().join("logs");
    let tmp_dir = temp_dir.path().join("tmp");
    let library_dir = temp_dir.path().join("library");
    std::fs::create_dir_all(&data_dir).expect("create data dir");
    std::fs::create_dir_all(&cache_dir).expect("create cache dir");
    std::fs::create_dir_all(&log_dir).expect("create log dir");
    std::fs::create_dir_all(&tmp_dir).expect("create tmp dir");
    std::fs::create_dir_all(&library_dir).expect("create library dir");

    config.paths.data_dir = data_dir.clone();
    config.paths.cache_dir = cache_dir;
    config.paths.log_dir = log_dir;
    config.paths.tmp_dir = tmp_dir;
    config.paths.library_dir = library_dir;
    config.db.sqlite_path = data_dir.join("caliberate.db");

    let config_path = temp_dir.path().join("control-plane.toml");
    config.save_to_path(&config_path).expect("save config");

    let app = CaliberateApp::try_new(config, config_path).expect("init app");
    std::mem::drop(app);
}

fn workspace_root() -> std::path::PathBuf {
    let manifest_dir = std::path::PathBuf::from(env!("CARGO_MANIFEST_DIR"));
    manifest_dir
        .parent()
        .and_then(|p| p.parent())
        .expect("workspace root")
        .to_path_buf()
}
