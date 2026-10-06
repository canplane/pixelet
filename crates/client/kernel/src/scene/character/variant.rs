// pxl-kernel/scene/character/variant.rs

//! The characters a player can be, as variant.json lists them.

use pxl_spec::asset::ingested::Asset;

use crate::io::load::{load_gltf, Load};
use crate::scene::model::asset::{register_model, ModelHandle};
use xpute_guest::global::Global;

use crate::scene::character::variant_spec::groups::{person, pet};
pub use crate::scene::character::variant_spec::Group;

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

/// A variant at its own height, and at one unit in a display case.
pub const MODELS_MAX: usize = 2 * CHARACTER_VARIANTS;

/// None inside while registration failed.
static MODELS: Global<Vec<(Asset, u64, Option<ModelHandle>)>> = Global::new();

/// `asset` registered `height` units tall, once.
pub fn model_of(asset: Asset, height: f64) -> Option<ModelHandle> {
    let models = MODELS.get(Vec::new);
    if let Some((_, _, entry)) = models.iter().find(|(a, h, _)| *a == asset && *h == height.to_bits()) {
        return entry.clone();
    }
    let gltf = load_gltf(Load::Asset(asset))?;
    let handle = register_model(&gltf, Some(height));
    models.push((asset, height.to_bits(), handle.clone()));
    handle
}

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
