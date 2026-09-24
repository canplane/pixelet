// pxl-kernel/layer/environment.rs

//! What time of day it is and what that does to the light: where the sun and
//! moon are, which theme is in force, and the state a frame is lit from. The
//! themes themselves are theme.rs's tables; the rest is reading the clock
//! and choosing between them.

pub mod celestial_position;
pub mod config;
pub mod fog;
pub mod state;
pub mod theme;
pub mod theme_clock;
