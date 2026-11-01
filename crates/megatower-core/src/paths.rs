//! Where the packaged runtime assets live, resolved from the workspace root
//! rather than the current directory.

use std::path::{Path, PathBuf};

/// Rust workspace root containing packaged runtime assets.
pub fn runtime_root() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR")).join("../..")
}
