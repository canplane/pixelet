// pxl-kernel/scene/environment/cloud.rs

//! The cumulus, drawn as decor is and held as decor is not. It claims no cell,
//! so nothing is built around it and a tower stands through it; it is no
//! layer and no chunk holds it. It is laid on the field's own map, which the
//! wind carries (`pxl-render look/cloud.rs` `drift`): a block of cells there
//! is built once and drawn moved, each instance a model, a sprite or nothing
//! by its size on the screen (`chunk/decor/cell.rs` `present_block`).

use fnv::FnvHashMap;

use pxl_render::frame::draw::DrawList;
use pxl_render::gpu::pass::decor::GROUPS;
use pxl_render::look::cloud::{drift, lattice_hash, shown, CUMULUS_KM};
use pxl_render::look::environment::{ENV_AIR, ENV_SKY};
use pxl_spec::asset::ingested::Asset;
use pxl_spec::block::decor::DECORS;
use pxl_spec::space::coord::world::nearest;
use xpute_guest::global::Global;
use xpute_guest::mem::base::ptr_at;
use xpute_guest::mem::heap::{alloc, free};
use xpute_guest::sched::frame_budget::FrameBudget;

use crate::chunk::decor::build::decor_sprite_px;
use crate::chunk::decor::cell::{present_block, Shown};
use crate::chunk::decor::instance::GroupLanes;
use crate::chunk::table::Frame;
use crate::io::load::{load_failed, load_gltf, load_image, Load};
use crate::kernel;
use crate::model::asset::register_model;
use crate::setting::store::settings_store;
use crate::view::frame::pose::{globe, km_per_u, units_per_px_at};
use crate::view::observation::eye;

const VARIANTS: [Asset; 3] = [Asset::sky_cloud_cumulus_1_glb, Asset::sky_cloud_cumulus_2_glb, Asset::sky_cloud_stratus_glb];
const SKIN: Asset = Asset::sky_cloud_white_png;

pub const MODELS_MAX: usize = VARIANTS.len();

/// Its draw groups follow the decors' variants.
const FIRST_GROUP: usize = {
    let (mut n, mut k) = (0, 0);
    while k < DECORS.len() {
        n += DECORS[k].variants.len();
        k += 1;
    }
    n
};
const _: () = assert!(FIRST_GROUP + VARIANTS.len() <= GROUPS, "a draw group each variant");

/// A cell holds a cloud or none: 2^12 units, about 8 km at mid latitudes.
const CELL_SHAMT: u32 = 12;
const CELL: f64 = (1u32 << CELL_SHAMT) as f64;
/// The cells around the world, so a cell's hash wraps with the map.
const CELLS_AROUND: i64 = 1 << (pxl_spec::space::coord::pvf3::core::IVEC_X_W - CELL_SHAMT);
/// A block is built and drawn as one, this many cells a side.
const BLOCK_CELLS: u32 = 16;
const BLOCK: f64 = CELL * BLOCK_CELLS as f64;
/// A cloud's width in units, at the least and the most: about 2 to 3.5 km,
/// whatever the spacing.
const WIDTH: [f64; 2] = [1024.0, 1840.0];
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
}

static CLOUDS: Global<Clouds> = Global::new();

fn clouds() -> &'static mut Clouds {
    CLOUDS.get(Clouds::default)
}

/// Each variant's draw group: its mesh, 0 while it loads, its skin and its extent.
pub fn cloud_groups() -> impl Iterator<Item = (usize, u32, Option<u32>, [f32; 3])> {
    let models = &kernel().models;
    let skin = load_image(Load::Asset(SKIN));
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

/// How much of the clouds shows: none where the setting is off, and fading
/// as the eye climbs to where they are gone.
pub fn clouds_shown() -> f64 {
    if !settings_store().clouds {
        return 0.0;
    }
    shown(f64::from(kernel().scene.environment.out[ENV_AIR]))
}

/// Takes the models as they arrive, keeps the blocks the eye can see a cloud
/// of, and lists them.
pub fn step(draw: &mut DrawList, frame: &Frame, budget: &FrameBudget) {
    let c = clouds();
    for (slot, asset) in c.models.iter_mut().zip(VARIANTS) {
        if slot.is_none() && !load_failed(Load::Asset(asset)) {
            *slot = load_gltf(Load::Asset(asset)).and_then(|gltf| register_model(&gltf, None)).map(|h| h.model);
        }
    }
    let Some(eye) = eye() else { return };
    let env = &kernel().scene.environment.out;
    // None, nothing is built or drawn, and what is held waits.
    let opacity = clouds_shown() as f32;
    if opacity <= 0.0 {
        return;
    }
    let moved = drift(f64::from(env[ENV_SKY + 1]));
    let base = layer_u();
    let largest = WIDTH[1];
    // To the horizon over the layer, and no farther than its largest cloud is a sprite's worth.
    let r = globe().radius_u();
    let horizon = |h: f64| libm::sqrt(2.0 * r * h + h * h);
    let reach = (horizon(eye[2].max(0.0)) + horizon(base)).min(largest / units_per_px_at(1.0) / decor_sprite_px());
    let on_map = [eye[0] + moved[0], eye[1] + moved[1]];
    let span = |k: usize| libm::floor((on_map[k] - reach) / BLOCK) as i64..=libm::floor((on_map[k] + reach) / BLOCK) as i64;
    let near = |key: [i64; 2]| {
        let corner = [key[0] as f64 * BLOCK - moved[0], key[1] as f64 * BLOCK - moved[1]];
        let gap = |lo: f64, v: f64| (lo - v).max(v - lo - BLOCK).max(0.0);
        let x0 = nearest(corner[0], eye[0]);
        let dz = (base - eye[2]).max(eye[2] - base - largest).max(0.0);
        libm::sqrt(gap(x0, eye[0]).powi(2) + gap(corner[1], eye[1]).powi(2) + dz * dz)
    };
    c.wanted.clear();
    for y in span(1) {
        for x in span(0) {
            let d = near([x, y]).max(1e-3);
            if d <= reach && largest / units_per_px_at(d) >= decor_sprite_px() {
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
        let middle = [corner[0] + BLOCK / 2.0, corner[1] + BLOCK / 2.0, (zlo + zhi) / 2.0];
        let radius = libm::sqrt(2.0 * (BLOCK / 2.0 + largest / 2.0).powi(2) + ((zhi - zlo) / 2.0 + largest / 2.0).powi(2));
        if !frame.sees(middle, radius) {
            continue;
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

/// The block's clouds on the field's map, as decor's lanes about its corner;
/// none where the heap refused.
fn build(key: [i64; 2], base: f64, lanes: &mut [Vec<[f32; 5]>; VARIANTS.len()], draw: &mut DrawList) -> Option<Block> {
    for l in lanes.iter_mut() {
        l.clear();
    }
    let mut heights = [f32::INFINITY, f32::NEG_INFINITY];
    for j in 0..BLOCK_CELLS {
        for i in 0..BLOCK_CELLS {
            let cell = [key[0] * i64::from(BLOCK_CELLS) + i64::from(i), key[1] * i64::from(BLOCK_CELLS) + i64::from(j)];
            let at = cell.map(|v| (v & (CELLS_AROUND - 1)) as u32);
            let jitter = [lattice_hash(at, SALT + 1), lattice_hash(at, SALT + 2)];
            let local = [(f64::from(i) + 0.25 + 0.5 * jitter[0]) * CELL, (f64::from(j) + 0.25 + 0.5 * jitter[1]) * CELL];
            if lattice_hash(at, SALT) >= SHARE {
                continue;
            }
            let variant = ((lattice_hash(at, SALT + 4) * VARIANTS.len() as f64) as usize).min(VARIANTS.len() - 1);
            let asset = VARIANTS[variant];
            let width = WIDTH[0] + (WIDTH[1] - WIDTH[0]) * lattice_hash(at, SALT + 3);
            let scale = width / widest(asset);
            let z = base + 0.5 * extent(asset)[2] * scale;
            let spin = core::f64::consts::TAU * lattice_hash(at, SALT + 5);
            heights = [heights[0].min((z - width / 2.0) as f32), heights[1].max((z + width / 2.0) as f32)];
            lanes[variant].push([local[0] as f32, local[1] as f32, z as f32, spin as f32, scale as f32]);
        }
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
