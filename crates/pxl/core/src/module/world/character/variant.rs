// pxl_core/module/world/character/variant.rs

//! The characters a player can be, read off the palette's actor rows
//! (pxl_shared::record::shipped): a person is the "person" group, a pet the
//! "pet" group, and each row names its model and its skin by content key. What
//! this adds is the actor tier's own knowledge — which clip is its walk —
//! from variant.json, which the palette does not carry.
//!
//! Split by gender (not one flat list) so the settings UI can show two
//! separate rows instead of one row a user has to scan by label text to tell
//! apart — display names live in locale files, keyed off `key`.

use pxl_shared::record::shipped::shipped_palette;

use super::variant_spec::{MOVE_CLIP, PERSON_GENDER_PREFIXES};
use crate::global::Global;
use crate::lib::env::route::asset_url;

pub use super::variant_spec::{DEFAULT_PERSON_VARIANT_KEY, DEFAULT_PET_VARIANT_KEY};

#[derive(Clone, Debug, PartialEq)]
pub struct CharacterVariant {
    pub key: String,
    pub model_url: String,
    pub skin_url: String,
    /// Rendering is natively in units throughout, so height_u is exactly the
    /// on-screen height the model is scaled to, and it is the palette row's
    /// `size`: one full cell tall for the person, a quarter cell for the pet.
    pub height_u: f64,
    /// Clip name for the catch-up/preview move.
    pub move_clip: &'static str,
}

fn variants_of(group: &str, move_clip: &'static str) -> Vec<CharacterVariant> {
    let palette = shipped_palette();
    palette
        .rows_of_group(group)
        .unwrap_or_default()
        .into_iter()
        .map(|row| CharacterVariant {
            key: palette.name_of(row).unwrap_or_default(),
            model_url: asset_url(&palette.model_key_of(row).unwrap_or_default()).to_string(),
            skin_url: asset_url(&palette.skin_key_of(row).unwrap_or_default()).to_string(),
            height_u: palette.size.get(row as usize) as f64,
            move_clip,
        })
        .collect()
}

struct Variants {
    female: Vec<CharacterVariant>,
    male: Vec<CharacterVariant>,
    pets: Vec<CharacterVariant>,
}

static VARIANTS: Global<Variants> = Global::new();

fn variants() -> &'static Variants {
    VARIANTS.get(|| {
        let persons = variants_of("person", MOVE_CLIP::person);
        Variants {
            female: persons.iter().filter(|v| v.key.starts_with(PERSON_GENDER_PREFIXES::female)).cloned().collect(),
            male: persons.iter().filter(|v| v.key.starts_with(PERSON_GENDER_PREFIXES::male)).cloned().collect(),
            pets: variants_of("pet", MOVE_CLIP::pet),
        }
    })
}

#[allow(non_snake_case)]
pub fn PERSON_VARIANTS_FEMALE() -> &'static [CharacterVariant] {
    &variants().female
}

#[allow(non_snake_case)]
pub fn PERSON_VARIANTS_MALE() -> &'static [CharacterVariant] {
    &variants().male
}

#[allow(non_snake_case)]
pub fn PET_VARIANTS() -> &'static [CharacterVariant] {
    &variants().pets
}

pub fn find_person_variant(key: &str) -> Option<&'static CharacterVariant> {
    let v = variants();
    v.female.iter().find(|x| x.key == key).or_else(|| v.male.iter().find(|x| x.key == key)).or_else(|| v.female.first())
}

pub fn find_pet_variant(key: &str) -> Option<&'static CharacterVariant> {
    let v = variants();
    v.pets.iter().find(|x| x.key == key).or_else(|| v.pets.first())
}
