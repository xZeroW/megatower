//! Native counterparts of the data rules in `save_system.gd` and `file.gd`.
//! The Godot encrypted `var_to_bytes` + gzip container, `inst_to_dict`
//! reflection and `load_data` property poking are NOT reproduced: existing
//! Godot save files cannot be read, and native saves are plain JSON.

use serde_json::{Map, Value};
use std::{fs, io, path::Path};

/// `get_value` for floats: negative infinity is stored as the string "-inf".
pub fn encode_float(value: f64) -> Value {
    if value == f64::NEG_INFINITY {
        Value::String("-inf".into())
    } else {
        serde_json::Number::from_f64(value).map_or(Value::Null, Value::Number)
    }
}

/// `deserialize` for a float property: "-inf" restores negative infinity,
/// any other string is returned unchanged by the source (here: `None`).
pub fn decode_float(value: &Value) -> Option<f64> {
    match value {
        Value::String(s) if s == "-inf" => Some(f64::NEG_INFINITY),
        Value::Number(n) => n.as_f64(),
        _ => None,
    }
}

/// `get_data` key filter: properties starting with `@` or `_` are not saved.
pub fn is_saved_property(name: &str) -> bool {
    !name.starts_with('@') && !name.starts_with('_')
}

pub fn filter_saved(map: &Map<String, Value>) -> Map<String, Value> {
    map.iter()
        .filter(|(k, _)| is_saved_property(k))
        .map(|(k, v)| (k.clone(), v.clone()))
        .collect()
}

/// `deserialize` for a `Resource` property: only `res://` string paths that
/// exist load; other strings pass through; anything else is null.
pub enum ResourceRef<'a> {
    Loaded(&'a str),
    PassThrough(&'a str),
    Null,
}

pub fn resolve_resource_ref(value: &Value, exists: impl Fn(&str) -> bool) -> ResourceRef<'_> {
    match value {
        Value::String(s) if !s.starts_with("res://") => ResourceRef::PassThrough(s),
        Value::String(s) if exists(s) => ResourceRef::Loaded(s),
        _ => ResourceRef::Null,
    }
}

/// `save_json`/`load_json` equivalents. `load` returns `None` when the file
/// is missing (the source returns without touching the object).
pub fn save_json(path: &Path, data: &Map<String, Value>) -> io::Result<()> {
    if let Some(parent) = path.parent() {
        fs::create_dir_all(parent)?;
    }
    fs::write(path, serde_json::to_vec(&filter_saved(data))?)
}

pub fn load_json(path: &Path) -> Option<Map<String, Value>> {
    match serde_json::from_slice::<Value>(&fs::read(path).ok()?).ok()? {
        Value::Object(map) => Some(map),
        _ => None,
    }
}

/// `File.get_user_file_dir` / `get_file_dir` without Steam resolve to
/// `user://`; here the per-user data directory, overridable for tests.
pub fn user_file_dir() -> std::path::PathBuf {
    std::env::var_os("MEGALOOT_USER_DIR")
        .map(Into::into)
        .unwrap_or_else(|| {
            std::env::var_os("XDG_DATA_HOME")
                .map(std::path::PathBuf::from)
                .or_else(|| std::env::var_os("HOME").map(|h| Path::new(&h).join(".local/share")))
                .unwrap_or_else(|| ".".into())
                .join("megaloot")
        })
}

/// `File.json_to_dict`: parse errors and non-object roots give an empty map.
pub fn json_to_dict(text: &str) -> Map<String, Value> {
    match serde_json::from_str::<Value>(text) {
        Ok(Value::Object(m)) => m,
        _ => Map::new(),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    #[test]
    fn neg_inf_and_filters() {
        assert_eq!(encode_float(f64::NEG_INFINITY), json!("-inf"));
        assert_eq!(decode_float(&json!("-inf")), Some(f64::NEG_INFINITY));
        assert_eq!(decode_float(&json!("x")), None);
        let m = json!({"a":1,"_b":2,"@c":3}).as_object().unwrap().clone();
        assert_eq!(filter_saved(&m).len(), 1);
    }

    #[test]
    fn resource_refs_and_roundtrip() {
        let v = json!("res://a.tres");
        assert!(matches!(
            resolve_resource_ref(&v, |_| true),
            ResourceRef::Loaded(_)
        ));
        assert!(matches!(
            resolve_resource_ref(&v, |_| false),
            ResourceRef::Null
        ));
        assert!(matches!(
            resolve_resource_ref(&json!("plain"), |_| false),
            ResourceRef::PassThrough("plain")
        ));
        assert!(matches!(
            resolve_resource_ref(&json!(3), |_| true),
            ResourceRef::Null
        ));
        let dir = std::env::temp_dir().join(format!("megaloot-save-{}", std::process::id()));
        let p = dir.join("x.json");
        save_json(&p, json!({"k":1,"_h":2}).as_object().unwrap()).unwrap();
        assert_eq!(
            load_json(&p).unwrap(),
            json!({"k":1}).as_object().unwrap().clone()
        );
        assert!(load_json(&dir.join("none.json")).is_none());
        let _ = fs::remove_dir_all(dir);
        assert!(json_to_dict("[1]").is_empty());
    }
}
