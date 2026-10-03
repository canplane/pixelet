// pxl-kernel/layer/character/variant.rs

//! The characters a player can be, as variant.json lists them.

use pxl_spec::asset::ingested::Asset;

use crate::layer::character::variant_spec::groups::{person, pet};
pub use crate::layer::character::variant_spec::Group;

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct CharacterVariant {
    pub group: Group,
    /// As the settings store holds it.
    pub number: u8,
    pub model: Asset,
    pub skin: Asset,
    /// The on-screen height in units: a cell for the person, a quarter for the pet.
    pub height_u: f64,
    /// The walk clip, by its animation's index.
    pub move_clip: u32,
}

pub const CHARACTER_VARIANTS: usize = person::Key::ALL.len() + pet::Key::ALL.len();

pub fn character_variant(group: Group, number: u8) -> Option<CharacterVariant> {
    let (model, skin, height_u, move_clip) = match group {
        Group::person => {
            let row = person::Key::ALL.iter().position(|k| *k as u8 == number)?;
            (person::models[row], person::skin, person::size, person::move_clip)
        }
        Group::pet => {
            let row = pet::Key::ALL.iter().position(|k| *k as u8 == number)?;
            (pet::models[row], pet::skin, pet::size, pet::move_clip)
        }
    };
    Some(CharacterVariant {
        group,
        number,
        model,
        skin,
        height_u,
        move_clip,
    })
}
