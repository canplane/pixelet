// pxl-kernel/model.rs

//! glTF models, each registered once as an uploaded mesh with its joints and
//! clips: the scene's actors play them and decor stands them in chunks.

pub mod asset;
pub mod registry;
/// The characters a player can be, by their models (variant.json).
#[path = "model/variant.spec.rs"]
pub mod variant_spec;
