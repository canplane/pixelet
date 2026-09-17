// pxl_core/module/world/character/variant.rs

//! The characters a player can be, as variant.json lists them: a group's keys
//! row for row with its models, the colormap the group's pack shares, how tall
//! it stands and which clip is its walk. Each model and skin is fetched by the
//! content key the asset ingest gave it.
//!
//! Split by gender (not one flat list) so the settings UI can show two
//! separate rows instead of one row a user has to scan by label text to tell
//! apart — display names live in locale files, keyed off `key`.

pub use super::variant_spec::{DEFAULT_PERSON_VARIANT_KEY, DEFAULT_PET_VARIANT_KEY};
use super::variant_spec::{MOVE_CLIP, PERSON, PERSON_GENDER_PREFIXES, PET};
use crate::lib::env::route::shipped_asset_url;
use crate::sys::global::Global;

#[derive(Clone, Debug, PartialEq)]
pub struct CharacterVariant {
    pub key: String,
    pub model_url: String,
    pub skin_url: String,
    /// Rendering is natively in units throughout, so height_u is exactly the
    /// on-screen height the model is scaled to, and it is the group's
    /// `size`: one full cell tall for the person, a quarter cell for the pet.
    pub height_u: f64,
    /// Clip name for the catch-up/preview move.
    pub move_clip: &'static str,
}

fn variants_of(keys: &[&str], models: &[&str], skin: &str, height_u: f64, move_clip: &'static str) -> Vec<CharacterVariant> {
    assert_eq!(keys.len(), models.len(), "character: variant.json lists {} keys and {} models", keys.len(), models.len());
    let skin_url = shipped_asset_url(skin);
    keys.iter()
        .zip(models)
        .map(|(key, model)| CharacterVariant {
            key: key.to_string(),
            model_url: shipped_asset_url(model),
            skin_url: skin_url.clone(),
            height_u,
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
        let persons = variants_of(PERSON::keys, PERSON::models, PERSON::skin, PERSON::size, MOVE_CLIP::person);
        Variants {
            female: persons.iter().filter(|v| v.key.starts_with(PERSON_GENDER_PREFIXES::female)).cloned().collect(),
            male: persons.iter().filter(|v| v.key.starts_with(PERSON_GENDER_PREFIXES::male)).cloned().collect(),
            pets: variants_of(PET::keys, PET::models, PET::skin, PET::size, MOVE_CLIP::pet),
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
