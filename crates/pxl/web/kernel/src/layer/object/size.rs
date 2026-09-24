// pxl-kernel/layer/object/size.rs

//! What a placed object may be sized to, and which of those a cube offers.
//!
//! The ladder itself is spec/pxl/spec/bake/cell.json's, generated into both
//! languages. What is here is the kernel's use of it: the default and decor's
//! fixed size.

pub use pxl_spec::bake::cell_spec::{ObjectScaleExponent, OBJECT_SCALE_EXPONENTS, OBJECT_SIZES};

/// The tier a fresh session places at: the coarsest, one full cell.
pub const DEFAULT_OBJECT_SIZE: f64 = OBJECT_SIZES[0];

/// Every decor kind places at exactly one terrain cell — unlike a color
/// cube, decor has no mode_bar size flyout of its own (yet) to vary this, and
/// a scattered tree already assumes its own candidates are one full cell
/// wide. Deliberately the same value as OBJECT_SIZES[0], not a re-export of
/// it — object_size is a live, user-changed selection for the object tool
/// specifically; this is decor's own fixed constant and must keep working
/// even if a future size flyout moves object_size elsewhere.
pub const DECOR_SIZE: f64 = OBJECT_SIZES[0];

const DEFAULT_ACTIVE_SCALES: &[ObjectScaleExponent] = &OBJECT_SCALE_EXPONENTS;

/// Per-cube override of which of OBJECT_SIZES' tiers mode_bar's size flyout
/// offers — empty today (every cube, flat color or terrain-styled, offers
/// every tier), reserved for a future one that shouldn't (a huge painted
/// mural block, say). Keyed by the full PlaceableCubeId since the hotbar's
/// own MRU list can surface a terrain-styled pick too.
const ACTIVE_SCALES: &[(&str, &[ObjectScaleExponent])] = &[];

pub fn object_scale_exponents(id: &str) -> &'static [ObjectScaleExponent] {
    ACTIVE_SCALES.iter().find(|(key, _)| *key == id).map_or(DEFAULT_ACTIVE_SCALES, |(_, scales)| scales)
}
