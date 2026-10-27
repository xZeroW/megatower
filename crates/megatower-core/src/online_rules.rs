//! Pure rules of online/utility singletons: `Leaderboards` (speedrun.com
//! request URLs and response parsing), `HTTP` response gate,
//! `ThreadManager` queue, `AssetManager` override paths and skins,
//! `ConnectionOption` status art, achievement image names. No network or
//! file IO happens here.

use crate::definitions::BorderType;
use serde_json::Value;

pub const GAME_API_URL: &str =
    "https://www.speedrun.com/api/v1/games/megaloot?embed=categories,variables,leaderboard";

/// `HTTP.send_request` callback gate: only a 200 response reaches the callable.
pub fn http_accepts(response_code: i32) -> bool {
    response_code == 200
}

/// `Leaderboards.send_leaderboards_request` URL for one category.
/// `game_variables` is `game_data["variables"]["data"]`.
pub fn leaderboard_url(
    category_name: &str,
    game_variables: &[Value],
    version: &str,
    adventurer_name: &str,
) -> String {
    let mut url = String::from("https://www.speedrun.com/api/v1/leaderboards/megaloot/category/");
    url += &category_name.to_lowercase().replace(' ', "-");
    url += "?top=100";
    let (mut version_var, mut version_id) = (String::new(), String::new());
    let (mut adv_var, mut adv_id) = (String::new(), String::new());
    for v in game_variables {
        if v["name"] == "Version" {
            version_var = v["id"].as_str().unwrap_or("").to_string();
            if let Some(choices) = v["values"]["choices"].as_object() {
                for (id, name) in choices {
                    if name.as_str() == Some(version) {
                        version_id = id.clone();
                    }
                }
            }
        }
    }
    for v in game_variables {
        if v["name"] == "Adventurer" {
            adv_var = v["id"].as_str().unwrap_or("").to_string();
            if let Some(choices) = v["values"]["choices"].as_object() {
                for (id, name) in choices {
                    if name.as_str().map(str::to_lowercase) == Some(adventurer_name.to_lowercase())
                    {
                        adv_id = id.clone();
                    }
                }
            }
        }
    }
    url += &format!("&var-{version_var}={version_id}");
    url += &format!("&var-{adv_var}={adv_id}");
    url += "&embed=players";
    url
}

/// `get_adventurer_name_from_id`.
pub fn adventurer_name_from_id(game_variables: &[Value], id: &str) -> String {
    for v in game_variables {
        if v["name"] == "Adventurer" {
            if let Some(name) = v["values"]["choices"].get(id) {
                return name.as_str().unwrap_or("").to_string();
            }
        }
    }
    String::new()
}

/// `get_category_name_from_id`.
pub fn category_name_from_id(categories: &[Value], id: &str) -> String {
    categories
        .iter()
        .find(|c| c["id"] == id)
        .and_then(|c| c["name"].as_str())
        .unwrap_or("")
        .to_string()
}

/// One parsed leaderboard run (`Leaderboard.Record`).
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Record {
    pub username: String,
    pub time: i64,
    pub user_id: String,
}

/// `parse_data`: the i-th run uses the i-th embedded player's name.
pub fn parse_runs(leaderboard: &Value) -> Vec<Record> {
    let runs = leaderboard["runs"].as_array().cloned().unwrap_or_default();
    runs.iter()
        .enumerate()
        .map(|(idx, run)| Record {
            username: leaderboard["players"]["data"][idx]["names"]["international"]
                .as_str()
                .unwrap_or("")
                .to_string(),
            time: run["run"]["times"]["primary_t"].as_f64().unwrap_or(0.0) as i64,
            user_id: run["run"]["players"][0]["id"]
                .as_str()
                .unwrap_or("")
                .to_string(),
        })
        .collect()
}

/// `ThreadManager`: a busy thread queues callables; once idle, the LAST
/// queued callable runs and the rest are discarded.
#[derive(Debug, Default)]
pub struct ThreadQueue<T> {
    pub queue: Vec<T>,
    pub running: bool,
}

impl<T> ThreadQueue<T> {
    pub fn start(&mut self, job: T) -> Option<T> {
        if self.running {
            self.queue.push(job);
            return None;
        }
        self.running = true;
        Some(job)
    }

    /// `_process` when the thread is idle.
    pub fn poll(&mut self) -> Option<T> {
        if self.running {
            return None;
        }
        let job = self.queue.pop();
        if job.is_some() {
            self.queue.clear();
            self.running = true;
        }
        job
    }

    pub fn finish(&mut self) {
        self.running = false;
    }
}

/// `AssetManager` per-kind override file name: `<name>_override.png`.
pub fn override_path(save_path: &str) -> String {
    format!("{save_path}_override.png")
}

/// `AssetManager.update_skins` command line flags.
#[derive(Debug, PartialEq, Eq)]
pub struct Skins {
    pub hand_of_blood: bool,
    pub xqc: bool,
    pub jesse: bool,
}

pub fn skins(args: &[String]) -> Skins {
    let has = |f: &str| args.iter().any(|a| a == f);
    Skins {
        hand_of_blood: has("-hand_of_blood"),
        xqc: has("-xqc"),
        jesse: has("-jesse"),
    }
}

/// Achievement badge file name: `<adventurer>_<border>.png`.
pub fn achievement_file(adventurer: &str, border: BorderType) -> String {
    format!(
        "{}_{}.png",
        adventurer.to_lowercase(),
        format!("{border:?}").to_lowercase()
    )
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ConnectionStatus {
    Connected,
    Disconnected,
    Connecting,
}

/// `ConnectionOption.set_connection_status`: (atlas x, status visible,
/// loading visible).
pub fn connection_art(status: ConnectionStatus) -> (f32, bool, bool) {
    match status {
        ConnectionStatus::Connected => (0.0, true, false),
        ConnectionStatus::Disconnected => (9.0, true, false),
        ConnectionStatus::Connecting => (0.0, false, true),
    }
}

/// `SpeedrunOption`: a full-length key triggers authentication, anything
/// else disconnects.
pub fn speedrun_key_action(key_len: usize, required_len: usize) -> bool {
    key_len == required_len
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    #[test]
    fn urls_and_parsing() {
        let vars = vec![
            json!({"name":"Version","id":"v1","values":{"choices":{"c1":"1.23.1","c2":"1.0"}}}),
            json!({"name":"Adventurer","id":"a1","values":{"choices":{"x":"Sid","y":"Arthur"}}}),
        ];
        let url = leaderboard_url("Any Percent", &vars, "1.23.1", "ARTHUR");
        assert_eq!(
            url,
            "https://www.speedrun.com/api/v1/leaderboards/megaloot/category/any-percent?top=100&var-v1=c1&var-a1=y&embed=players"
        );
        assert_eq!(adventurer_name_from_id(&vars, "x"), "Sid");
        assert_eq!(adventurer_name_from_id(&vars, "z"), "");
        assert_eq!(
            category_name_from_id(&[json!({"id":"k","name":"Any %"})], "k"),
            "Any %"
        );
        let lb = json!({
            "runs":[{"run":{"times":{"primary_t":125.0},"players":[{"id":"u1"}]}}],
            "players":{"data":[{"names":{"international":"Zed"}}]}
        });
        assert_eq!(
            parse_runs(&lb),
            vec![Record {
                username: "Zed".into(),
                time: 125,
                user_id: "u1".into()
            }]
        );
        assert!(http_accepts(200) && !http_accepts(404));
    }

    #[test]
    fn queue_and_misc() {
        let mut q = ThreadQueue::default();
        assert_eq!(q.start(1), Some(1));
        assert_eq!(q.start(2), None);
        assert_eq!(q.start(3), None);
        assert_eq!(q.poll(), None);
        q.finish();
        assert_eq!(q.poll(), Some(3));
        assert!(q.queue.is_empty());
        q.finish();
        assert_eq!(q.poll(), None);
        assert_eq!(override_path("a/b"), "a/b_override.png");
        assert_eq!(
            skins(&["-xqc".into()]),
            Skins {
                hand_of_blood: false,
                xqc: true,
                jesse: false
            }
        );
        assert_eq!(
            achievement_file("Arthur", BorderType::Silver),
            "arthur_silver.png"
        );
        assert_eq!(
            connection_art(ConnectionStatus::Connecting),
            (0.0, false, true)
        );
        assert!(speedrun_key_action(4, 4) && !speedrun_key_action(3, 4));
    }
}
