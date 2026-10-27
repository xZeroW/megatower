//! Where the recovered Godot project lives. The game reads its assets and the
//! tests read the original scenes from it; both resolve it from the workspace
//! root rather than the current directory.

use std::path::{Path, PathBuf};

/// `Megaloot-GDRE-Recovered/` at the workspace root.
pub fn recovered_root() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR")).join("../../Megaloot-GDRE-Recovered")
}
