//! Bevy capability plugins that sit between the simulation and the screens:
//! kira audio, input actions and bindings, and the off-screen room and HUD
//! layout rendering. None of them knows about screens.

pub mod audio;
pub mod input;
pub mod render;
