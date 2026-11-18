// pxl-spec/asset/sky.spec.rs
//
// GENERATED from spec/spec/asset/sky.json5 (xpute-gen data.rs)
//
// The sky: what hangs in the sky, claiming no cell, so no
// cell holds it. Each by its files' paths under asset/content/; what a
// path is, its number and its extent, is the catalog's (pxl_spec::asset::catalog).

/// The cumulus, drawn on the wind's moving field as decor is drawn, each
/// cloud one of its models (pxl-core domain/scene/environment/cloud.rs).
/// Quaternius, CC0.
pub mod cumulus {
    pub const MODELS: [u16; 3] = [66, 67, 68];
    pub const SKIN: u16 = 69;
}

/// The moon's near side, the image the sky draws it with (NASA).
pub mod moon {
    pub const IMAGE: u16 = 70;
}
