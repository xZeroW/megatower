//! Bevy capability plugins that sit between the simulation and the screens:
//! kira audio, input actions and bindings, and the off-screen room and HUD
//! layout rendering. None of them knows about screens.
#![allow(
    clippy::too_many_arguments,
    clippy::type_complexity,
    clippy::drop_non_drop,
    clippy::field_reassign_with_default
)]

pub mod audio;
pub mod input;
pub mod render;
