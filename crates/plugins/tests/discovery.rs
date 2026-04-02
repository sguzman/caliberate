use caliberate_core::config::PluginsConfig;
use caliberate_plugins::discovery::discover_plugins;
use tempfile::TempDir;

#[test]
fn discovers_plugin_manifest() {
    let temp = TempDir::new().expect("tempdir");
    let plugin_root = temp.path().join("plugins");
    let plugin_dir = plugin_root.join("sample");
    std::fs::create_dir_all(&plugin_dir).expect("create plugin dir");
    std::fs::write(
        plugin_dir.join("plugin.toml"),
        r#"
name = "sample"
version = "0.1.0"
entrypoint = "main.py"
permissions = ["files.read"]
"#,
    )
    .expect("write manifest");

    let config = PluginsConfig {
        enabled: true,
        plugins_dir: plugin_root.clone(),
    };
    let registry = discover_plugins(&config).expect("discover");
    assert_eq!(registry.list().len(), 1);
    assert_eq!(registry.list()[0].manifest.name, "sample");
}
