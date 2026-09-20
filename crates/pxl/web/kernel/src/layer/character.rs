// pxl-kernel/layer/character.rs

//! The companion that stands at the map's centre — the person and pet a
//! player picked, how they move, the bubble over their head, and the display
//! cases the picker shows them in. Which characters exist is variant.json's;
//! what is here is one standing in the world.

pub mod chat_bubble;
pub mod companion;
pub mod path;
pub mod preview;
pub mod store;
pub mod variant;
#[path = "character/variant.spec.rs"]
pub mod variant_spec;
