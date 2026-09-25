// pxl-kernel/layer/decor/kind.rs

//! The kinds themselves, one file each: what models a kind uses, what it may
//! stand on, and how thickly it scatters, as one `DecorKindDef`. Adding one
//! is a file here and its `DEF` in the registry (decor/registry.rs), and
//! nothing else.

use pxl_spec::cell::DecorId;

use crate::layer::decor::build::DecorVariantAsset;
use crate::layer::decor::scatter::{DecorKindSeed, DecorRules};

pub mod boat;
pub mod car;
pub mod train;
pub mod tree;

/// A kind as its file writes it.
/// A kind's rule is per ground channel: the channel's types it stands on,
/// the channels that must hold nothing, and any other channel is anything.
/// The scatter stays generic, so a later kind (a fence along a road edge, a
/// candle inside a structure, ...) is one more file and one more entry in
/// the registry.
pub struct DecorKindDef {
    pub seed: DecorKindSeed,
    /// Where it stands.
    pub rules: DecorRules,
    pub variants: &'static [DecorVariantAsset],
    /// The object store's decor value a placed instance of the kind carries;
    /// none for a kind nobody places by hand.
    pub decor_id: Option<DecorId>,
}
