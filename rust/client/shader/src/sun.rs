// pxl-shader/sun.rs

//! The world's depth from the sun into its clipmap (pxl-spec-core `sun`):
//! the tiles that may walk again listed (`sun_mark_cs`), then their slots
//! walked (`sun_cs`).

use pxl_spec_core::cast::{self, Detail, Until};
use pxl_spec_core::fill::put;
use pxl_spec_core::{at, ways};
use spirv_std::glam::{IVec4, UVec3, UVec4, Vec4};
use spirv_std::memory::{Scope, Semantics};
use spirv_std::{spirv, Image};

use crate::add;
use crate::eye::{at_word, eye_origin, Eye};
use crate::world::{Unlooked, World};

/// The world's depth from the sun, a slot of the sun's clipmap each
/// (pxl-spec-core `sun`): the slot's texel of the light box, and its ray
/// from the box's near plane along the sun's rays, walked through the
/// world's one query to the block that fills (`cast::first`) as the eye's
/// are, each node asked at the zoom the eye's rays ask it, and the word it
/// leaves there; none walked where the slot held its world texel last cast
/// and the eye asks its stop as it did (`Sun::kept`, `Sun::stands`), its
/// ray passing over no node the world's query answers otherwise since
/// (`moved`, `sun::over`). Its one layer read and written in place, the one
/// format family WebGPU lets a shader do both to, as the rays' are.
type SunWords = Image!(2D, format = r32ui, sampled = false, arrayed = true);

/// The nodes the world's query answers otherwise than when the sun's map
/// was last cast (pxl-spec-core `sun::MOVED_WORDS`): how many, then each X,
/// Y and its zoom.
#[repr(C)]
pub struct SunMoved {
    pub len: UVec4,
    pub at: [IVec4; pxl_spec_core::sun::MOVED],
}
const _: () = assert!(core::mem::size_of::<SunMoved>() == at_word(pxl_spec_core::sun::MOVED_WORDS));

/// The sun's cast off the eye's words (pxl-spec-core `sun::Sun::words`).
fn sun_of(eye: &Eye) -> pxl_spec_core::sun::Sun {
    let v3 = |v: Vec4| [v.x, v.y, v.z];
    let i = |v: Vec4| v.w.to_bits() as i32;
    pxl_spec_core::sun::Sun {
        first: v3(eye.sun[0]),
        reach: eye.sun[0].w,
        across: v3(eye.sun[1]),
        down: v3(eye.sun[2]),
        dir: v3(eye.sun[3]),
        bias: v3(eye.sun[4]),
        corner: [i(eye.sun[1]), i(eye.sun[2])],
        kept: [i(eye.sun[3]), i(eye.sun[4])],
    }
}

/// Slot `slot` of the clipmap `size` a side: its texel's ray, from where
/// and at what zooms, and whether it walks again, where its world texel
/// was not held, the eye asks its stop at another zoom, or its ray to its
/// stop passes over a node moved (pxl-spec-core `sun::over`).
fn sun_slot(
    eye: &Eye,
    sun: &SunWords,
    moved: &SunMoved,
    size: u32,
    slot: UVec3,
) -> (bool, [i32; 3], cast::Ray, Detail) {
    let s = sun_of(eye);
    let p = s.texel(size, [slot.x, slot.y]);
    let (origin, mut detail) = eye_origin(eye);
    let (o, ray, from) = pxl_spec_core::sun::ray(origin, &s, p);
    detail.eye = from;
    let mut walks = true;
    if s.kept(size, p) {
        let held: u32 = sun.read(slot);
        if s.stands(detail, held) {
            let t = pxl_spec_core::sun::depth_of(held) * s.reach;
            let mut k = 0;
            while k < moved.len.x {
                let n = moved.at[k as usize];
                if pxl_spec_core::sun::over(o, s.dir, t, [n.x, n.y], n.z as u32) {
                    break;
                }
                k += 1;
            }
            walks = k < moved.len.x;
        }
    }
    (walks, o, ray, detail)
}

/// The sun's clipmap's tiles a slot of which may walk again, listed
/// (pxl-spec-core `sun::TILE`): a thread a tile, judged whole — whether its
/// run of box texels was all kept, whether the eye asks the zoom its stops
/// were cast at wherever in their box it looks (`Sun::tile_stands`), and
/// whether its rays together pass over a node moved (`sun::over_any`) —
/// and the tile appended where it may, its row of the walk's dispatch with
/// it; the walk judges each slot of a listed tile itself (`sun_slot`).
/// Judged a tile at once: every slot's own judgement, its rays against every
/// node moved, costs the shadows tens of milliseconds a moving frame.
#[spirv(compute(threads(64, 1, 1), entry_point_name = "sun_mark_cs"))]
#[allow(clippy::too_many_arguments)]
pub fn sun_mark_cs(
    #[spirv(global_invocation_id)] id: UVec3,
    #[spirv(uniform, descriptor_set = 0, binding = 0)] eye: &Eye,
    #[spirv(descriptor_set = 0, binding = 10)] sun: &SunWords,
    #[spirv(uniform, descriptor_set = 0, binding = 11)] moved: &SunMoved,
    #[spirv(storage_buffer, descriptor_set = 0, binding = 12)] list: &mut [u32],
) {
    let size: UVec3 = sun.query_size();
    let t = pxl_spec_core::sun::TILE;
    let across = size.x / t;
    if id.x >= across * across {
        return;
    }
    let tile = [id.x % across, id.x / across];
    let s = sun_of(eye);
    let (p, run) = s.tile_texel(size.x, tile);
    let mut walks = !run || !s.tile_kept(size.x, p);
    if !walks {
        // Its slots' words, as one: the stops they hold and the zoom they
        // were asked at, judged where they lie as a box.
        let mut held = pxl_spec_core::sun::Held::NONE;
        let mut k = 0;
        while k < t * t {
            let word: u32 = sun.read(UVec3::new(tile[0] * t + k % t, tile[1] * t + k / t, 0));
            held = held.with(word);
            k += 1;
        }
        let (origin, detail) = eye_origin(eye);
        walks = !s.tile_stands(detail, p, held);
        if !walks && moved.len.x != 0 {
            // Its rays as one, to the deepest stop.
            let (o, off) = s.tile_rays(origin, p);
            let reach = held.reach as f32 * s.reach / pxl_spec_core::sun::DEPTH_ONE as f32;
            let mut n = 0;
            while n < moved.len.x {
                let m = moved.at[n as usize];
                if pxl_spec_core::sun::over_any(o, off, s.dir, reach, [m.x, m.y], m.z as u32) {
                    break;
                }
                n += 1;
            }
            walks = n < moved.len.x;
        }
    }
    if walks {
        let at = add(list, pxl_spec_core::sun::LIST_COUNT as u32, 1);
        let row = at / pxl_spec_core::sun::LIST_ACROSS + 1;
        if pxl_spec_core::sun::LIST_ROWS < list.len() {
            // SAFETY: just bounded by the slice's own length.
            unsafe {
                spirv_std::arch::atomic_u_max::<
                    u32,
                    { Scope::QueueFamily as u32 },
                    { Semantics::NONE.bits() },
                >(list.get_unchecked_mut(pxl_spec_core::sun::LIST_ROWS), row)
            };
        }
        put(
            list,
            pxl_spec_core::sun::LIST_AT as u32 + at,
            tile[1] << 16 | tile[0],
        );
    }
}

const _: () = assert!(
    pxl_spec_core::sun::TILE == 8,
    "`sun_cs` is a workgroup a tile, its threads a side of it"
);

/// The world's depth from the sun into the slots the mark found walk again
/// (`sun_mark_cs`): a workgroup a listed tile, a thread a slot, each walked
/// through the world's one query to the block that fills (`cast::first`)
/// as the eye's rays are, and the word it leaves there.
#[spirv(compute(threads(8, 8, 1), entry_point_name = "sun_cs"))]
#[allow(clippy::too_many_arguments)]
pub fn sun_cs(
    #[spirv(workgroup_id)] group: UVec3,
    #[spirv(local_invocation_id)] lane: UVec3,
    #[spirv(uniform, descriptor_set = 0, binding = 0)] eye: &Eye,
    #[spirv(storage_buffer, descriptor_set = 0, binding = 1)] window: &mut [IVec4],
    #[spirv(storage_buffer, descriptor_set = 0, binding = 2)] ground: &[u32],
    #[spirv(storage_buffer, descriptor_set = 0, binding = 5)] shards: &[IVec4],
    #[spirv(storage_buffer, descriptor_set = 0, binding = 6)] overwrites: &[u32],
    #[spirv(storage_buffer, descriptor_set = 0, binding = 7)] pool: &[u32],
    #[spirv(descriptor_set = 0, binding = 10)] sun: &SunWords,
    #[spirv(uniform, descriptor_set = 0, binding = 11)] moved: &SunMoved,
    #[spirv(storage_buffer, descriptor_set = 0, binding = 12)] list: &[u32],
) {
    let window: &[IVec4] = window;
    let mut world = World {
        window,
        ground,
        looks: Unlooked(0),
        shards,
        overwrites,
        eye,
        pool: ways::tree::Read(pool),
    };
    let t = pxl_spec_core::sun::TILE;
    let size: UVec3 = sun.query_size();
    let k = group.y * pxl_spec_core::sun::LIST_ACROSS + group.x;
    if k >= at(list, pxl_spec_core::sun::LIST_COUNT as u32) {
        return;
    }
    let tile = at(list, pxl_spec_core::sun::LIST_AT as u32 + k);
    let slot = UVec3::new((tile & 0xffff) * t + lane.x, (tile >> 16) * t + lane.y, 0);
    if slot.x >= size.x || slot.y >= size.y {
        return;
    }
    let (walks, o, ray, detail) = sun_slot(eye, sun, moved, size.x, slot);
    if !walks {
        return;
    }
    let s = sun_of(eye);
    let met = cast::first(&mut world, o, &ray, detail, Until::Fills);
    // SAFETY: within the map, bounded by its size above.
    unsafe { sun.write(slot, s.word(detail, pxl_spec_core::sun::depth(&s, &met))) };
}
