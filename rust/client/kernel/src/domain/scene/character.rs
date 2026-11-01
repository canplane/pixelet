// pxl-kernel/domain/scene/character.rs

//! The companion at the pivot and the picker's previews, and the characters
//! a player can be, as the actor kind's catalog lists them
//! (`spec/spec/asset/actor.json`).

pub mod companion;
pub mod param;
pub mod path;
pub mod preview;

use pxl_spec::asset::actor::groups::{person, pet};
pub use pxl_spec::asset::actor::Group;
use pxl_spec::asset::catalog::Slot;

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Character {
    pub group: Group,
    /// As the settings store holds it.
    pub number: u8,
    pub model: Slot,
    pub skin: Slot,
    /// The on-screen height in units: a cell for the person, a quarter for the pet.
    pub height_u: f64,
    /// The walk clip, by its animation's index.
    pub move_clip: u32,
}

pub const CHARACTERS: usize = person::models.len() + pet::models.len();

/// A character's model, registered once at its own height, which a display
/// preview's instance scales.
pub const MODELS_MAX: usize = CHARACTERS;

impl Character {
    /// The character numbered `number` in `group`; none past the group's rows.
    pub fn of(group: Group, number: u8) -> Option<Character> {
        let (model, skin, height_u, move_clip) = match group {
            Group::person => (
                *person::models.get(usize::from(number))?,
                person::skin,
                person::size,
                person::move_clip,
            ),
            Group::pet => (
                *pet::models.get(usize::from(number))?,
                pet::skin,
                pet::size,
                pet::move_clip,
            ),
        };
        Some(Character {
            group,
            number,
            model,
            skin,
            height_u,
            move_clip,
        })
    }
}
