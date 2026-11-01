//! Tags on skills and supports, and the damage types.
//!
//! A modifier can require tags; it applies only when the context (the skill
//! being used, plus the damage types a hit packet has been) has all of them.

use serde::{Deserialize, Serialize};
use std::ops::{BitOr, BitOrAssign};

/// Damage types in conversion order: damage only converts towards later types.
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
pub enum DamageType {
    Physical,
    Lightning,
    Cold,
    Fire,
    Chaos,
}

impl DamageType {
    pub const ALL: [DamageType; 5] = [
        DamageType::Physical,
        DamageType::Lightning,
        DamageType::Cold,
        DamageType::Fire,
        DamageType::Chaos,
    ];

    pub const fn index(self) -> usize {
        self as usize
    }

    pub const fn tag(self) -> Tag {
        match self {
            DamageType::Physical => Tag::Physical,
            DamageType::Lightning => Tag::Lightning,
            DamageType::Cold => Tag::Cold,
            DamageType::Fire => Tag::Fire,
            DamageType::Chaos => Tag::Chaos,
        }
    }

    pub fn is_elemental(self) -> bool {
        matches!(
            self,
            DamageType::Lightning | DamageType::Cold | DamageType::Fire
        )
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum Tag {
    Attack,
    Spell,
    Melee,
    Projectile,
    Area,
    Chain,
    Duration,
    Physical,
    Lightning,
    Cold,
    Fire,
    Chaos,
}

impl Tag {
    const fn bit(self) -> u32 {
        1 << self as u32
    }
}

/// A set of [`Tag`]s. Written in RON as a list, e.g. `[Spell, Fire]`.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(from = "Vec<Tag>", into = "Vec<Tag>")]
pub struct Tags(u32);

impl Tags {
    pub const NONE: Tags = Tags(0);
    const DAMAGE_TYPES: Tags = Tags(
        Tag::Physical.bit()
            | Tag::Lightning.bit()
            | Tag::Cold.bit()
            | Tag::Fire.bit()
            | Tag::Chaos.bit(),
    );

    pub fn of(tags: &[Tag]) -> Self {
        tags.iter().fold(Tags::NONE, |set, &tag| set | tag)
    }

    pub const fn has(self, tag: Tag) -> bool {
        self.0 & tag.bit() != 0
    }

    /// Every tag of `required` is in `self`.
    pub const fn contains_all(self, required: Tags) -> bool {
        self.0 & required.0 == required.0
    }

    /// At least one tag in common.
    pub const fn intersects(self, other: Tags) -> bool {
        self.0 & other.0 != 0
    }

    /// The tags as bits, in [`Tag`] order.
    pub const fn bits(self) -> u32 {
        self.0
    }

    pub const fn is_empty(self) -> bool {
        self.0 == 0
    }

    /// The set with its damage type tags removed. A fire skill's physical
    /// damage is not fire damage, so damage modifiers see a packet's own types
    /// instead of the skill's.
    pub const fn without_damage_types(self) -> Self {
        Tags(self.0 & !Self::DAMAGE_TYPES.0)
    }
}

impl BitOr for Tags {
    type Output = Tags;
    fn bitor(self, rhs: Tags) -> Tags {
        Tags(self.0 | rhs.0)
    }
}

impl BitOr<Tag> for Tags {
    type Output = Tags;
    fn bitor(self, rhs: Tag) -> Tags {
        Tags(self.0 | rhs.bit())
    }
}

impl BitOrAssign<Tag> for Tags {
    fn bitor_assign(&mut self, rhs: Tag) {
        self.0 |= rhs.bit();
    }
}

impl From<Tag> for Tags {
    fn from(tag: Tag) -> Self {
        Tags(tag.bit())
    }
}

impl From<Vec<Tag>> for Tags {
    fn from(tags: Vec<Tag>) -> Self {
        Tags::of(&tags)
    }
}

impl From<Tags> for Vec<Tag> {
    fn from(tags: Tags) -> Self {
        const ALL: [Tag; 12] = [
            Tag::Attack,
            Tag::Spell,
            Tag::Melee,
            Tag::Projectile,
            Tag::Area,
            Tag::Chain,
            Tag::Duration,
            Tag::Physical,
            Tag::Lightning,
            Tag::Cold,
            Tag::Fire,
            Tag::Chaos,
        ];
        ALL.into_iter().filter(|&t| tags.has(t)).collect()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn contains_all_needs_every_tag() {
        let fireball = Tags::of(&[Tag::Spell, Tag::Projectile, Tag::Fire]);
        assert!(fireball.contains_all(Tags::of(&[Tag::Spell, Tag::Fire])));
        assert!(!fireball.contains_all(Tags::of(&[Tag::Spell, Tag::Melee])));
        assert!(fireball.contains_all(Tags::NONE));
    }

    #[test]
    fn strips_damage_types() {
        let fireball = Tags::of(&[Tag::Spell, Tag::Fire]);
        assert_eq!(fireball.without_damage_types(), Tags::from(Tag::Spell));
    }

    #[test]
    fn round_trips_as_a_list() {
        let tags = Tags::of(&[Tag::Attack, Tag::Cold]);
        let list: Vec<Tag> = tags.into();
        assert_eq!(list, vec![Tag::Attack, Tag::Cold]);
        assert_eq!(Tags::from(list), tags);
    }
}
