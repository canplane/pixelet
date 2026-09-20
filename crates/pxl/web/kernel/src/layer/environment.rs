// pxl-kernel/layer/environment.rs

//! What time of day it is and what that does to the light: where the sun and
//! moon are, which theme is in force, and the state a frame is lit from. The
//! themes are theme.json's; what is here is reading the clock and choosing
//! between them.

pub mod celestial_position;
#[path = "environment/config.spec.rs"]
pub mod config;
pub mod state;
pub mod theme;
pub mod theme_clock;
#[path = "environment/theme.spec.rs"]
pub mod theme_spec;
