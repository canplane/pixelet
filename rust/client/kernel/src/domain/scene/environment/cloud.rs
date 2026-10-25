// pxl-kernel/domain/scene/environment/cloud.rs

//! The cumulus, the sky kind's (`spec/spec/asset/sky.json` `cumulus`), drawn
//! as decor is and held as decor is not. It claims no cell,
//! so nothing is built around it and a tower stands through it; it is no
//! layer and no window's square holds it. It is laid on the field's own map, which the
//! wind carries (`pxl-spec-core cloud.rs` `drift`): a block of cells there
//! is built once and drawn moved, each instance a model, a sprite or nothing
//! by its size on the screen (`present_block`), by the tiers the GPU draws
//! decor by, one rule (pxl-spec-core scatter.rs `drawn::tier`). A block is
//! laid in the lanes the decor and impostor shaders read
//! (domain/scene/lane.rs).

use fnv::FnvHashMap;

use pxl_render::frame::draw::{DrawList, PIPELINE_DECOR, PIPELINE_GHOST_DECOR, PIPELINE_IMPOSTOR};
use pxl_render::gpu::pass::decor::GROUPS;
use pxl_render::look::environment::{ENV_AIR, ENV_SKY};
use pxl_spec::asset::ingested::Asset;
use pxl_spec::asset::sky::cumulus;
use pxl_spec::space::coord::v3::{self, V3};
use pxl_spec::space::coord::world::nearest;
use pxl_spec_core::cloud::{drift, lattice_hash, CUMULUS_KM, WIND};
use pxl_spec_core::scatter::drawn;
use xpute_guest::global::Global;
use xpute_guest::mem::base::ptr_at;
use xpute_guest::mem::heap::{alloc, free};
use xpute_guest::sched::frame_budget::FrameBudget;

use crate::domain::query::model::asset::model_of;
use crate::domain::scene::decor::drawn::{decor_gone_px, decor_sprite_px, sprite_reach};
use crate::domain::scene::frame::Frame;
use crate::domain::scene::lane::{self, GroupLanes};
use crate::domain::scene::scene;
use crate::domain::view::frame::pose::{km_per_u, units_per_px_at};
use crate::domain::view::observation::eye;
use crate::domain::view::setting::store::settings_store;
use crate::sys::io::loader::load_image;
use crate::sys::load::Load;

/// Its models, a cloud each one of them.
const VARIANTS: [Asset; cumulus::models.len()] = cumulus::models;

pub const MODELS_MAX: usize = VARIANTS.len();

/// Its draw groups follow the decors' variants, which number the groups
/// before it (pxl-spec-core scatter.rs `GROUPS`).
const FIRST_GROUP: usize = pxl_spec_core::scatter::GROUPS as usize;
const _: () = assert!(FIRST_GROUP + VARIANTS.len() <= GROUPS, "a draw group each variant");

/// A cell holds a cloud or none: 2^12 units, about 8 km at mid latitudes.
const CELL_SHAMT: u32 = 12;
const CELL: f64 = (1u32 << CELL_SHAMT) as f64;
/// The cells around the world, so a cell's hash wraps with the map.
const CELLS_AROUND: i64 = 1 << (pxl_spec::space::coord::world::EXTENT[0] - CELL_SHAMT);
/// A block is built and drawn as one, this many cells a side.
const BLOCK_CELLS: u32 = 16;
const BLOCK: f64 = CELL * BLOCK_CELLS as f64;
/// A cloud's width in units, at the least and the most: about 2 to 3.5 km,
/// whatever the spacing.
const WIDTH: [f64; 2] = [1024.0, 1840.0];
/// The farthest a block is built: where the widest cloud is a sprite's worth.
const REACH_MAX: f64 = sprite_reach(WIDTH[1]);
/// The most blocks the reach spans, the keys of a side squared: its share of
/// the renderer's block table.
pub const BLOCKS_MAX: usize = {
    let side = (2.0 * REACH_MAX / BLOCK) as usize + 2;
    side * side
};
/// The share of cells holding a cloud, the same everywhere: tied to the
/// field, the cumulus gathered in patches with bare country between.
const SHARE: f64 = 0.5;
const SALT: u32 = 0x0c0c;

struct Block {
    at: u32,
    bytes: u32,
    handle: u32,
    index: u32,
    heights: [f32; 2],
}

#[derive(Default)]
struct Clouds {
    models: [Option<u32>; VARIANTS.len()],
    blocks: FnvHashMap<[i64; 2], Block>,
    /// Kept so a settled turn asks the heap for nothing.
    wanted: Vec<[i64; 2]>,
    gone: Vec<[i64; 2]>,
    lanes: [Vec<[f32; 5]>; VARIANTS.len()],
    sprites: Vec<u32>,
    /// How far from the eye the nearest block drawn last turn stands; none
    /// drawn, infinite.
    seen: f64,
}

static CLOUDS: Global<Clouds> = Global::new();

fn clouds() -> &'static mut Clouds {
    CLOUDS.get(Clouds::default)
}

/// Each variant's draw group: its mesh, 0 while it loads, its skin and its extent.
pub fn cloud_groups() -> impl Iterator<Item = (usize, u32, Option<pxl_render::gpu::api::Image>, [f32; 3])> {
    let models = crate::domain::query::model::models();
    let skin = load_image(Load::Asset(cumulus::skin));
    clouds().models.iter().zip(VARIANTS).enumerate().map(move |(k, (m, asset))| {
        let mesh = m.and_then(|m| models.mesh(m)).map_or(0, |m| m.handle);
        (FIRST_GROUP + k, mesh, skin, extent(asset).map(|e| e as f32))
    })
}

fn extent(asset: Asset) -> [f64; 3] {
    asset.ingested().extent.unwrap_or([0.0; 3])
}

fn widest(asset: Asset) -> f64 {
    extent(asset).into_iter().fold(0.0, f64::max)
}

/// The layer's height over the datum, in units, as the sky draws it.
fn layer_u() -> f64 {
    CUMULUS_KM / km_per_u()
}

/// How long the wind takes to carry the clouds a pixel at the nearest drawn,
/// in seconds (`units_per_px_at`): what drifts less is not seen to move. A
/// camera cell's step, a dozen pixels at once, was seen as the clouds
/// stopping and jumping. Infinite while none is drawn.
pub fn drift_step_s() -> f64 {
    units_per_px_at(clouds().seen) / libm::hypot(WIND[0], WIND[1])
}

/// Where the clouds are gone, faded from half of it: a product number.
const GONE_KM: f64 = 64.0;

/// How much of the clouds shows, the cumulus and the sheets alike (the stage's
/// `CLOUDS`): none where the setting is off, all of them up to half of
/// `GONE_KM` and none from it.
pub fn clouds_shown() -> f64 {
    if !settings_store().clouds {
        return 0.0;
    }
    let altitude_km = f64::from(scene().environment.out[ENV_AIR]);
    let t = ((altitude_km - GONE_KM / 2.0) / (GONE_KM / 2.0)).clamp(0.0, 1.0);
    1.0 - t * t * (3.0 - 2.0 * t)
}

/// Takes the models as they arrive, keeps the blocks the eye can see a cloud
/// of, and lists them.
pub fn step(draw: &mut DrawList, frame: &Frame, budget: &FrameBudget) {
    let c = clouds();
    c.seen = f64::INFINITY;
    for (slot, asset) in c.models.iter_mut().zip(VARIANTS) {
        if slot.is_none() {
            *slot = model_of(asset, None).map(|h| h.model);
        }
    }
    let Some(eye) = eye() else { return };
    let env = &scene().environment.out;
    // None, nothing is built or drawn, and what is held waits.
    let opacity = clouds_shown() as f32;
    if opacity <= 0.0 {
        return;
    }
    let moved = drift(f64::from(env[ENV_SKY + 1]));
    let base = layer_u();
    let largest = WIDTH[1];
    // As far as the eye observes, a flat layer hiding under no horizon, and no
    // farther than its largest cloud is a sprite's worth.
    let reach = crate::domain::view::observation::eye_reach().unwrap_or(0.0).min(REACH_MAX);
    let on_map = [eye[0] + moved[0], eye[1] + moved[1]];
    let span = |k: usize| libm::floor((on_map[k] - reach) / BLOCK) as i64..=libm::floor((on_map[k] + reach) / BLOCK) as i64;
    let near = |key: [i64; 2]| {
        let corner = [key[0] as f64 * BLOCK - moved[0], key[1] as f64 * BLOCK - moved[1]];
        let gap = |lo: f64, v: f64| (lo - v).max(v - lo - BLOCK).max(0.0);
        let x0 = nearest(corner[0], eye[0]);
        let dz = (base - eye[2]).max(eye[2] - base - largest).max(0.0);
        let [dx, dy] = [gap(x0, eye[0]), gap(corner[1], eye[1])];
        libm::sqrt(dx * dx + dy * dy + dz * dz)
    };
    c.wanted.clear();
    for y in span(1) {
        for x in span(0) {
            let d = near([x, y]).max(1e-3);
            if d <= reach && drawn::tier((largest / units_per_px_at(d)) as f32, decor_gone_px() as f32, decor_sprite_px() as f32) != drawn::GONE {
                c.wanted.push([x, y]);
            }
        }
    }
    c.wanted.sort_unstable();
    c.gone.clear();
    c.gone.extend(c.blocks.keys().copied().filter(|k| c.wanted.binary_search(k).is_err()));
    for k in c.gone.drain(..) {
        if let Some(b) = c.blocks.remove(&k) {
            draw.free(b.handle);
            // SAFETY: the block's, not given back yet; nothing reads it past this turn.
            unsafe { free((b.at as usize, b.bytes)) };
        }
    }
    let (blocks, lanes) = (&mut c.blocks, &mut c.lanes);
    c.wanted.retain(|k| !blocks.contains_key(k));
    budget.run(
        c.wanted.iter().copied(),
        |key| match build(key, base, lanes, draw) {
            Some(b) => {
                blocks.insert(key, b);
                true
            }
            None => false,
        },
        None,
    );
    let frame_eye = frame.eye;
    for (&key, b) in &c.blocks {
        let corner = [key[0] as f64 * BLOCK - moved[0], key[1] as f64 * BLOCK - moved[1]];
        let [zlo, zhi] = b.heights.map(f64::from);
        let reach = largest / 2.0;
        if !frame.sees_box([corner[0] - reach, corner[1] - reach, zlo - reach], [corner[0] + BLOCK + reach, corner[1] + BLOCK + reach, zhi + reach]) {
            continue;
        }
        // The nearest cloud the view holds, each as the box it stands in, no
        // nearer than the near plane, short of which nothing is drawn: from
        // inside a box the drift is still a cell's at that distance.
        for (local, half) in built(b) {
            let at = [corner[0] + local[0], corner[1] + local[1], local[2]];
            if frame.sees_box(core::array::from_fn(|k| at[k] - half[k]), core::array::from_fn(|k| at[k] + half[k])) {
                let gap: V3 = core::array::from_fn(|k| ((if k == 0 { nearest(at[0], eye[0]) } else { at[k] }) - eye[k]).abs() - half[k]);
                c.seen = c.seen.min(v3::len(&gap.map(|g| g.max(0.0))).max(crate::domain::view::param::NEAR));
            }
        }
        let block = Shown {
            handle: b.handle,
            at: b.at,
            index: b.index,
            bytes: b.bytes,
            corner: [nearest(corner[0], eye[0]), corner[1]],
            side: BLOCK,
            heights: b.heights,
        };
        let translation = DrawList::translation([nearest(corner[0], frame_eye[0]), corner[1], 0.0], frame_eye);
        let groups = VARIANTS.iter().enumerate().map(|(k, &a)| ((FIRST_GROUP + k) as u32, (widest(a), WIDTH.map(|w| w / widest(a)))));
        present_block(draw, &block, translation, eye, groups, opacity, &mut c.sprites);
    }
}

/// A cloud of a block, about the block's corner on the field's map.
struct Cloud {
    variant: usize,
    local: [f64; 2],
    /// Its middle's height and its width, in units.
    z: f64,
    width: f64,
    spin: f64,
    scale: f64,
}

/// The block's clouds: a cell a cloud where the field's hash says one is.
fn clouds_of(key: [i64; 2], base: f64) -> impl Iterator<Item = Cloud> {
    (0..BLOCK_CELLS).flat_map(move |j| (0..BLOCK_CELLS).map(move |i| (i, j))).filter_map(move |(i, j)| {
        let cell = [key[0] * i64::from(BLOCK_CELLS) + i64::from(i), key[1] * i64::from(BLOCK_CELLS) + i64::from(j)];
        let at = cell.map(|v| (v & (CELLS_AROUND - 1)) as u32);
        let hash = |salt: u32| f64::from(lattice_hash(at, salt));
        if hash(SALT) >= SHARE {
            return None;
        }
        let jitter = [hash(SALT + 1), hash(SALT + 2)];
        let local = [(f64::from(i) + 0.25 + 0.5 * jitter[0]) * CELL, (f64::from(j) + 0.25 + 0.5 * jitter[1]) * CELL];
        let variant = ((hash(SALT + 4) * VARIANTS.len() as f64) as usize).min(VARIANTS.len() - 1);
        let asset = VARIANTS[variant];
        let width = WIDTH[0] + (WIDTH[1] - WIDTH[0]) * hash(SALT + 3);
        let scale = width / widest(asset);
        let height = extent(asset)[2] * scale;
        Some(Cloud {
            variant,
            local,
            z: base + 0.5 * height,
            width,
            spin: core::f64::consts::TAU * hash(SALT + 5),
            scale,
        })
    })
}

/// The block's clouds as `build` laid them, read off its lanes: each one's
/// middle about the block's corner, its height over the datum, and its half
/// extent. The field's hash is asked once a block, where it is built.
fn built(b: &Block) -> impl Iterator<Item = ([f64; 3], [f64; 3])> + '_ {
    // SAFETY: the block's bytes, held until it goes, which is after this turn reads them.
    let words: &[u32] = unsafe { core::slice::from_raw_parts(ptr_at::<u32>(b.at as usize), b.bytes as usize / 4) };
    lane::groups(words).zip(VARIANTS).flat_map(move |((first, n), asset)| {
        let lane = GroupLanes {
            first: first as usize,
            count: n as usize,
        };
        (0..n as usize).map(move |i| {
            let at = |k: usize| f64::from(f32::from_bits(words[lane.f32_at(k, i)]));
            let scale = at(4);
            let side = widest(asset) * scale / 2.0;
            ([at(0), at(1), at(2)], [side, side, extent(asset)[2] * scale / 2.0])
        })
    })
}

/// The block's clouds on the field's map, as decor's lanes about its corner;
/// none where the heap refused.
fn build(key: [i64; 2], base: f64, lanes: &mut [Vec<[f32; 5]>; VARIANTS.len()], draw: &mut DrawList) -> Option<Block> {
    for l in lanes.iter_mut() {
        l.clear();
    }
    let mut heights = [f32::INFINITY, f32::NEG_INFINITY];
    for c in clouds_of(key, base) {
        heights = [heights[0].min((c.z - c.width / 2.0) as f32), heights[1].max((c.z + c.width / 2.0) as f32)];
        lanes[c.variant].push([c.local[0] as f32, c.local[1] as f32, c.z as f32, c.spin as f32, c.scale as f32]);
    }
    let instances: usize = lanes.iter().map(Vec::len).sum();
    let header = 1 + lanes.len();
    let words = header + lanes.iter().map(|l| GroupLanes::words(l.len())).sum::<usize>();
    let index = (words * 4) as u32;
    let (at, bytes) = alloc(index + instances as u32 * 4)?;
    // SAFETY: a block of the bytes asked for, this block's until given back.
    let out: &mut [u32] = unsafe { core::slice::from_raw_parts_mut(ptr_at::<u32>(at), bytes as usize / 4) };
    out[0] = lanes.len() as u32;
    let mut first = header;
    for (g, l) in lanes.iter().enumerate() {
        out[1 + g] = l.len() as u32;
        let lane = GroupLanes { first, count: l.len() };
        for (i, v) in l.iter().enumerate() {
            for (k, &w) in v.iter().enumerate() {
                out[lane.f32_at(k, i)] = w.to_bits();
            }
        }
        out[lane.flags()..lane.flags() + l.len().div_ceil(4)].fill(0);
        first += GroupLanes::words(l.len());
    }
    let handle = draw.upload(at as u32, bytes, 0, 0);
    Some(Block {
        at: at as u32,
        bytes,
        handle,
        index,
        heights: if instances == 0 { [0.0; 2] } else { heights },
    })
}

/// A block of the clouds' lanes as `present_block` reads it: its square's corner
/// about the eye's lap, its side, and the heights its instances stand between.
struct Shown {
    handle: u32,
    at: u32,
    index: u32,
    bytes: u32,
    corner: [f64; 2],
    side: f64,
    heights: [f32; 2],
}

/// Lists one block's instances, model, sprite or nothing each, under the
/// draw groups `groups` names in the block's order, each with its model's
/// extent and the least and most scale its instances are drawn at. Under an
/// `opacity` of one the models are laid over the world blended, and a sprite,
/// which cannot be, is left out.
#[allow(clippy::too_many_arguments)]
fn present_block(draw: &mut DrawList, b: &Shown, translation: [f32; 3], eye: V3, groups: impl Iterator<Item = (u32, (f64, [f64; 2]))>, opacity: f32, sprites: &mut Vec<u32>) {
    let (model_px, sprite_px) = (decor_gone_px() as f32, decor_sprite_px() as f32);
    let tier = |px: f64| drawn::tier(px as f32, model_px, sprite_px);
    let [x0, y0] = b.corner;
    let [zlo, zhi] = b.heights.map(f64::from);
    let gap = |lo: f64, hi: f64, v: f64| (lo - v).max(v - hi).max(0.0);
    let reach = |lo: f64, hi: f64, v: f64| (v - lo).abs().max((hi - v).abs());
    let nearest = v3::len(&[gap(x0, x0 + b.side, eye[0]), gap(y0, y0 + b.side, eye[1]), gap(zlo, zhi, eye[2])]).max(1e-3);
    let farthest = v3::len(&[reach(x0, x0 + b.side, eye[0]), reach(y0, y0 + b.side, eye[1]), reach(zlo, zhi, eye[2])]).max(1e-3);
    // SAFETY: the block, `bytes` long, its owner's to write past `index`.
    let words: &mut [u32] = unsafe { core::slice::from_raw_parts_mut(ptr_at::<u32>(b.at as usize), b.bytes as usize / 4) };
    let (lanes_of, list) = words.split_at_mut(b.index as usize / 4);
    let (mut at, mut changed) = (0usize, false);
    for ((offset, count), (group, (extent, [least, most]))) in lane::groups(lanes_of).zip(groups) {
        let lane = GroupLanes {
            first: offset as usize,
            count: count as usize,
        };
        let first = at;
        sprites.clear();
        let gone = tier(extent * most / units_per_px_at(nearest)) == drawn::GONE;
        let modeled = tier(extent * least / units_per_px_at(farthest)) == drawn::MODEL;
        for i in 0..if gone { 0 } else { count as usize } {
            if (lanes_of[lane.flags()..][i / 4] >> ((i % 4) * 8)) & 1 != 0 {
                continue;
            }
            if modeled {
                changed |= list[at] != i as u32;
                list[at] = i as u32;
                at += 1;
                continue;
            }
            let f = |l: usize| f32::from_bits(lanes_of[lane.f32_at(l, i)]) as f64;
            let d = v3::len(&[x0 + f(0) - eye[0], y0 + f(1) - eye[1], f(2) - eye[2]]).max(1e-3);
            let t = tier(extent * f(4) / units_per_px_at(d));
            if t == drawn::MODEL {
                changed |= list[at] != i as u32;
                list[at] = i as u32;
                at += 1;
            } else if t == drawn::SPRITE && opacity >= 1.0 {
                sprites.push(i as u32);
            }
        }
        let models = (at - first) as u32;
        changed |= list[at..at + sprites.len()] != sprites[..];
        list[at..at + sprites.len()].copy_from_slice(sprites);
        at += sprites.len();
        let index = b.index / 4 + first as u32;
        if models > 0 {
            let pipeline = if opacity < 1.0 { PIPELINE_GHOST_DECOR } else { PIPELINE_DECOR };
            draw.draw_instances(pipeline, b.handle, group, models, [offset, count, index], translation, opacity);
        }
        if !sprites.is_empty() {
            draw.draw_instances(PIPELINE_IMPOSTOR, b.handle, group, sprites.len() as u32, [offset, count, index + models], translation, 1.0);
        }
    }
    if changed {
        draw.rewrite_part(b.handle, b.at, b.index, at as u32 * 4);
    }
}
