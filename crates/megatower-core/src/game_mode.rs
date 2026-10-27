//! `game_mode.gd`, `game_mode_script.gd` and the Challenge/Practice scripts.
//! PvP is deferred with multiplayer. Stale serialized `total_battles` in the
//! resources is not part of the current schema, so `base_room_count` is 4.

use crate::stats::modify;

#[derive(Clone, Copy, Debug, Eq, PartialEq, Hash)]
pub enum Room {
    Entrance,
    Final,
    Merchant,
    Battle,
    Chest,
    EndlessMonolith,
    MysticTrader,
    ReforgeTransfer,
    EnemyUpgrade,
    Dismantle,
    Forest,
}

impl Room {
    /// Debug lookup by the source `Rooms` constant name (case-insensitive);
    /// used by `MEGALOOT_START_ROOM`.
    pub fn from_debug_name(name: &str) -> Option<Self> {
        Some(match name.to_ascii_uppercase().as_str() {
            "ENTRANCE" => Self::Entrance,
            "FINAL" => Self::Final,
            "MERCHANT" => Self::Merchant,
            "BATTLE" => Self::Battle,
            "CHEST" => Self::Chest,
            "ENDLESS_MONOLITH" => Self::EndlessMonolith,
            "MYSTIC_TRADER" => Self::MysticTrader,
            "REFORGE_TRANSFER_ROOM" => Self::ReforgeTransfer,
            "ENEMY_UPGRADE" => Self::EnemyUpgrade,
            "DISMANTLE" => Self::Dismantle,
            "FOREST_ROOM" => Self::Forest,
            _ => return None,
        })
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq, Hash)]
pub enum GameMode {
    Challenge,
    Practice,
}

impl GameMode {
    pub const LIST: [Self; 2] = [Self::Challenge, Self::Practice];
    pub fn id(self) -> &'static str {
        match self {
            Self::Challenge => "challenge",
            Self::Practice => "practice",
        }
    }
    pub fn last_floor(self) -> i32 {
        match self {
            Self::Challenge => 100,
            Self::Practice => -1,
        }
    }
    pub fn base_room_count(self) -> i32 {
        4
    }
    pub fn script(self, endless: bool) -> GameModeScript {
        GameModeScript {
            mode: self,
            endless,
        }
    }
}

/// Stat inputs for `get_gold_on_kill(player, ...)`.
#[derive(Clone, Copy, Debug)]
pub struct GoldStats {
    pub gold_on_kill: f64,
    pub greed: f64,
}

#[derive(Clone, Copy, Debug)]
pub struct GameModeScript {
    pub mode: GameMode,
    /// `memory.is_endless`.
    pub endless: bool,
}

impl GameModeScript {
    pub fn room_type(&self, floor: i32, room_idx: i32) -> Room {
        if room_idx == -1 {
            return Room::Entrance;
        }
        if !self.endless && floor >= self.mode.last_floor() {
            return Room::Final;
        }
        if self.is_last_room(floor, room_idx) {
            return self.last_room(floor);
        }
        if self.is_chest_floor(floor) && room_idx == self.room_count(floor) - 2 {
            return Room::Chest;
        }
        Room::Battle
    }

    pub fn last_room(&self, floor: i32) -> Room {
        match self.mode {
            GameMode::Challenge => {
                let number = floor + 1;
                if number == 100 {
                    Room::Battle
                } else if number % 10 == 0 {
                    Room::Dismantle
                } else if [7, 17, 27, 37].contains(&number)
                    || (floor >= 39 && (floor - 37) % 20 == 0)
                {
                    Room::MysticTrader
                } else if [1, 3, 7, 15].contains(&number) || (floor >= 17 && (floor - 15) % 8 == 0)
                {
                    Room::Merchant
                } else {
                    Room::Battle
                }
            }
            GameMode::Practice => {
                if (floor + 1).to_string().contains('7') {
                    Room::MysticTrader
                } else if self.is_chest_floor(floor + 2) || self.is_chest_floor(floor + 1) {
                    Room::Merchant
                } else {
                    Room::Battle
                }
            }
        }
    }

    pub fn is_chest_floor(&self, floor: i32) -> bool {
        floor % 3 == 2
    }

    pub fn is_last_room(&self, floor: i32, room_idx: i32) -> bool {
        room_idx >= self.room_count(floor) - 1
    }

    pub fn room_count(&self, floor: i32) -> i32 {
        match self.mode {
            GameMode::Challenge => {
                let mut count = match floor {
                    59.. => 1,
                    29.. => 2,
                    14.. => 3,
                    _ => 4,
                };
                if self.last_room(floor) != Room::Battle {
                    count += 1;
                }
                count
            }
            GameMode::Practice => {
                self.mode.base_room_count() + i32::from(self.is_chest_floor(floor))
            }
        }
    }

    /// Source uses the STOP floor's room count for every earlier floor.
    pub fn rooms_up_to_floor(&self, stop_floor: i32, stop_room_idx: i32) -> Vec<Room> {
        let _ = stop_room_idx;
        let count = self.room_count(stop_floor);
        (0..stop_floor)
            .flat_map(|floor| (0..count).map(move |room| (floor, room)))
            .map(|(floor, room)| self.room_type(floor, room))
            .collect()
    }

    pub fn rooms_in_floor(&self, floor: i32, stop_room_idx: i32) -> Vec<Room> {
        (0..stop_room_idx.min(self.room_count(floor)))
            .map(|room| self.room_type(floor, room))
            .collect()
    }

    pub fn battles_this_floor(&self, floor: i32) -> usize {
        self.rooms_in_floor(floor, self.room_count(floor))
            .iter()
            .filter(|room| **room == Room::Battle)
            .count()
    }

    pub fn battle_count(&self, floor: i32, room_idx: i32) -> usize {
        self.rooms_in_floor(floor, room_idx)
            .iter()
            .filter(|room| **room == Room::Battle)
            .count()
    }

    pub fn total_battle_count(&self, floor: i32, room_idx: i32) -> usize {
        self.rooms_up_to_floor(floor, room_idx)
            .iter()
            .filter(|room| **room == Room::Battle)
            .count()
    }

    pub fn max_enemies(&self, floor: i32, room_idx: i32) -> i32 {
        let mut max = self.battle_count(floor, room_idx) as i32 + 1;
        if self.mode == GameMode::Challenge {
            max += 3.min((f64::from(floor + 1) / 10.0).floor() as i32);
        }
        max
    }

    pub fn gold_reward(&self, floor: i32) -> f64 {
        match self.mode {
            GameMode::Challenge => f64::from((floor + 1) * 25),
            GameMode::Practice => f64::from((floor + 1) * 5),
        }
    }

    pub fn gold_on_kill(&self, player: Option<GoldStats>, floor: i32) -> f64 {
        let mut reward = self.gold_reward(floor);
        if let Some(stats) = player {
            reward += stats.gold_on_kill;
            reward = modify(reward, stats.greed);
        }
        reward.ceil()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::Value;

    fn room(name: &str) -> Room {
        match name {
            "ENTRANCE" => Room::Entrance,
            "FINAL" => Room::Final,
            "MERCHANT" => Room::Merchant,
            "BATTLE" => Room::Battle,
            "CHEST" => Room::Chest,
            "ENDLESS_MONOLITH" => Room::EndlessMonolith,
            "MYSTIC_TRADER" => Room::MysticTrader,
            "REFORGE_TRANSFER_ROOM" => Room::ReforgeTransfer,
            "ENEMY_UPGRADE" => Room::EnemyUpgrade,
            "DISMANTLE" => Room::Dismantle,
            "FOREST_ROOM" => Room::Forest,
            other => panic!("{other}"),
        }
    }

    fn mode(id: &Value) -> GameMode {
        *GameMode::LIST.iter().find(|mode| mode.id() == id).unwrap()
    }

    #[test]
    fn room_sequences_counts_and_gold_match_executed_scripts() {
        let data: Value =
            serde_json::from_str(include_str!("../../../artifacts/game-mode-golden.json")).unwrap();
        let rows = data["rows"].as_array().unwrap();
        let floors = data["floors"].as_array().unwrap();
        assert_eq!((rows.len(), floors.len()), (6040, 604));
        let players = [
            None,
            Some(GoldStats {
                gold_on_kill: 0.0,
                greed: 0.0,
            }),
            Some(GoldStats {
                gold_on_kill: 7.0,
                greed: 33.0,
            }),
            Some(GoldStats {
                gold_on_kill: 120.0,
                greed: -20.0,
            }),
        ];
        for row in rows {
            let script = mode(&row[0]).script(row[1].as_bool().unwrap());
            let (floor, idx) = (
                row[2].as_i64().unwrap() as i32,
                row[3].as_i64().unwrap() as i32,
            );
            let why = format!("{row}");
            assert_eq!(
                script.room_type(floor, idx),
                room(row[4].as_str().unwrap()),
                "{why}"
            );
            assert_eq!(
                i64::from(script.room_count(floor)),
                row[5].as_i64().unwrap(),
                "{why}"
            );
            assert_eq!(
                i64::from(script.max_enemies(floor, idx)),
                row[6].as_i64().unwrap(),
                "{why}"
            );
            assert_eq!(script.gold_reward(floor), row[7].as_f64().unwrap(), "{why}");
            assert_eq!(
                script.battle_count(floor, idx) as i64,
                row[8].as_i64().unwrap(),
                "{why}"
            );
            assert_eq!(
                script.is_last_room(floor, idx),
                row[9].as_bool().unwrap(),
                "{why}"
            );
            for (player, expected) in players.iter().zip(row[10].as_array().unwrap()) {
                assert_eq!(
                    script.gold_on_kill(*player, floor),
                    expected.as_f64().unwrap(),
                    "{why}"
                );
            }
        }
        for row in floors {
            let script = mode(&row[0]).script(row[1].as_bool().unwrap());
            let floor = row[2].as_i64().unwrap() as i32;
            let why = format!("{row}");
            assert_eq!(
                script.last_room(floor),
                room(row[3].as_str().unwrap()),
                "{why}"
            );
            assert_eq!(
                script.battles_this_floor(floor) as i64,
                row[4].as_i64().unwrap(),
                "{why}"
            );
            assert_eq!(
                script.total_battle_count(floor, 0) as i64,
                row[5].as_i64().unwrap(),
                "{why}"
            );
            assert_eq!(
                script.total_battle_count(floor, 3) as i64,
                row[6].as_i64().unwrap(),
                "{why}"
            );
        }
    }

    #[test]
    fn practice_without_endless_is_always_final_because_last_floor_is_negative() {
        let script = GameMode::Practice.script(false);
        assert_eq!(script.room_type(0, 0), Room::Final);
        assert_eq!(
            GameMode::Practice.script(true).room_type(0, 0),
            Room::Battle
        );
    }
}
