// pxl-spec/asset/sky.spec.rs
//
// GENERATED from spec/spec/asset/sky.json (xpute-gen data.rs)
//
// The sky: what hangs in the sky, claiming no cell, so no
// cell holds it. Each by its files' paths under asset/content/; what a
// path is, its number and its extent, is the catalog's (pxl_spec::asset::catalog).

// Names as the spec spells them, so one record has one name on every side.
// A table is generated whole, whether or not this crate calls an entry.
#![allow(
    non_upper_case_globals,
    non_snake_case,
    dead_code,
    reason = "generated from the spec"
)]

/// The cumulus, drawn on the wind's moving field as decor is drawn, each
/// cloud one of its models (pxl-core domain/scene/environment/cloud.rs).
/// Quaternius, CC0.
pub mod cumulus {
    pub const models: [u16; 3] = [66, 67, 68];
    pub const skin: u16 = 69;
}

/// The moon's near side, the picture the sky draws it with (NASA).
pub mod moon {
    pub const picture: u16 = 70;
}
