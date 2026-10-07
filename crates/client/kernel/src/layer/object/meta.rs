// pxl-kernel/layer/object/meta.rs

//! What a placed thing carries beyond its value: decor's variant and heading,
//! seeded by where it is placed, its one stable identity.

use pxl_spec::procedural::MASTER_SEED;
use pxl_spec::space::coord::pvf3::ivec::IVec;
use xpute_core::math::rng::{derive_seed_u64, u32_to_unit, SplitMix64};

/// Seeded only by the placed object's position, its one stable identity, so
/// the hover ghost predicts exactly the variant and heading a placement gets.
pub fn user_decor_pick(i: IVec, variant_count: u32) -> (u32, f64) {
    let mut stream = SplitMix64::new(derive_seed_u64(MASTER_SEED, i));
    let mut rng = || stream.next_u32();
    let variant = ((u32_to_unit(rng()) * variant_count as f64).floor() as u32).min(variant_count - 1);
    let rotation = u32_to_unit(rng()) * core::f64::consts::PI * 2.0;
    (variant, rotation)
}
