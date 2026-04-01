//! Raw asset compression policies.

use caliberate_core::config::AssetsConfig;
use caliberate_core::error::{CoreError, CoreResult};
use std::fs::File;
use std::io::{self, Read, Write};
use std::path::Path;
use zstd::stream::{read::Decoder, write::Encoder};

pub fn should_compress_asset(config: &AssetsConfig) -> bool {
    config.compress_raw_assets
}

pub fn should_compress_metadata_db(config: &AssetsConfig) -> bool {
    config.compress_metadata_db
}

pub fn compress_file(source: &Path, dest: &Path, level: i32) -> CoreResult<u64> {
    let mut input = File::open(source)
        .map_err(|err| CoreError::Io("open asset for compression".to_string(), err))?;
    let output = File::create(dest)
        .map_err(|err| CoreError::Io("create compressed asset".to_string(), err))?;
    let mut encoder =
        Encoder::new(output, level).map_err(|err| CoreError::ConfigValidate(err.to_string()))?;
    let written = io::copy(&mut input, &mut encoder)
        .map_err(|err| CoreError::Io("compress asset".to_string(), err))?;
    encoder
        .finish()
        .map_err(|err| CoreError::Io("finish compression".to_string(), err))?;
    Ok(written)
}

pub fn decompress_file(source: &Path, dest: &Path) -> CoreResult<u64> {
    let input = File::open(source)
        .map_err(|err| CoreError::Io("open compressed asset".to_string(), err))?;
    let mut decoder =
        Decoder::new(input).map_err(|err| CoreError::ConfigValidate(err.to_string()))?;
    let mut output = File::create(dest)
        .map_err(|err| CoreError::Io("create decompressed asset".to_string(), err))?;
    let written = io::copy(&mut decoder, &mut output)
        .map_err(|err| CoreError::Io("decompress asset".to_string(), err))?;
    Ok(written)
}

pub fn decompress_to_writer(source: &Path, mut dest: impl Write) -> CoreResult<u64> {
    let input = File::open(source)
        .map_err(|err| CoreError::Io("open compressed asset".to_string(), err))?;
    let mut decoder =
        Decoder::new(input).map_err(|err| CoreError::ConfigValidate(err.to_string()))?;
    let written = io::copy(&mut decoder, &mut dest)
        .map_err(|err| CoreError::Io("stream decompress asset".to_string(), err))?;
    Ok(written)
}

pub fn compress_from_reader(mut source: impl Read, dest: &Path, level: i32) -> CoreResult<u64> {
    let output = File::create(dest)
        .map_err(|err| CoreError::Io("create compressed asset".to_string(), err))?;
    let mut encoder =
        Encoder::new(output, level).map_err(|err| CoreError::ConfigValidate(err.to_string()))?;
    let written = io::copy(&mut source, &mut encoder)
        .map_err(|err| CoreError::Io("compress asset".to_string(), err))?;
    encoder
        .finish()
        .map_err(|err| CoreError::Io("finish compression".to_string(), err))?;
    Ok(written)
}

// Stream helpers intentionally use the explicit encoder/decoder to retain byte counts.
