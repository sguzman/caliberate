use std::process::Command;

#[test]
fn calibredb_check_config() {
    let exe = env!("CARGO_BIN_EXE_calibredb");
    let config_path = workspace_config();
    let status = Command::new(exe)
        .args(["--config", config_path.to_str().unwrap(), "check-config"])
        .status()
        .expect("run calibredb");
    assert!(status.success());
}

#[test]
fn calibre_server_check_config() {
    let exe = env!("CARGO_BIN_EXE_calibre-server");
    let config_path = workspace_config();
    let status = Command::new(exe)
        .args(["--config", config_path.to_str().unwrap(), "check-config"])
        .status()
        .expect("run calibre-server");
    assert!(status.success());
}

fn workspace_config() -> std::path::PathBuf {
    let manifest_dir = std::path::PathBuf::from(env!("CARGO_MANIFEST_DIR"));
    manifest_dir
        .parent()
        .and_then(|p| p.parent())
        .expect("workspace root")
        .join("config")
        .join("control-plane.toml")
}
