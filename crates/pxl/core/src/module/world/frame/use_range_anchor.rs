// pxl_core/module/world/frame/use_range_anchor.rs

use pxl_shared::spec::space::pvf3::ivec::IVec;

use super::range::range_anchor_of;
use crate::module::map::store::use_map_store;

/// The one anchor every range is measured from: what mounts, what is fetched,
/// what stays resident.
///
/// Chunk-aligned, so it changes once per 64u of travel rather than every
/// frame, and each consumer re-derives nothing. It is the view centre on the
/// ground, not the camera: residency has to be invariant under rotation, or
/// spinning at a wide zoom would sweep pages in and out of the cache. What the
/// camera itself measures is visibility and tier — see
/// issue/20260912-what-to-render-when.md.
pub fn use_range_anchor() -> Option<IVec> {
    use_map_store().map_center.map(|c| range_anchor_of(c.i))
}
