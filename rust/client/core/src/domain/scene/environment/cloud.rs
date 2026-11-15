// pxl-core/domain/scene/environment/cloud.rs

//! The cumulus clouds (`spec/spec/asset/sky.json5` `cumulus`). They are drawn
//! the way decor is drawn, but not held the way decor is held. A cloud claims
//! no cell, so nothing is built around it and a tower stands through it. No
//! layer and no clip level's brick holds it. The clouds are laid on the cloud
//! field's own map, which the wind carries (pxl-content cloud.rs `drift`). A
//! patch of that map's cells is built once and then drawn moved by the
//! drift. Each cloud in it is drawn as a model, as an impostor or not at all
//! by its size on the screen (`present_patch`), under the rule the GPU
//! applies to decor (pxl-content generation.rs `drawing::lod`). A patch is laid
//! out in the columns the decor and impostor shaders read
//! (domain/scene/column.rs).

use fnv::FnvHashMap;

use pxl_content::cloud::{cloud_draw, holds_cloud};
use pxl_content::generation::drawing;
use pxl_content::param::cloud::{CUMULUS_KM, WIND};
use pxl_global::Global;
use pxl_render::frame::shader::record::{PIPELINE_DECOR, PIPELINE_GHOST_DECOR, PIPELINE_IMPOSTOR};
use pxl_render::frame::shader::word::DECOR_GROUP_CNT;
use pxl_render::gpu::draw::{DrawId, DrawList, Position, ReceiverBox};
use pxl_spec::asset::catalog::{Slot, model_extent};
use pxl_spec::asset::sky::cumulus;
use pxl_spec::space::coord::v3::{self, V3};
use pxl_spec::space::coord::world::nearest;
use xpute_link::mem;

use crate::domain::scene::column::{self, GroupRange};
use crate::domain::scene::environment::param::{
    CLOUD_GONE_KM, CLOUD_SALT, CLOUD_SHARE, CLOUD_SPACING_CELLS, CLOUD_WIDTH,
};
use crate::domain::scene::environment::{environment_alt_km, environment_drift};
use crate::domain::scene::frame::Frame;
use crate::domain::scene::model;
use crate::domain::scene::model::asset::model_of;
use crate::domain::setting::settings;
use crate::domain::view::frame::pose::{eye, km_per_u};
use crate::domain::view::lens::{self, cull_dist_u, decor_impostor_angle, decor_model_angle};
use crate::domain::view::observation;
use crate::domain::view::param;
use crate::sys::heap;
use crate::sys::io::loader::{catalog, load_image, load_of};
use crate::sys::sched;
use pxl_content::generation;
use pxl_content::param::cloud;
use pxl_render::gpu::api::Image;

/// The cumulus models. Each model is one whole cloud.
const VARIANTS: [Slot; cumulus::MODELS.len()] = cumulus::MODELS;

pub(crate) const MODEL_CAP: usize = VARIANTS.len();

/// The clouds' draw groups come right after those of the decor variants,
/// which take the numbers before them (pxl-content generation.rs `GROUP_CNT`).
const FIRST_GROUP: usize = generation::GROUP_CNT as usize;
const _: () = assert!(
    FIRST_GROUP + VARIANTS.len() <= DECOR_GROUP_CNT,
    "a draw group each variant"
);

/// A cell of the cloud field is a unit of the field's lattice and holds one
/// cloud or none. The field's hash says which cells hold one (pxl-content
/// param.rs `cloud::LATTICE_SHAMT`).
const CELL_SHAMT: u32 = cloud::LATTICE_SHAMT;
const CELL: f64 = (1u32 << CELL_SHAMT) as f64;
/// The side of a patch, in cells. A patch is built and drawn as one piece.
const PATCH_CELLS: u32 = 16;
const PATCH: f64 = CELL * PATCH_CELLS as f64;
/// The farthest a patch is built. Beyond it even the widest cloud is too
/// small for an impostor.
const DRAW_DIST_MAX: f64 = cull_dist_u(CLOUD_WIDTH[1]);
/// The most patches built at once: the number of patch keys along a side of
/// the reach, squared. It sets the clouds' share of the renderer's upload
/// table.
pub(crate) const PATCH_CAP: usize = {
    let side = (2.0 * DRAW_DIST_MAX / PATCH) as usize + 2;
    side * side
};

/// The distance from the eye, in units, at which the clouds begin to thin:
/// where a cell spans `CLOUD_SPACING_CELLS` camera grid cells.
const THIN_FROM: f64 = CELL / lens::CELL / CLOUD_SPACING_CELLS;

/// Whether the cloud whose middle lies at `local` about its patch's corner
/// shows at `d2` square units from the eye. It shows when its random draw is
/// under (THIN_FROM / d)². The test is written as a product, so it costs a
/// cloud one multiply. The draw is keyed by the bits of the cloud's middle,
/// through the seed that also draws its place and look (`cloud_draw`), so a
/// cloud that shows at some distance shows at every nearer one. It is decided
/// as the cloud is drawn, never when the patch is built.
fn shows(local: [f32; 2], d2: f64) -> bool {
    cloud_draw(local.map(|v| v.to_bits() as i32), CLOUD_SALT + 6) * d2 < THIN_FROM * THIN_FROM
}

struct Patch {
    /// Its columns, then from `idx` its list, which the renderer uploads.
    words: Box<[u32]>,
    handle: u32,
    idx: u32,
    alt_range: [f32; 2],
    /// For each variant, the draw of its models and the draw of its
    /// impostors.
    draws: [[DrawId; 2]; VARIANTS.len()],
}

#[derive(Default)]
struct CloudLayer {
    models: [Option<u32>; VARIANTS.len()],
    patches: FnvHashMap<[i64; 2], Patch>,
    /// The vectors from here on are kept between frames, so a frame where
    /// nothing changed allocates nothing.
    demand: Vec<[i64; 2]>,
    stale: Vec<[i64; 2]>,
    columns: [Vec<[f32; 5]>; VARIANTS.len()],
    impostors: Vec<u32>,
    /// How far from the eye the nearest cloud drawn last frame stands, or
    /// infinity when none was drawn.
    nearest_dist: f64,
}

static CLOUDS: Global<CloudLayer> = Global::new();

fn clouds() -> &'static mut CloudLayer {
    CLOUDS.get(CloudLayer::default)
}

/// Each variant's draw group, with its mesh (0 while it loads), its skin and
/// its extent.
pub(crate) fn cloud_groups() -> impl Iterator<Item = (usize, u32, Option<Image>, [f32; 3])> {
    let models = model::models();
    let skin = load_of(cumulus::SKIN).and_then(load_image);
    clouds()
        .models
        .iter()
        .zip(VARIANTS)
        .enumerate()
        .map(move |(k, (m, file))| {
            let mesh = m.and_then(|m| models.mesh(m)).map_or(0, |m| m.handle);
            (
                FIRST_GROUP + k,
                mesh,
                skin,
                model_extent(catalog(), file).map(|e| e as f32),
            )
        })
}

fn widest(slot: Slot) -> f64 {
    model_extent(catalog(), slot)
        .into_iter()
        .fold(0.0, f64::max)
}

/// The cumulus layer's altitude over the datum, in units. It is the altitude at
/// which the sky draws its cloud layer.
fn layer_u() -> f64 {
    CUMULUS_KM / km_per_u()
}

/// How long, in seconds, the wind takes to carry the clouds the seen step at
/// the distance of the nearest drawn cloud (`lens::visible_step_s`). The
/// clouds' clock steps by this, so a drawn cloud lags the wind by at most the
/// seen step. That step is an angle, so the lag looks the same on every
/// screen. It is infinite while no cloud is drawn.
pub(crate) fn drift_step_s() -> f64 {
    lens::visible_step_s(clouds().nearest_dist, libm::hypot(WIND[0], WIND[1]))
}

/// How much of the clouds shows, from 0 to 1, for the cumulus and the sheets
/// alike (`FrameInput::clouds`). It is 0 with the clouds setting off.
/// Otherwise it is 1 while the eye, at the altitude the sky last saw it, is
/// below half of `CLOUD_GONE_KM`, and it fades to 0 at `CLOUD_GONE_KM`.
pub(crate) fn clouds_shown() -> f64 {
    if !settings().cloud {
        return 0.0;
    }
    let t = ((environment_alt_km() - CLOUD_GONE_KM / 2.0) / (CLOUD_GONE_KM / 2.0)).clamp(0.0, 1.0);
    1.0 - t * t * (3.0 - 2.0 * t)
}

/// Takes the cloud models as they arrive, keeps built the patches in which
/// the eye could see a cloud, and lists their draws, moved as far as the wind
/// has carried the field (`environment_drift`).
pub(crate) fn step(draw: &mut DrawList, frame: &Frame) {
    let c = clouds();
    c.nearest_dist = f64::INFINITY;
    for (slot, file) in c.models.iter_mut().zip(VARIANTS) {
        if slot.is_none() {
            *slot = model_of(file, None);
        }
    }
    let Some(eye) = eye() else { return };
    // With no clouds showing, nothing is built or drawn, and the patches
    // already built are kept for later.
    let opacity = clouds_shown() as f32;
    if opacity <= 0.0 {
        return;
    }
    let off = environment_drift();
    let base = layer_u();
    let largest = CLOUD_WIDTH[1];
    // Patches are wanted out to the eye's draw distance, since a flat layer
    // of clouds never sinks under a horizon, but no farther than the largest
    // cloud stays large enough for an impostor.
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
    c.demand.clear();
    for y in span(1) {
        for x in span(0) {
            let d = near([x, y]).max(1e-3);
            if d <= dist
                && drawing::lod(
                    (largest / d) as f32,
                    decor_model_angle() as f32,
                    decor_impostor_angle() as f32,
                ) != drawing::CULLED
            {
                c.demand.push([x, y]);
            }
        }
    }
    c.demand.sort_unstable();
    c.stale.clear();
    c.stale.extend(
        c.patches
            .keys()
            .copied()
            .filter(|k| c.demand.binary_search(k).is_err()),
    );
    for k in c.stale.drain(..) {
        if let Some(b) = c.patches.remove(&k) {
            draw.free(b.handle);
        }
    }
    let (patches, columns) = (&mut c.patches, &mut c.columns);
    c.demand.retain(|k| !patches.contains_key(k));
    sched::run(c.demand.iter().copied(), |key| {
        match build(key, base, columns, draw) {
            Some(b) => {
                patches.insert(key, b);
                true
            }
            None => false,
        }
    });
    for (&key, b) in c.patches.iter_mut() {
        let corner = [
            key[0] as f64 * PATCH - off[0],
            key[1] as f64 * PATCH - off[1],
        ];
        let [zlo, zhi] = b.alt_range.map(f64::from);
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
        // `nearest_dist` is the distance to the nearest cloud in view, measured to the
        // box it stands in. It is held no nearer than the near plane, short of
        // which nothing is drawn, so even from inside a cloud's box the drift
        // steps by a cell at the near plane rather than by nothing.
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
                c.nearest_dist = c
                    .nearest_dist
                    .min(v3::len(&gap.map(|g| g.max(0.0))).max(param::NEAR));
            }
        }
        let mut patch = PatchDraw {
            handle: b.handle,
            words: &mut b.words,
            idx: b.idx,
            corner: [nearest(corner[0], eye[0]), corner[1]],
            box_corner: box_corner(key, off, eye),
            side: PATCH,
            alt_range: b.alt_range,
            draws: &b.draws,
        };
        // The draw's position holds neither the eye nor the drift, so the
        // draw stays valid while both move.
        let pos = patch_place(key);
        let groups = VARIANTS.iter().enumerate().map(|(k, &a)| {
            (
                (FIRST_GROUP + k) as u32,
                (widest(a), CLOUD_WIDTH.map(|w| w / widest(a))),
            )
        });
        present_patch(
            draw,
            &mut patch,
            pos,
            eye,
            groups,
            opacity,
            &mut c.impostors,
        );
    }
}

/// The corner of patch `key` on the field's map, as the Q3 position its draws
/// hold. The renderer moves it back by the wind's drift when it draws
/// (pxl-render gpu/draw.rs `DrawList::draw_instances`).
fn patch_place(key: [i64; 2]) -> Position {
    Position::of([key[0] as f64 * PATCH, key[1] as f64 * PATCH, 0.0])
}

/// The corner of patch `key` that its draws' stand is built about
/// (`patch_box`), on the lap of X nearest `eye`. It is where the drift
/// `off`, rounded down to a whole cell of the lattice, leaves the corner. The
/// corner as drawn lies less than a cell short of it. A stand grown a cell
/// back from it therefore holds the drawn patch at every drift until the
/// drift crosses into the next cell. Meanwhile the stand stays still, for as
/// long as the wind takes to carry the field a cell.
fn box_corner(key: [i64; 2], off: [f64; 2], eye: V3) -> [f64; 2] {
    let cell_off = off.map(|v| libm::floor(v / CELL) * CELL);
    let x = key[0] as f64 * PATCH - cell_off[0];
    [nearest(x, eye[0]), key[1] as f64 * PATCH - cell_off[1]]
}

/// A patch's stand: the box its draws declare they stand within (pxl-render
/// `ReceiverBox`). The patch's stood corner is `corner` (`box_corner`), its
/// side is `side`, its instances lie between the alts `[zlo, zhi]`, and
/// its models reach `e` past them. The box is grown a cell back from the
/// corner, so it holds the patch at every drift for which that corner holds.
fn patch_box(corner: [f64; 2], side: f64, e: f64, [zlo, zhi]: [f64; 2]) -> ReceiverBox {
    let [x, y] = corner;
    ReceiverBox::of(
        [x - CELL - e, y - CELL - e, zlo - e],
        [x + side + e, y + side + e, zhi + e],
    )
}

/// A cloud of a patch. `local` is its place relative to the patch's corner on
/// the field's map.
struct Cloud {
    variant: usize,
    local: [f64; 2],
    /// The altitude of its middle and its width, in units.
    z: f64,
    width: f64,
    spin: f64,
    scale: f64,
}

/// The clouds of patch `key`: one in each cell where the field's hash says
/// there is one.
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
            // The patch's mean count of clouds is handed down to its cells
            // (pxl-content cloud.rs `holds_cloud`), the way the counts of the
            // decor and the caves are.
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
            let height = model_extent(catalog(), file)[2] * scale;
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

/// The patch's clouds as `build` laid them out, read back from its columns:
/// each one's middle relative to the patch's corner, with its altitude over the
/// datum, and its half extent. Reading them back asks nothing of the field's
/// hash, which is asked once per patch when it is built, and asks the catalog
/// once per variant.
fn patch_clouds(b: &Patch) -> impl Iterator<Item = ([f64; 3], [f64; 3])> + '_ {
    let words = &b.words;
    column::groups(words)
        .zip(VARIANTS.map(|file| (widest(file), model_extent(catalog(), file)[2])))
        .flat_map(move |((first, n), (wide, tall))| {
            let column = GroupRange {
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

/// Builds the clouds of patch `key` into decor's columns, relative to the
/// patch's corner on the field's map, and uploads them. Returns None when the
/// heap refuses the room.
fn build(
    key: [i64; 2],
    base: f64,
    columns: &mut [Vec<[f32; 5]>; VARIANTS.len()],
    draw: &mut DrawList,
) -> Option<Patch> {
    for l in columns.iter_mut() {
        l.clear();
    }
    let mut alt_range = [f32::INFINITY, f32::NEG_INFINITY];
    for c in clouds_of(key, base) {
        alt_range = [
            alt_range[0].min((c.z - c.width / 2.0) as f32),
            alt_range[1].max((c.z + c.width / 2.0) as f32),
        ];
        columns[c.variant].push([
            c.local[0] as f32,
            c.local[1] as f32,
            c.z as f32,
            c.spin as f32,
            c.scale as f32,
        ]);
    }
    let instance_cnt: usize = columns.iter().map(Vec::len).sum();
    let hdr = 1 + columns.len();
    let words = hdr
        + columns
            .iter()
            .map(|l| GroupRange::word_cnt(l.len()))
            .sum::<usize>();
    let idx = (words * 4) as u32;
    let mut out = heap::zeroed::<u32>(words + instance_cnt)?;
    out[0] = columns.len() as u32;
    let mut first = hdr;
    for (g, l) in columns.iter().enumerate() {
        out[1 + g] = l.len() as u32;
        let column = GroupRange {
            first,
            cnt: l.len(),
        };
        for (i, v) in l.iter().enumerate() {
            for (k, &w) in v.iter().enumerate() {
                out[column.f32_at(k, i)] = w.to_bits();
            }
        }
        out[column.flags()..column.flags() + l.len().div_ceil(4)].fill(0);
        first += GroupRange::word_cnt(l.len());
    }
    let (at, bytes) = mem::span(&out[..]);
    let handle = draw.upload(at, bytes, 0, 0);
    Some(Patch {
        words: out,
        handle,
        idx,
        alt_range: if instance_cnt == 0 {
            [0.0; 2]
        } else {
            alt_range
        },
        draws: core::array::from_fn(|_| Default::default()),
    })
}

/// A built patch as `present_patch` reads it. `corner` is the corner of its
/// square on the lap of X nearest the eye, and `alt_range` are the alts its
/// instances stand between.
struct PatchDraw<'a> {
    handle: u32,
    words: &'a mut [u32],
    idx: u32,
    corner: [f64; 2],
    /// The corner its draws stand by (`box_corner`).
    box_corner: [f64; 2],
    side: f64,
    alt_range: [f32; 2],
    draws: &'a [[DrawId; 2]],
}

/// Lists the draws of one patch's instances, each drawn as a model, as an
/// impostor or not at all. `groups` gives a draw group for each of the
/// patch's variants, in the patch's order, with its model's extent and the
/// least and most scale its instances are drawn at. Under an `opacity` of
/// one, the models are blended over the world and the impostors are left
/// out, because an impostor cannot be blended.
fn present_patch(
    draw: &mut DrawList,
    b: &mut PatchDraw,
    pos: Position,
    eye: V3,
    groups: impl Iterator<Item = (u32, (f64, [f64; 2]))>,
    opacity: f32,
    impostors: &mut Vec<u32>,
) {
    let (model, impostor) = (decor_model_angle() as f32, decor_impostor_angle() as f32);
    let lod = |across: f64| drawing::lod(across as f32, model, impostor);
    let [x0, y0] = b.corner;
    let [zlo, zhi] = b.alt_range.map(f64::from);
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
    let (columns_of, list) = b.words.split_at_mut(b.idx as usize / 4);
    // A patch wholly within THIN_FROM shows every cloud, so no cloud's draw
    // is tested.
    let thin = farthest > THIN_FROM;
    let (mut at, mut changed) = (0usize, false);
    let variants = column::groups(columns_of).zip(groups).zip(b.draws);
    for (((off, cnt), (group, (extent, [least, most]))), [model_id, impostor_id]) in variants {
        let column = GroupRange {
            first: off as usize,
            cnt: cnt as usize,
        };
        let first = at;
        impostors.clear();
        let culled = lod(extent * most / nearest) == drawing::CULLED;
        let modeled = lod(extent * least / farthest) == drawing::MODEL;
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
            let t = lod(extent * f(4) / d);
            if t == drawing::MODEL {
                changed |= list[at] != i as u32;
                list[at] = i as u32;
                at += 1;
            } else if t == drawing::IMPOSTOR && opacity >= 1.0 {
                impostors.push(i as u32);
            }
        }
        let models = (at - first) as u32;
        changed |= list[at..at + impostors.len()] != impostors[..];
        list[at..at + impostors.len()].copy_from_slice(impostors);
        at += impostors.len();
        let idx = b.idx / 4 + first as u32;
        // The group's draws read the sun's map, so they declare a box to stand
        // within. It is the patch's box grown by the reach of the group's
        // models at their most scale, and grown a cell back from the stood
        // corner, so it holds the draws at every drift that corner holds
        // (`box_corner`).
        let receiver = patch_box(b.box_corner, b.side, extent * most, [zlo, zhi]);
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
                Some(receiver),
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
                Some(receiver),
            );
        }
    }
    if changed {
        draw.rewrite_part(b.handle, mem::addr(&b.words[0]), b.idx, at as u32 * 4);
    }
}

#[cfg(test)]
#[path = "cloud.test.rs"]
mod test;
