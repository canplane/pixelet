// pxl-kernel/domain/scene/environment/cloud.rs

//! The cumulus, the sky kind's (`spec/spec/asset/sky.json` `cumulus`),
//! drawn as decor is and held as decor is not. It claims no cell, so
//! nothing is built around it and a tower stands through it; it is no layer
//! and no clip level's brick holds it. It is laid on the field's own map,
//! which the wind carries (`pxl-content cloud.rs` `drift`): a patch of
//! cells there is built once and drawn moved, each instance a model, an
//! impostor or nothing by its size on the screen (`present_patch`), by the
//! tiers the GPU draws decor by, one rule (pxl-content scatter.rs
//! `drawn::tier`). A patch is laid in the columns the decor and impostor
//! shaders read (domain/scene/column.rs).

use fnv::FnvHashMap;

use pxl_content::cloud::{cloud_draw, holds_cloud};
use pxl_content::generation::drawn;
use pxl_content::param::cloud::{CUMULUS_KM, WIND};
use pxl_render::frame::draw::{
    DrawId, DrawList, Position, Stand, PIPELINE_DECOR, PIPELINE_GHOST_DECOR, PIPELINE_IMPOSTOR,
};
use pxl_render::frame::shader::word::DECOR_GROUPS;
use pxl_spec::asset::catalog::Slot;
use pxl_spec::asset::sky::cumulus;
use pxl_spec::space::coord::v3::{self, V3};
use pxl_spec::space::coord::world::nearest;
use xpute_core::global::Global;
use xpute_core::mem;
use xpute_core::mem::heap::{alloc, free};
use xpute_core::sched::pass::Pass;

use crate::domain::scene::column::{self, GroupColumns};
use crate::domain::scene::decor::gpu::{cull_dist_u, decor_impostor_angle, decor_model_angle};
use crate::domain::scene::environment::param::{
    CLOUD_GONE_KM, CLOUD_SALT, CLOUD_SHARE, CLOUD_SPACING_CELLS, CLOUD_WIDTH,
};
use crate::domain::scene::environment::{environment_alt_km, environment_drift};
use crate::domain::scene::frame::Frame;
use crate::domain::scene::model;
use crate::domain::scene::model::asset::model_of;
use crate::domain::view::frame::pose::{eye, km_per_u};
use crate::domain::view::lens;
use crate::domain::view::observation;
use crate::domain::view::param;
use crate::domain::view::setting::store::settings_store;
use crate::sys::io::loader::{catalog, load_image, load_of};
use pxl_content::generation;
use pxl_content::param::cloud;
use pxl_render::gpu::api::Image;
use pxl_spec::space::camera::grid;

/// Its models, a cloud each one of them.
const VARIANTS: [Slot; cumulus::models.len()] = cumulus::models;

pub const MODELS_MAX: usize = VARIANTS.len();

/// Its draw groups follow the decors' variants, which number the groups
/// before it (pxl-content scatter.rs `GROUPS`).
const FIRST_GROUP: usize = generation::GROUPS as usize;
const _: () = assert!(
    FIRST_GROUP + VARIANTS.len() <= DECOR_GROUPS,
    "a draw group each variant"
);

/// A cell holds a cloud or none: a unit of the field's lattice, whose hash
/// says where one is (pxl-content param.rs `cloud::LATTICE_SHAMT`).
const CELL_SHAMT: u32 = cloud::LATTICE_SHAMT;
const CELL: f64 = (1u32 << CELL_SHAMT) as f64;
/// A patch is built and drawn as one, this many cells a side.
const PATCH_CELLS: u32 = 16;
const PATCH: f64 = CELL * PATCH_CELLS as f64;
/// The farthest a patch is built: where the widest cloud is an impostor's worth.
const DRAW_DIST_MAX: f64 = cull_dist_u(CLOUD_WIDTH[1]);
/// The most patches the farthest spans, the keys of a side squared: its share of
/// the renderer's upload table.
pub const PATCHES_MAX: usize = {
    let side = (2.0 * DRAW_DIST_MAX / PATCH) as usize + 2;
    side * side
};

/// How far from the eye the thinning starts, in units: where a cell spans
/// `CLOUD_SPACING_CELLS` of the view's camera cells.
const THIN_FROM: f64 = CELL / grid::CELL / CLOUD_SPACING_CELLS;

/// Whether the cloud whose middle lies at `local` about its patch's corner
/// shows `d2` square units from the eye: its draw under (THIN_FROM / d)²,
/// asked as a product, so a cloud costs a multiply; the draw keyed by its
/// middle's bits, by the seed its place and look are drawn by
/// (`cloud_draw`). Decided as it is drawn, never built.
fn shows(local: [f32; 2], d2: f64) -> bool {
    cloud_draw(local.map(|v| v.to_bits() as i32), CLOUD_SALT + 6) * d2 < THIN_FROM * THIN_FROM
}

struct Patch {
    at: u32,
    bytes: u32,
    handle: u32,
    idx: u32,
    heights: [f32; 2],
    /// Each variant's draws, its models' and its impostors'.
    draws: [[DrawId; 2]; VARIANTS.len()],
}

#[derive(Default)]
struct Clouds {
    models: [Option<u32>; VARIANTS.len()],
    patches: FnvHashMap<[i64; 2], Patch>,
    /// Kept so a settled turn asks the heap for nothing.
    wanted: Vec<[i64; 2]>,
    gone: Vec<[i64; 2]>,
    columns: [Vec<[f32; 5]>; VARIANTS.len()],
    impostors: Vec<u32>,
    /// How far from the eye the nearest patch drawn last turn stands; none
    /// drawn, infinite.
    seen: f64,
}

static CLOUDS: Global<Clouds> = Global::new();

fn clouds() -> &'static mut Clouds {
    CLOUDS.get(Clouds::default)
}

/// Each variant's draw group: its mesh, 0 while it loads, its skin and its extent.
pub fn cloud_groups() -> impl Iterator<Item = (usize, u32, Option<Image>, [f32; 3])> {
    let models = model::models();
    let skin = load_of(cumulus::skin).and_then(load_image);
    clouds()
        .models
        .iter()
        .zip(VARIANTS)
        .enumerate()
        .map(move |(k, (m, file))| {
            let mesh = m.and_then(|m| models.mesh(m)).map_or(0, |m| m.handle);
            (FIRST_GROUP + k, mesh, skin, extent(file).map(|e| e as f32))
        })
}

/// Its model's, as the catalog measures it; none before one is read.
fn extent(slot: Slot) -> [f64; 3] {
    catalog().map_or([0.0; 3], |c| c.extent(slot))
}

fn widest(slot: Slot) -> f64 {
    extent(slot).into_iter().fold(0.0, f64::max)
}

/// The layer's height over the datum, in units, as the sky draws it.
fn layer_u() -> f64 {
    CUMULUS_KM / km_per_u()
}

/// How long the wind takes to carry the clouds the seen step at the nearest
/// drawn (`lens::seen_step_s`), in seconds: so a drawn cloud lags the wind
/// by up to that, an angle, the same on every screen. Infinite while none
/// is drawn.
pub fn drift_step_s() -> f64 {
    lens::seen_step_s(clouds().seen, libm::hypot(WIND[0], WIND[1]))
}

/// How much of the clouds shows, the cumulus and the sheets alike
/// (`FrameInput::clouds`), the eye as high as the sky last saw it: none where
/// the setting is off, all of them up to half of `CLOUD_GONE_KM` and none
/// from it.
pub fn clouds_shown() -> f64 {
    if !settings_store().clouds {
        return 0.0;
    }
    let t = ((environment_alt_km() - CLOUD_GONE_KM / 2.0) / (CLOUD_GONE_KM / 2.0)).clamp(0.0, 1.0);
    1.0 - t * t * (3.0 - 2.0 * t)
}

/// Takes the models as they arrive, keeps the patches the eye can see a cloud
/// of, and lists them, carried as far as the wind has (`environment_drift`).
pub fn step(draw: &mut DrawList, frame: &Frame, pass: &Pass) {
    let c = clouds();
    c.seen = f64::INFINITY;
    for (slot, file) in c.models.iter_mut().zip(VARIANTS) {
        if slot.is_none() {
            *slot = model_of(file, None);
        }
    }
    let Some(eye) = eye() else { return };
    // None, nothing is built or drawn, and what is held waits.
    let opacity = clouds_shown() as f32;
    if opacity <= 0.0 {
        return;
    }
    let off = environment_drift();
    let base = layer_u();
    let largest = CLOUD_WIDTH[1];
    // As far as the eye draws, a flat layer hiding under no horizon, and no
    // farther than its largest cloud is an impostor's worth.
    let dist = observation::eye_draw_dist()
        .unwrap_or(0.0)
        .min(DRAW_DIST_MAX);
    let on_map = [eye[0] + off[0], eye[1] + off[1]];
    let span = |k: usize| {
        libm::floor((on_map[k] - dist) / PATCH) as i64
            ..=libm::floor((on_map[k] + dist) / PATCH) as i64
    };
    let near = |key: [i64; 2]| {
        let corner = [
            key[0] as f64 * PATCH - off[0],
            key[1] as f64 * PATCH - off[1],
        ];
        let gap = |lo: f64, v: f64| (lo - v).max(v - lo - PATCH).max(0.0);
        let x0 = nearest(corner[0], eye[0]);
        let dz = (base - eye[2]).max(eye[2] - base - largest).max(0.0);
        let [dx, dy] = [gap(x0, eye[0]), gap(corner[1], eye[1])];
        libm::sqrt(dx * dx + dy * dy + dz * dz)
    };
    c.wanted.clear();
    for y in span(1) {
        for x in span(0) {
            let d = near([x, y]).max(1e-3);
            if d <= dist
                && drawn::tier(
                    (largest / d) as f32,
                    decor_model_angle() as f32,
                    decor_impostor_angle() as f32,
                ) != drawn::CULLED
            {
                c.wanted.push([x, y]);
            }
        }
    }
    c.wanted.sort_unstable();
    c.gone.clear();
    c.gone.extend(
        c.patches
            .keys()
            .copied()
            .filter(|k| c.wanted.binary_search(k).is_err()),
    );
    for k in c.gone.drain(..) {
        if let Some(b) = c.patches.remove(&k) {
            draw.free(b.handle);
            // SAFETY: the patch's, not given back yet; nothing reads it past this turn.
            unsafe { free((b.at as usize, b.bytes)) };
        }
    }
    let (patches, columns) = (&mut c.patches, &mut c.columns);
    c.wanted.retain(|k| !patches.contains_key(k));
    pass.run(
        c.wanted.iter().copied(),
        |key| match build(key, base, columns, draw) {
            Some(b) => {
                patches.insert(key, b);
                true
            }
            None => false,
        },
        None,
    );
    for (&key, b) in &c.patches {
        let corner = [
            key[0] as f64 * PATCH - off[0],
            key[1] as f64 * PATCH - off[1],
        ];
        let [zlo, zhi] = b.heights.map(f64::from);
        let half = largest / 2.0;
        if !frame.sees_box(
            [corner[0] - half, corner[1] - half, zlo - half],
            [
                corner[0] + PATCH + half,
                corner[1] + PATCH + half,
                zhi + half,
            ],
        ) {
            continue;
        }
        // The nearest cloud the view holds, each as the box it stands in, no
        // nearer than the near plane, short of which nothing is drawn: from
        // inside a box the drift is still a cell's at that distance.
        for (local, half) in patch_clouds(b) {
            let at = [corner[0] + local[0], corner[1] + local[1], local[2]];
            if frame.sees_box(
                core::array::from_fn(|k| at[k] - half[k]),
                core::array::from_fn(|k| at[k] + half[k]),
            ) {
                let gap: V3 = core::array::from_fn(|k| {
                    ((if k == 0 {
                        nearest(at[0], eye[0])
                    } else {
                        at[k]
                    }) - eye[k])
                        .abs()
                        - half[k]
                });
                c.seen = c
                    .seen
                    .min(v3::len(&gap.map(|g| g.max(0.0))).max(param::NEAR));
            }
        }
        let patch = PatchDraw {
            handle: b.handle,
            at: b.at,
            idx: b.idx,
            bytes: b.bytes,
            corner: [nearest(corner[0], eye[0]), corner[1]],
            stood: stood_corner(key, off, eye),
            side: PATCH,
            heights: b.heights,
            draws: &b.draws,
        };
        // The patch's corner on the field's map, on Q3, which the renderer
        // moves back by the wind's drift: its draw holds no eye and no drift.
        let pos = patch_place(key);
        let groups = VARIANTS.iter().enumerate().map(|(k, &a)| {
            (
                (FIRST_GROUP + k) as u32,
                (widest(a), CLOUD_WIDTH.map(|w| w / widest(a))),
            )
        });
        present_patch(draw, &patch, pos, eye, groups, opacity, &mut c.impostors);
    }
}

/// The patch `key`'s corner on the field's map, on Q3, as its draws hold it:
/// the renderer moves it back by the wind's drift (pxl-render frame/draw.rs
/// `DrawList::draw_instances`).
fn patch_place(key: [i64; 2]) -> Position {
    Position::of([key[0] as f64 * PATCH, key[1] as f64 * PATCH, 0.0])
}

/// The patch `key`'s corner its draws stand by, on the lap of X nearest
/// `eye`: where the drift `off` held to a whole cell, the lattice's unit,
/// leaves it. The corner as drawn lies within a cell short of it, so a stand
/// grown a cell back from it (`present_patch`) holds the patch drawn at every
/// drift until the drift crosses into the next cell, and stands still
/// meanwhile, as long as the wind takes to carry the field a cell.
fn stood_corner(key: [i64; 2], off: [f64; 2], eye: V3) -> [f64; 2] {
    let held = off.map(|v| libm::floor(v / CELL) * CELL);
    let x = key[0] as f64 * PATCH - held[0];
    [nearest(x, eye[0]), key[1] as f64 * PATCH - held[1]]
}

/// The box a patch's draws stand within, its corner `stood`
/// (`stood_corner`), its side `side`, its instances between the heights
/// `[zlo, zhi]`, its models reaching `e` past them: grown a cell back from
/// the corner, so it holds the patch at every drift the corner holds.
fn patch_stand(stood: [f64; 2], side: f64, e: f64, [zlo, zhi]: [f64; 2]) -> Stand {
    let [x, y] = stood;
    Stand::of(
        [x - CELL - e, y - CELL - e, zlo - e],
        [x + side + e, y + side + e, zhi + e],
    )
}

/// A cloud of a patch, about the patch's corner on the field's map.
struct Cloud {
    variant: usize,
    local: [f64; 2],
    /// Its middle's height and its width, in units.
    z: f64,
    width: f64,
    spin: f64,
    scale: f64,
}

/// The patch's clouds: a cell a cloud where the field's hash says one is.
fn clouds_of(key: [i64; 2], base: f64) -> impl Iterator<Item = Cloud> {
    (0..PATCH_CELLS)
        .flat_map(move |j| (0..PATCH_CELLS).map(move |i| (i, j)))
        .filter_map(move |(i, j)| {
            let cell = [
                key[0] * i64::from(PATCH_CELLS) + i64::from(i),
                key[1] * i64::from(PATCH_CELLS) + i64::from(j),
            ];
            let at = cell.map(|v| v as i32);
            let hash = |salt: u32| cloud_draw(at, salt);
            // Its patch's count handed down to it (pxl-content cloud.rs
            // `holds_cloud`), as the decor's and the caves' are.
            let mean = (CLOUD_SHARE * f64::from(PATCH_CELLS * PATCH_CELLS)) as u32;
            if !holds_cloud(at, mean) {
                return None;
            }
            let jitter = [hash(CLOUD_SALT + 1), hash(CLOUD_SALT + 2)];
            let local = [
                (f64::from(i) + 0.25 + 0.5 * jitter[0]) * CELL,
                (f64::from(j) + 0.25 + 0.5 * jitter[1]) * CELL,
            ];
            let variant =
                ((hash(CLOUD_SALT + 4) * VARIANTS.len() as f64) as usize).min(VARIANTS.len() - 1);
            let file = VARIANTS[variant];
            let width = CLOUD_WIDTH[0] + (CLOUD_WIDTH[1] - CLOUD_WIDTH[0]) * hash(CLOUD_SALT + 3);
            let scale = width / widest(file);
            let height = extent(file)[2] * scale;
            Some(Cloud {
                variant,
                local,
                z: base + 0.5 * height,
                width,
                spin: core::f64::consts::TAU * hash(CLOUD_SALT + 5),
                scale,
            })
        })
}

/// The patch's clouds as `build` laid them, read off its columns: each one's
/// middle about the patch's corner, its height over the datum, and its half
/// extent. The field's hash is asked once a patch, where it is built, and
/// the catalog once a variant.
fn patch_clouds(b: &Patch) -> impl Iterator<Item = ([f64; 3], [f64; 3])> + '_ {
    // SAFETY: the patch's bytes, held until it goes, which is after this turn reads them.
    let words: &[u32] =
        unsafe { core::slice::from_raw_parts(mem::at::<u32>(b.at as usize), b.bytes as usize / 4) };
    column::groups(words)
        .zip(VARIANTS.map(|file| (widest(file), extent(file)[2])))
        .flat_map(move |((first, n), (wide, tall))| {
            let column = GroupColumns {
                first: first as usize,
                cnt: n as usize,
            };
            (0..n as usize).map(move |i| {
                let at = |k: usize| f64::from(f32::from_bits(words[column.f32_at(k, i)]));
                let scale = at(4);
                let side = wide * scale / 2.0;
                ([at(0), at(1), at(2)], [side, side, tall * scale / 2.0])
            })
        })
}

/// The patch's clouds on the field's map, as decor's columns about its corner;
/// none where the heap refused.
fn build(
    key: [i64; 2],
    base: f64,
    columns: &mut [Vec<[f32; 5]>; VARIANTS.len()],
    draw: &mut DrawList,
) -> Option<Patch> {
    for l in columns.iter_mut() {
        l.clear();
    }
    let mut heights = [f32::INFINITY, f32::NEG_INFINITY];
    for c in clouds_of(key, base) {
        heights = [
            heights[0].min((c.z - c.width / 2.0) as f32),
            heights[1].max((c.z + c.width / 2.0) as f32),
        ];
        columns[c.variant].push([
            c.local[0] as f32,
            c.local[1] as f32,
            c.z as f32,
            c.spin as f32,
            c.scale as f32,
        ]);
    }
    let instances: usize = columns.iter().map(Vec::len).sum();
    let header = 1 + columns.len();
    let words = header
        + columns
            .iter()
            .map(|l| GroupColumns::words(l.len()))
            .sum::<usize>();
    let idx = (words * 4) as u32;
    let (at, bytes) = alloc(idx + instances as u32 * 4)?;
    // SAFETY: a heap block of the bytes asked for, this patch's until given back.
    let out: &mut [u32] =
        unsafe { core::slice::from_raw_parts_mut(mem::at::<u32>(at), bytes as usize / 4) };
    out[0] = columns.len() as u32;
    let mut first = header;
    for (g, l) in columns.iter().enumerate() {
        out[1 + g] = l.len() as u32;
        let column = GroupColumns {
            first,
            cnt: l.len(),
        };
        for (i, v) in l.iter().enumerate() {
            for (k, &w) in v.iter().enumerate() {
                out[column.f32_at(k, i)] = w.to_bits();
            }
        }
        out[column.flags()..column.flags() + l.len().div_ceil(4)].fill(0);
        first += GroupColumns::words(l.len());
    }
    let handle = draw.upload(at as u32, bytes, 0, 0);
    Some(Patch {
        at: at as u32,
        bytes,
        handle,
        idx,
        heights: if instances == 0 { [0.0; 2] } else { heights },
        draws: core::array::from_fn(|_| Default::default()),
    })
}

/// A patch of the clouds' columns as `present_patch` reads it: its square's corner
/// about the eye's lap, its side, and the heights its instances stand between.
struct PatchDraw<'a> {
    handle: u32,
    at: u32,
    idx: u32,
    bytes: u32,
    corner: [f64; 2],
    /// The corner its draws stand by (`stood_corner`).
    stood: [f64; 2],
    side: f64,
    heights: [f32; 2],
    draws: &'a [[DrawId; 2]],
}

/// Lists one patch's instances, model, impostor or nothing each, under the
/// draw groups `groups` names in the patch's order, each with its model's
/// extent and the least and most scale its instances are drawn at. Under an
/// `opacity` of one the models are laid over the world blended, and an impostor,
/// which cannot be, is left out.
fn present_patch(
    draw: &mut DrawList,
    b: &PatchDraw,
    pos: Position,
    eye: V3,
    groups: impl Iterator<Item = (u32, (f64, [f64; 2]))>,
    opacity: f32,
    impostors: &mut Vec<u32>,
) {
    let (model, impostor) = (decor_model_angle() as f32, decor_impostor_angle() as f32);
    let tier = |across: f64| drawn::tier(across as f32, model, impostor);
    let [x0, y0] = b.corner;
    let [zlo, zhi] = b.heights.map(f64::from);
    let gap = |lo: f64, hi: f64, v: f64| (lo - v).max(v - hi).max(0.0);
    let far = |lo: f64, hi: f64, v: f64| (v - lo).abs().max((hi - v).abs());
    let nearest = v3::len(&[
        gap(x0, x0 + b.side, eye[0]),
        gap(y0, y0 + b.side, eye[1]),
        gap(zlo, zhi, eye[2]),
    ])
    .max(1e-3);
    let farthest = v3::len(&[
        far(x0, x0 + b.side, eye[0]),
        far(y0, y0 + b.side, eye[1]),
        far(zlo, zhi, eye[2]),
    ])
    .max(1e-3);
    // SAFETY: the patch's bytes, `bytes` long, its owner's to write past `index`.
    let words: &mut [u32] = unsafe {
        core::slice::from_raw_parts_mut(mem::at::<u32>(b.at as usize), b.bytes as usize / 4)
    };
    let (columns_of, list) = words.split_at_mut(b.idx as usize / 4);
    // A patch all within THIN_FROM shows every cloud, and asks none.
    let thin = farthest > THIN_FROM;
    let (mut at, mut changed) = (0usize, false);
    let variants = column::groups(columns_of).zip(groups).zip(b.draws);
    for (((off, cnt), (group, (extent, [least, most]))), [model_id, impostor_id]) in variants {
        let column = GroupColumns {
            first: off as usize,
            cnt: cnt as usize,
        };
        let first = at;
        impostors.clear();
        let culled = tier(extent * most / nearest) == drawn::CULLED;
        let modeled = tier(extent * least / farthest) == drawn::MODEL;
        for i in 0..if culled { 0 } else { cnt as usize } {
            if (columns_of[column.flags()..][i / 4] >> ((i % 4) * 8)) & 1 != 0 {
                continue;
            }
            let f = |l: usize| f32::from_bits(columns_of[column.f32_at(l, i)]) as f64;
            let gap = [x0 + f(0) - eye[0], y0 + f(1) - eye[1], f(2) - eye[2]];
            let d2 = if thin || !modeled {
                v3::dot(&gap, &gap)
            } else {
                0.0
            };
            if thin && !shows([f(0) as f32, f(1) as f32], d2) {
                continue;
            }
            if modeled {
                changed |= list[at] != i as u32;
                list[at] = i as u32;
                at += 1;
                continue;
            }
            let d = libm::sqrt(d2).max(1e-3);
            let t = tier(extent * f(4) / d);
            if t == drawn::MODEL {
                changed |= list[at] != i as u32;
                list[at] = i as u32;
                at += 1;
            } else if t == drawn::IMPOSTOR && opacity >= 1.0 {
                impostors.push(i as u32);
            }
        }
        let models = (at - first) as u32;
        changed |= list[at..at + impostors.len()] != impostors[..];
        list[at..at + impostors.len()].copy_from_slice(impostors);
        at += impostors.len();
        let idx = b.idx / 4 + first as u32;
        // Its group's draws read the sun's map, so they stand within the patch's box grown by
        // as far as the group's models reach at their most scale, in the world, at every
        // drift the stood corner holds: a cell back from it (`stood_corner`).
        let stand = patch_stand(b.stood, b.side, extent * most, [zlo, zhi]);
        if models > 0 {
            let pipeline = if opacity < 1.0 {
                PIPELINE_GHOST_DECOR
            } else {
                PIPELINE_DECOR
            };
            draw.draw_instances(
                model_id,
                pipeline,
                b.handle,
                group,
                models,
                [off, cnt, idx],
                pos,
                opacity,
                true,
                Some(stand),
            );
        }
        if !impostors.is_empty() {
            draw.draw_instances(
                impostor_id,
                PIPELINE_IMPOSTOR,
                b.handle,
                group,
                impostors.len() as u32,
                [off, cnt, idx + models],
                pos,
                1.0,
                true,
                Some(stand),
            );
        }
    }
    if changed {
        draw.rewrite_part(b.handle, b.at, b.idx, at as u32 * 4);
    }
}

#[cfg(test)]
#[path = "cloud.test.rs"]
mod test;
