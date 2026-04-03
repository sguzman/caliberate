use caliberate_app::bootstrap;
use caliberate_core::config::ControlPlane;
use std::path::PathBuf;

#[test]
fn bootstrap_creates_runtime_directories() {
    let temp_dir = tempfile::tempdir().expect("temp dir");
    let root = temp_dir.path();

    let manifest_dir = PathBuf::from(env!("CARGO_MANIFEST_DIR"));
    let config_path = manifest_dir
        .join("..")
        .join("..")
        .join("config")
        .join("control-plane.toml");
    let mut config = ControlPlane::load_from_path(&config_path).expect("load config");

    let data_dir = root.join("data");
    let cache_dir = root.join("cache");
    let log_dir = root.join("logs");
    let tmp_dir = root.join("tmp");
    let library_dir = root.join("library");
    let conversion_temp = root.join("conversion").join("tmp");
    let conversion_output = root.join("conversion").join("out");
    let sqlite_path = data_dir.join("caliberate.db");

    config.paths.data_dir = data_dir.clone();
    config.paths.cache_dir = cache_dir.clone();
    config.paths.log_dir = log_dir.clone();
    config.paths.tmp_dir = tmp_dir.clone();
    config.paths.library_dir = library_dir.clone();
    config.db.sqlite_path = sqlite_path.clone();
    config.conversion.temp_dir = conversion_temp.clone();
    config.conversion.output_dir = conversion_output.clone();

    let temp_config_path = PathBuf::from(root.join("control-plane.toml"));
    config
        .save_to_path(&temp_config_path)
        .expect("save config");

    bootstrap::init(&temp_config_path).expect("bootstrap");

    assert!(data_dir.exists(), "data dir should exist");
    assert!(cache_dir.exists(), "cache dir should exist");
    assert!(log_dir.exists(), "log dir should exist");
    assert!(tmp_dir.exists(), "tmp dir should exist");
    assert!(library_dir.exists(), "library dir should exist");
    assert!(conversion_temp.exists(), "conversion temp dir should exist");
    assert!(conversion_output.exists(), "conversion output dir should exist");
    assert!(
        sqlite_path.parent().unwrap().exists(),
        "sqlite parent should exist"
    );
}
