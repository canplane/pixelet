// pxl-kernel/layer/object/meta.rs

//! What a placed thing carries beyond its value: decor's variant and heading,
//! seeded by where it is placed, its one stable identity.

use pxl_spec::cell::decor::{DecorKind, DecorMeta};
use pxl_spec::cell::object::{decor_id_from_num, object_value_is_decor};
use pxl_spec::procedural::MASTER_SEED;
use pxl_spec::space::coord::pvf3::ivec::IVec;
use xpute_kit::math::rng::{derive_seed_u64, u32_to_unit, SplitMix64};

use crate::layer::instance::{Instance, Meta, Source};
use crate::layer::object::store::Entry;

/// Seeded only by the placed object's position, its one stable identity, so
/// the hover ghost predicts exactly the variant and heading a placement gets.
pub fn user_decor_pick(i: IVec, variant_count: u32) -> (u32, f64) {
    let mut stream = SplitMix64::new(derive_seed_u64(MASTER_SEED, i));
    let mut rng = || stream.next_u32();
    let variant = ((u32_to_unit(rng()) * variant_count as f64).floor() as u32).min(variant_count - 1);
    let rotation = u32_to_unit(rng()) * core::f64::consts::PI * 2.0;
    (variant, rotation)
}

/// A placed decor as one instance: it stands at its cube's bottom middle; none
/// for anything else placed.
pub fn placed(e: &Entry) -> Option<Instance> {
    if !object_value_is_decor(e.state) {
        return None;
    }
    let (c, kind) = (e.cube(), DecorKind::of(decor_id_from_num(e.state)));
    let (variant, rotation) = user_decor_pick(e.corner().i, kind.variant_count());
    Some(Instance {
        id: kind.value(),
        at: [(c.min[0] + c.max[0]) / 2.0, (c.min[1] + c.max[1]) / 2.0, c.min[2]],
        source: Source::Object(e.id()),
        meta: Meta::Decor(DecorMeta { variant, rotation, scale: 1.0 }),
    })
}
