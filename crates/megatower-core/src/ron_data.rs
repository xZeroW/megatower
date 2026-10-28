//! Loading of the RON data files under `assets/data/`.

use serde::de::DeserializeOwned;

/// Parses a data file's text; `name` is only for the panic message, since a
/// malformed embedded file is a build mistake, not a runtime condition.
pub fn from_str<T: DeserializeOwned>(text: &str, name: &str) -> T {
    ron::from_str(text).unwrap_or_else(|e| panic!("{name}: {e}"))
}
