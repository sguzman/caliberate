use caliberate_conversion::jobs::{ConversionJobRunner, build_request};
use caliberate_conversion::pipeline::convert_file;
use caliberate_conversion::settings::ConversionSettings;
use caliberate_core::config::ControlPlane;
use std::fs;
use tempfile::tempdir;

#[test]
fn passthrough_conversion_copies_file() {
    let dir = tempdir().expect("tempdir");
    let input = dir.path().join("book.epub");
    let output = dir.path().join("out.epub");
    fs::write(&input, b"book data").expect("write input");

    let config_path = std::path::PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("../../config/control-plane.toml");
    let config = ControlPlane::load_from_path(&config_path).expect("load config");
    let settings = ConversionSettings::from_config(&config.conversion);

    let report = convert_file(&input, &output, &settings).expect("convert");
    assert!(report.output_path.exists());
    let contents = fs::read(&output).expect("read output");
    assert_eq!(contents, b"book data");
}

#[test]
fn conversion_rejects_unsupported_format() {
    let dir = tempdir().expect("tempdir");
    let input = dir.path().join("book.epub");
    let output = dir.path().join("out.pdf");
    fs::write(&input, b"book data").expect("write input");

    let config_path = std::path::PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("../../config/control-plane.toml");
    let config = ControlPlane::load_from_path(&config_path).expect("load config");
    let settings = ConversionSettings::from_config(&config.conversion)
        .with_output_format(Some("pdf".to_string()));

    let err = convert_file(&input, &output, &settings).expect_err("convert");
    assert!(err.to_string().contains("converter not implemented"));
}

#[test]
fn conversion_job_runner_reports_status() {
    let dir = tempdir().expect("tempdir");
    let input = dir.path().join("book.epub");
    let output = dir.path().join("out.epub");
    fs::write(&input, b"book data").expect("write input");

    let config_path = std::path::PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("../../config/control-plane.toml");
    let config = ControlPlane::load_from_path(&config_path).expect("load config");
    let settings = ConversionSettings::from_config(&config.conversion);

    let runner = ConversionJobRunner::new();
    let request = build_request(&input, &output, settings);
    let summary = runner.run(request).expect("run job");
    let list = runner.list();
    assert_eq!(list.len(), 1);
    assert_eq!(list[0].id, summary.id);
}

#[test]
fn conversion_rejects_large_input() {
    let dir = tempdir().expect("tempdir");
    let input = dir.path().join("book.epub");
    let output = dir.path().join("out.epub");
    fs::write(&input, b"book data").expect("write input");

    let config_path = std::path::PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("../../config/control-plane.toml");
    let config = ControlPlane::load_from_path(&config_path).expect("load config");
    let mut settings = ConversionSettings::from_config(&config.conversion);
    settings.max_input_bytes = 1;

    let err = convert_file(&input, &output, &settings).expect_err("convert");
    assert!(err.to_string().contains("max_input_bytes"));
}
