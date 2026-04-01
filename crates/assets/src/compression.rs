//! Raw asset compression policies.

use caliberate_core::config::AssetsConfig;

pub fn should_compress_asset(config: &AssetsConfig) -> bool {
    config.compress_raw_assets
}

pub fn should_compress_metadata_db(config: &AssetsConfig) -> bool {
    config.compress_metadata_db
}
