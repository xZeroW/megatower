//! Dependency direction between the workspace crates (see README
//! "Architecture"): the app composes, lower crates never reach upward.

use std::{collections::BTreeSet, fs, path::Path};

fn internal_deps(manifest: &Path) -> BTreeSet<String> {
    let text = fs::read_to_string(manifest).unwrap_or_else(|_| panic!("{}", manifest.display()));
    text.lines()
        .filter_map(|line| line.split_once('=').map(|(key, _)| key.trim()))
        .filter(|key| key.starts_with("megatower-"))
        .map(str::to_owned)
        .collect()
}

/// Each crate and the only internal crates it may depend on.
const ALLOWED: &[(&str, &[&str])] = &[
    ("megatower-sim", &["megatower-stats"]),
    ("megatower-stats", &[]),
    ("megatower-core", &[]),
    ("megatower-assets", &["megatower-core"]),
    ("megatower-runtime", &["megatower-core", "megatower-assets"]),
    (
        "megatower-ui",
        &[
            "megatower-sim",
            "megatower-core",
            "megatower-assets",
            "megatower-runtime",
        ],
    ),
    (
        "megatower-client",
        &[
            "megatower-core",
            "megatower-assets",
            "megatower-runtime",
            "megatower-ui",
        ],
    ),
];

#[test]
fn crates_only_depend_downwards() {
    let root = Path::new(env!("CARGO_MANIFEST_DIR")).join("../../crates");
    for (name, allowed) in ALLOWED {
        let found = internal_deps(&root.join(name).join("Cargo.toml"));
        let extra: Vec<_> = found
            .iter()
            .filter(|dep| !allowed.contains(&dep.as_str()))
            .collect();
        assert!(extra.is_empty(), "{name} must not depend on {extra:?}");
    }
}

#[test]
fn every_crate_is_covered() {
    let root = Path::new(env!("CARGO_MANIFEST_DIR")).join("../../crates");
    for entry in fs::read_dir(root).unwrap() {
        let name = entry.unwrap().file_name().to_string_lossy().into_owned();
        assert!(
            ALLOWED.iter().any(|(n, _)| *n == name),
            "{name} is missing from the architecture table"
        );
    }
}
