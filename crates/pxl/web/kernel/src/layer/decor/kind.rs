// pxl-kernel/layer/decor/kind.rs

//! The decor kinds, one file each; adding one is a file here and its `DEF`
//! in the registry.

use pxl_spec::cell::DecorId;

use crate::layer::decor::build::DecorVariantAsset;
use crate::layer::decor::scatter::{DecorKindSeed, DecorRules};

pub mod boat;
pub mod car;
pub mod train;
pub mod tree;

pub struct DecorKindDef {
    pub seed: DecorKindSeed,
    pub rules: DecorRules,
    pub variants: &'static [DecorVariantAsset],
    /// None for a kind nobody places by hand.
    pub decor_id: Option<DecorId>,
}
