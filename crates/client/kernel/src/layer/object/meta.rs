// pxl-kernel/layer/object/meta.rs

//! What a placed thing carries beyond its value: decor's variant and heading,
//! seeded by where it is placed, its one stable identity.

use pxl_spec::block::decor::Decor;
use pxl_spec::block::object::{decor_id_from_num, object_value_is_decor};
use pxl_spec::procedural::MASTER_SEED;
use pxl_spec::space::coord::pvf3::ivec::IVec;
use xpute_kit::math::rng::{derive_seed_u64, u32_to_unit, SplitMix64};

use crate::layer::instance::{Instance, Meta, Source};
use crate::layer::object::store::Entry;

/// Seeded only by the placed object's position, its one stable identity, so
/// the hover ghost predicts exactly the variant and quarter turns a placement gets.
pub fn user_decor_pick(i: IVec, variant_count: u32) -> (u32, u32) {
    let mut stream = SplitMix64::new(derive_seed_u64(MASTER_SEED, i));
    let mut rng = || stream.next_u32();
    let variant = ((u32_to_unit(rng()) * variant_count as f64).floor() as u32).min(variant_count - 1);
    let turn = (u32_to_unit(rng()) * 4.0) as u32;
    (variant, turn)
}

/// A placed decor as one instance, its box's min corner at its entry's; none
/// for anything else placed.
pub fn placed(e: &Entry) -> Option<Instance> {
    if !object_value_is_decor(e.state) {
        return None;
    }
    let decor = Decor::of(decor_id_from_num(e.state));
    let (variant, turn) = user_decor_pick(e.corner().i, decor.variant_count());
    let (at, meta) = decor.stand(e.cube().min, variant, turn, 1.0);
    Some(Instance {
        id: decor.value(),
        at,
        source: Source::Object(e.id()),
        meta: Meta::Decor(meta),
    })
}
