// pxl-kernel/layer/decor/kind.rs

//! The decor kinds, one file each; adding one is a file here and its `DEF`
//! in `KINDS`.

use pxl_spec::cell::object::decor_num_id;
use pxl_spec::cell::DecorId;

use crate::layer::decor::build::{largest, variant_extent, DecorVariantAsset};
use crate::layer::decor::scatter::{DecorKindSeed, DecorOrientationMode, DecorRules};

pub mod boat;
pub mod car;
pub mod train;
pub mod tree;

pub struct Kind {
    pub seed: DecorKindSeed,
    pub rules: DecorRules,
    pub variants: &'static [DecorVariantAsset],
    /// None for a kind nobody places by hand.
    pub decor_id: Option<DecorId>,
}

/// In group order: a kind's variants are consecutive groups of every decor block.
pub static KINDS: [&Kind; 4] = [&tree::DEF, &boat::DEF, &car::DEF, &train::DEF];

impl Kind {
    pub fn variant_count(&self) -> u32 {
        self.variants.len() as u32
    }

    pub fn align_transport(&self) -> bool {
        self.seed.orientation == DecorOrientationMode::AlignTransport
    }

    /// The largest dimension over every variant's model, in units.
    pub fn extent(&self) -> f64 {
        largest(self.variants.iter().map(variant_extent))
    }

    /// The value a placed one is stored as.
    pub fn decor_value(&self) -> Option<u32> {
        self.decor_id.map(decor_num_id)
    }

    /// The variants of the kinds before it.
    pub fn first_group(&self) -> u32 {
        KINDS.iter().take_while(|k| k.seed.tag != self.seed.tag).map(|k| k.variant_count()).sum()
    }
}

pub fn kind_for(decor_id: DecorId) -> Option<&'static Kind> {
    KINDS.iter().copied().find(|k| k.decor_id == Some(decor_id))
}
