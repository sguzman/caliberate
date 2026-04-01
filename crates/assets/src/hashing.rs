//! Asset hashing utilities.

use caliberate_core::error::{CoreError, CoreResult};
use sha2::{Digest, Sha256};
use std::fs::File;
use std::io::Read;
use std::path::Path;
use zstd::stream::read::Decoder;

pub fn hash_file_sha256(path: &Path) -> CoreResult<String> {
    let mut file =
        File::open(path).map_err(|err| CoreError::Io("open file for hashing".to_string(), err))?;
    hash_reader_sha256(&mut file)
}

pub fn hash_zstd_file_sha256(path: &Path) -> CoreResult<String> {
    let file = File::open(path)
        .map_err(|err| CoreError::Io("open compressed file for hashing".to_string(), err))?;
    let mut decoder =
        Decoder::new(file).map_err(|err| CoreError::ConfigValidate(err.to_string()))?;
    hash_reader_sha256(&mut decoder)
}

fn hash_reader_sha256(reader: &mut dyn Read) -> CoreResult<String> {
    let mut hasher = Sha256::new();
    let mut buffer = [0u8; 8192];
    loop {
        let read = reader
            .read(&mut buffer)
            .map_err(|err| CoreError::Io("read data for hashing".to_string(), err))?;
        if read == 0 {
            break;
        }
        hasher.update(&buffer[..read]);
    }
    Ok(hex::encode(hasher.finalize()))
}
