//! `singletons/trials.gd` identities. Rewards/BB text are not ported here.

use serde::{Deserialize, Serialize};

#[derive(Clone, Copy, Debug, Deserialize, Serialize, Eq, PartialEq, Hash)]
#[serde(rename_all = "snake_case")]
pub enum Trial {
    Honor,
    Resilience,
    Swarm,
    Confrontation,
    Sagacity,
}

impl Trial {
    /// `Trials.LIST` source order.
    pub const LIST: [Self; 5] = [
        Self::Honor,
        Self::Resilience,
        Self::Swarm,
        Self::Confrontation,
        Self::Sagacity,
    ];
}
