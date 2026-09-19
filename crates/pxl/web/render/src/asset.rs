// pxl-render/asset.rs

//! What ships with the build, read into what the device can hold: the glTF
//! the models come in, and the one texture every billboard's bitmap is packed
//! into. Which assets exist is the spec's (pxl-spec asset/); what is here is
//! how their bytes become a mesh or a sheet.

pub mod atlas;
pub mod gltf;
