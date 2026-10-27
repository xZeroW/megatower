//! Item definitions and instance rules. Native containers and partial creation/
//! interaction policies live in `inventory`; live manager integration is pending.

pub mod catalog;
pub mod definition;
pub mod instance;
pub mod lookup;
pub mod rarity;

use crate::stats::StatId;

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Price {
    pub currency: Option<StatId>,
    pub amount: f64,
}

#[cfg(test)]
mod tests;

#[cfg(test)]
mod catalog_tests;
