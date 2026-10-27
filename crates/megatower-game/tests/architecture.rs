//! Dependency direction between the workspace crates (see README
//! "Architecture"): the app composes, capability crates never reach upward.

use std::{collections::BTreeSet, fs, path::Path};

fn internal_deps(name: &str) -> BTreeSet<String> {
    let manifest = Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("..")
        .join(name)
        .join("Cargo.toml");
    let text = fs::read_to_string(&manifest).unwrap_or_else(|_| panic!("{}", manifest.display()));
    text.lines()
        .filter_map(|line| line.split_once('=').map(|(key, _)| key.trim()))
        .filter(|key| key.starts_with("megatower-"))
        .map(str::to_owned)
        .collect()
}

/// Each crate and the only internal crates it may depend on.
const ALLOWED: &[(&str, &[&str])] = &[
    ("megatower-core", &[]),
    ("megatower-ui", &["megatower-core"]),
    ("megatower-gameplay", &["megatower-core", "megatower-ui"]),
    ("megatower-input", &[]),
    ("megatower-settings", &["megatower-gameplay"]),
    (
        "megatower-profile",
        &["megatower-core", "megatower-gameplay"],
    ),
    (
        "megatower-layout",
        &["megatower-core", "megatower-gameplay"],
    ),
    ("megatower-audio", &["megatower-core"]),
    (
        "megatower-render",
        &["megatower-core", "megatower-layout", "megatower-gameplay"],
    ),
    (
        "megatower-frontend",
        &[
            "megatower-audio",
            "megatower-core",
            "megatower-gameplay",
            "megatower-input",
            "megatower-layout",
            "megatower-profile",
            "megatower-render",
            "megatower-settings",
            "megatower-ui",
        ],
    ),
    (
        "megatower-dev",
        &[
            "megatower-frontend",
            "megatower-gameplay",
            "megatower-render",
        ],
    ),
];

#[test]
fn crates_only_depend_downwards() {
    for (name, allowed) in ALLOWED {
        let found = internal_deps(name);
        let extra: Vec<_> = found
            .iter()
            .filter(|dep| !allowed.contains(&dep.as_str()))
            .collect();
        assert!(extra.is_empty(), "{name} must not depend on {extra:?}");
    }
}

#[test]
fn every_crate_is_covered() {
    let crates = Path::new(env!("CARGO_MANIFEST_DIR")).join("..");
    for entry in fs::read_dir(crates).unwrap() {
        let name = entry.unwrap().file_name().to_string_lossy().into_owned();
        assert!(
            name == "megatower-game" || ALLOWED.iter().any(|(n, _)| *n == name),
            "{name} is missing from the architecture table"
        );
    }
}
