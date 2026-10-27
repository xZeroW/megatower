//! Every `get_amount_to_upgrade` in `resources/stat_scripts/`.
//! Faith's cleanse and Luck's attack observers are NOT implemented here.

use crate::game_math::diminished_stat;
use serde::{Deserialize, Serialize};

#[derive(Clone, Copy, Debug, Deserialize, Serialize, Eq, PartialEq)]
#[serde(rename_all = "snake_case")]
pub enum StatUpgrade {
    AdaptiveAttack,
    Agility,
    Armor,
    ArmorShields,
    ArmorSteal,
    ArmoredAttack,
    CinderDamage,
    Combat,
    CritChance,
    CriticalDamage,
    Dazzle,
    Electricity,
    ElementalPower,
    ElementalResistance,
    Faith,
    Flow,
    FreezeAttack,
    GoldOnKill,
    Greed,
    Insight,
    LifeSteal,
    Luck,
    MagicAttack,
    Malice,
    MaxHealth,
    MaxMana,
    Penetration,
    PhysicalAttack,
    Power,
    Recovery,
    TotalAttacks,
    Toughness,
    Toxicity,
    Wisdom,
}

impl StatUpgrade {
    /// `weapon` means exactly SocketTypes.WEAPON; replicated weapons do not
    /// satisfy that source check. Missing item references pass `false`.
    pub fn amount(self, weapon: bool, modifier: bool, floor: i32) -> f64 {
        use StatUpgrade::*;
        let level = f64::from(floor) + 1.0;
        if weapon
            && matches!(
                self,
                Agility | Armor | ArmorShields | ArmoredAttack | FreezeAttack | PhysicalAttack
            )
        {
            return 0.0;
        }
        match self {
            AdaptiveAttack | Luck | MagicAttack | Recovery | TotalAttacks | Toughness
            | Toxicity | Wisdom => 0.0,
            Agility => level.min(25.0),
            Armor | Combat | Dazzle | Electricity | FreezeAttack | Malice => {
                if modifier {
                    diminished_stat(floor + 1, 25.0, 7.5)
                } else {
                    level.powf(1.12).ceil()
                }
            }
            PhysicalAttack => {
                if modifier {
                    diminished_stat(floor + 1, 25.0, 7.5)
                } else {
                    level.powf(1.15).ceil()
                }
            }
            ArmorShields | MaxMana => 1.0,
            ArmorSteal | ArmoredAttack => (level * 0.25).ceil().min(10.0),
            CinderDamage => {
                if modifier {
                    level
                } else {
                    0.0
                }
            }
            CritChance | ElementalResistance | LifeSteal => level.min(10.0),
            CriticalDamage => f64::from(floor) * 5.0,
            ElementalPower => diminished_stat(floor + 1, 75.0, 45.0),
            Faith | Power => (level * 2.5).floor(),
            Flow => {
                if modifier {
                    0.0
                } else {
                    5.0
                }
            }
            GoldOnKill => {
                if modifier {
                    0.0
                } else {
                    diminished_stat(floor + 1, 45.0, 75.0)
                }
            }
            Greed => (level * 0.25).ceil().min(25.0),
            Insight => (level * 0.25).ceil().min(7.0),
            MaxHealth => {
                if modifier {
                    diminished_stat(floor + 1, 25.0, 10.0)
                } else {
                    (level * 2.5).floor()
                }
            }
            Penetration => (level * 0.45).ceil().min(10.0),
        }
    }
}
