//! Where the packaged runtime assets live, resolved from the workspace root
//! rather than the current directory.

use std::path::{Path, PathBuf};

/// Rust workspace root containing packaged runtime assets.
pub fn runtime_root() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR")).join("../..")
}

/// The `assets/` folder the game loads from, for code that reads asset files
/// itself: under `BEVY_ASSET_ROOT` if set, else the workspace's, else the one
/// beside the executable (a release download).
pub fn assets_root() -> PathBuf {
    let beside_exe = || {
        std::env::current_exe()
            .ok()
            .and_then(|exe| Some(exe.parent()?.join("assets")))
    };
    std::env::var_os("BEVY_ASSET_ROOT")
        .map(|root| PathBuf::from(root).join("assets"))
        .into_iter()
        .chain([runtime_root().join("assets")])
        .chain(beside_exe())
        .find(|dir| dir.is_dir())
        .unwrap_or_else(|| runtime_root().join("assets"))
}

#[cfg(test)]
mod tests {
    #[test]
    fn asset_files_are_found_under_the_assets_root() {
        let root = super::assets_root();
        for file in ["sfx/button_press.wav", "textures/ui/logo.png"] {
            assert!(root.join(file).is_file(), "{file}");
        }
    }
}
