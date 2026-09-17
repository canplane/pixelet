// pxl-kernel/layer/character/variant.rs

//! The characters a player can be, as variant.json lists them: each group's
//! variants by number, row for row with their models, the colormap the group's
//! pack shares, how tall it stands and which clip is its walk. A variant's
//! name, and how the picker splits the people by gender, are the shell's.

use pxl_spec::asset::ingested::Asset;

use crate::layer::character::variant_spec::groups::{person, pet};
pub use crate::layer::character::variant_spec::{DEFAULT_PERSON_VARIANT_KEY, DEFAULT_PET_VARIANT_KEY};

/// Which of variant.json's groups a character is of.
pub type CharacterGroup = crate::layer::character::variant_spec::Group;

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct CharacterVariant {
    pub group: CharacterGroup,
    /// Its number in its group, as the settings store holds it.
    pub number: u8,
    pub model: Asset,
    pub skin: Asset,
    /// Rendering is natively in units throughout, so height_u is exactly the
    /// on-screen height the model is scaled to, and it is the group's
    /// `size`: one full cell tall for the person, a quarter cell for the pet.
    pub height_u: f64,
    /// The catch-up/preview move's clip, by its animation's index.
    pub move_clip: u32,
}

/// A variant by its group and number, or none for a number the group does not
/// give out.
pub fn character_variant(group: CharacterGroup, number: u8) -> Option<CharacterVariant> {
    let (model, skin, height_u, move_clip) = match group {
        CharacterGroup::person => {
            let row = person::Key::ALL.iter().position(|k| *k as u8 == number)?;
            (person::models[row], person::skin, person::size, person::move_clip)
        }
        CharacterGroup::pet => {
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
