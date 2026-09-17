// pxl-kernel/layer/poi/placement.rs

//! POI's own placement — world-anchored-billboard tier (see
//! issue/20260907-world-rendering-tiers.md), a billboard set beside the chat
//! bubble's. A category-themed badge per place, or a count where places
//! gather; which ones draw is decided here against the index, the lattice, the
//! frustum and the screen.
//!
//! Placement is a job of the frame's content phase. It was one synchronous
//! pass: 2 ms on most runs and 180 ms on a phone over a cold index, none of it
//! under the budget. The same pass runs as stages of bounded units under the
//! budget, and a frame that cannot afford the whole of it does what it can and
//! comes back: query, then file the hits into lattice nodes a thousand at a
//! time, measure and offer the nodes a few hundred at a time, select, build the
//! badges a few at a time, and commit the set in one step so the screen only
//! ever shows a whole placement.
//!
//! A stage's cursor is into scratch the previous stage filled, so nothing is
//! rebuilt between frames — but a query answers in indices into the live index,
//! and a rebuild that finishes mid-job swaps that index out from under them. The
//! job checks the rebuild count and starts over when it moved.

use pxl_render::asset::atlas::NO_SLOT;
use pxl_render::look::lod::{fade_end_u, get_lod_level, lod_step_at, BADGE_SHAMT_OVER_CUBE, LOD_STEP_BY_LEVEL};
use pxl_spec::bake::format::PoiPage;
use pxl_spec::space::indexer::{FROM_L3_IDX, SPAN_CENTER};
use pxl_spec::space::pvf3::ivec::{self, IVec};
use pxl_spec::space::pvf3::pvf3::{self, PVF3};
use pxl_spec::space::v3::V3;

use crate::control::camera::store::{camera_store, ViewState};
use crate::layer::billboard::batch::BillboardBatch;
use crate::layer::billboard::cell_node::CellNodes;
use crate::layer::billboard::frustum_budget::{select_begin, select_place, select_remaining};
use crate::layer::billboard::sheet::{billboard_sheet, new_billboard_set, place_billboard_set, write_billboard_set, BILLBOARD_ORDER};
use crate::layer::poi::badge::{cluster_badge_logical_w, cluster_badge_slot, poi_badge_logical_w, poi_badge_slot, BADGE_CENTER_Y, BADGE_COLLIDE_H, BADGE_LOGICAL_H};
use crate::layer::poi::index::{
    poi_cell_x, poi_cell_y, poi_cell_z, poi_grounded, poi_index_stats, poi_index_sweep, poi_page, poi_rec, poi_tiebreak, poi_tier, poi_x, poi_y, poi_z, query_poi_index, set_poi_tier, sync_poi_index,
    NO_TIER,
};
use crate::sched::tick::{FrameBudget, TickContext};
use crate::shell::map_store::map_store;
use crate::shell::setting::store::settings_store;
use crate::sys::global::{now, Global};
use crate::world::change::request_world_tick;
use crate::world::frame::coord::{get_local_world_origin, local_world_to_canonical};
use crate::world::frame::view::view;

/// A rail, not the mechanism. What decides how many badges are drawn is how
/// many fit: a candidate that lands on pixels another badge has taken is folded
/// into that badge's count, so density answers itself. This only bounds the
/// sprite and draw count if something goes wrong.
const MAX_RENDERED: u32 = 512;

/// The finest and coarsest cell a badge is allowed to snap to, as shifts of the
/// canonical unit. 0 is 1u, the terrain cell itself; 6 is 64u, an L2 chunk and
/// the top of the ladder terrain and decor already climb.
const MIN_SHAMT: u32 = 0;
const MAX_SHAMT: u32 = 6;

/// Which lattice tier a badge snaps to: the cube tier at its range, by the same
/// criterion and the same hysteresis chunks use, raised BADGE_SHAMT_OVER_CUBE
/// tiers. The level it last showed is its record's one-byte state.
fn tier_shamt(dist: f64, i: u32) -> u32 {
    let last = poi_tier(i);
    let current = if last == NO_TIER {
        LOD_STEP_BY_LEVEL[LOD_STEP_BY_LEVEL.len() - 1]
    } else {
        LOD_STEP_BY_LEVEL[last as usize]
    };
    let level = get_lod_level(lod_step_at(dist.max(1e-3), current));
    if level != last {
        set_poi_tier(i, level);
    }
    (level + BADGE_SHAMT_OVER_CUBE).clamp(MIN_SHAMT, MAX_SHAMT)
}

/// center.xyz (page-local real units) resolved against the page's own global
/// corner, untouched. The index grounds it and the lattice snaps it.
pub fn poi_flat_pos(page: &PoiPage, rec: u32, page_idx: IVec) -> PVF3 {
    let center = [page.x.get(rec as usize), page.y.get(rec as usize), page.z.get(rec as usize)];
    let mut out = PVF3::default();
    local_world_to_canonical(&mut out, &center, FROM_L3_IDX(page_idx));
    out
}

/// absolute, projected into the local world frame's origin (map_origin).
pub fn local_pos_from_absolute(absolute: &PVF3, map_origin: IVec) -> V3 {
    let local = PVF3 {
        i: ivec::sub(absolute.i, map_origin),
        f: absolute.f,
    };
    [pvf3::to_real(pvf3::X(&local)), pvf3::to_real(pvf3::Y(&local)), pvf3::to_real(pvf3::Z(&local))]
}

/// What the last placement run cost and how much it threw away, for the ledger.
/// `hits` is what the spatial query returned, `candidates` what survived the
/// reach circle, `kept` what is drawn.
#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct PoiPlaceStats {
    pub ms: f64,
    pub max_ms: f64,
    pub sync_ms: f64,
    pub max_sync_ms: f64,
    pub runs: u32,
    pub hits: u32,
    pub max_hits: u32,
    pub candidates: u32,
    pub kept: u32,
    pub absorbed: u32,
    pub biggest: u32,
    /// Ask to commit, and the frames the job spanned: the starvation question
    /// for a content step.
    pub wait_ms: f64,
    pub max_wait_ms: f64,
    pub frames: u32,
    pub max_frames: u32,
}

#[derive(Clone, Copy, Debug, PartialEq)]
struct PoiPick {
    local_x: f64,
    local_y: f64,
    local_z: f64,
    /// How many places this badge stands for, itself included. Above one it is
    /// drawn as a count rather than as any of them.
    members: u32,
    page_idx_node: u32,
    slot: i32,
    logical_w: f64,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord)]
enum Stage {
    Query,
    File,
    Offer,
    Select,
    Place,
    Build,
    Commit,
}

/// Units, sized so the slowest of each fits a small budget a few times over on
/// a phone: filing is float arithmetic, an offer is a width estimate, a
/// placement is a projection and a grid probe, a build is a sheet entry.
const FILE_UNIT: u32 = 1024;
const OFFER_UNIT: u32 = 256;
const PLACE_UNIT: u32 = 128;
const BUILD_UNIT: u32 = 4;

struct PlaceJob {
    stage: Stage,
    t0: f64,
    wanted_at: f64,
    frames: u32,
    /// The rebuild count the hits were queried against.
    rebuilds: u32,
    map_origin: IVec,
    cx: f64,
    cy: f64,
    cz: f64,
    sync_ms: f64,
    hits: u32,
    /// Cursors: into hits, into nodes, into kept.
    h: u32,
    n: u32,
    kept: u32,
    /// How far the greedy placement has walked the sorted candidates.
    placed: u32,
    k: u32,
    picks: Vec<PoiPick>,
    absorbed: u32,
    biggest: u32,
}

struct Layer {
    /// Sized for the query's *box*, not its circle: the densest page puts a
    /// reach-sized box at ~11k, so this is the next power of two above it — a
    /// query that filled the buffer would truncate in cell order, unrelated to
    /// distance, and drop near badges to keep far ones.
    hits: Vec<u32>,
    batch: BillboardBatch,
    nodes: CellNodes,
    kept: Vec<u32>,
    job: Option<PlaceJob>,
    /// A placement has been asked for since the last one began.
    wanted: bool,
    wanted_at: f64,
    last_start: f64,
    badge_set: Option<u32>,
    stats: PoiPlaceStats,
    /// The inputs a placement depends on, as last seen.
    seen: Option<(ViewState, Option<PVF3>, bool)>,
}

static LAYER: Global<Layer> = Global::new();

fn layer() -> &'static mut Layer {
    LAYER.get(|| Layer {
        hits: vec![0; 16384],
        batch: BillboardBatch::new(),
        nodes: CellNodes::new(4096),
        kept: vec![0; MAX_RENDERED as usize],
        job: None,
        wanted: false,
        wanted_at: 0.0,
        last_start: f64::NEG_INFINITY,
        badge_set: None,
        stats: PoiPlaceStats::default(),
        seen: None,
    })
}

pub fn poi_place_stats() -> PoiPlaceStats {
    layer().stats
}

/// How long placement is allowed to lag the camera. Placement does not have to
/// run every frame — Mapbox GL's own conclusion. What the interval buys here is
/// the whole per-candidate pass, which in the densest part of the bake is
/// thousands of terrain lookups; at 60fps this is a 12x reduction. The anchors
/// themselves never lag: a badge in the set is placed every frame regardless.
const PLACEMENT_MS: f64 = 200.0;

/// Asks for a placement. Called on every input the placement depends on, and
/// cheap enough to be: it sets a flag the content step reads, throttled there
/// to one run per PLACEMENT_MS while inputs keep changing, and once more after
/// they stop.
pub fn poi_place_request() {
    let l = layer();
    if !l.wanted {
        l.wanted_at = now();
    }
    l.wanted = true;
}

/// True while a placement is pending or running — the tick must keep coming,
/// since nothing else may be asking.
fn poi_place_busy() -> bool {
    let l = layer();
    l.wanted || l.job.is_some()
}

/// The core's billboard set the badges are, made on first use.
fn badge_set() -> u32 {
    *layer().badge_set.get_or_insert_with(|| new_billboard_set(BILLBOARD_ORDER::poi))
}

/// Advances the placement under the budget. Returns true on the step that
/// commits one.
fn poi_place_step(budget: &FrameBudget, t: f64, map_center: Option<PVF3>, show_poi: bool, viewport_w: f64, viewport_h: f64) -> bool {
    let l = layer();
    if l.job.is_none() {
        if !l.wanted || t - l.last_start < PLACEMENT_MS {
            return false;
        }
        l.wanted = false;
        l.last_start = t;
        let Some(center) = map_center.filter(|_| show_poi) else {
            write_billboard_set(badge_set(), &[]);
            request_world_tick();
            return true;
        };
        l.job = Some(PlaceJob {
            stage: Stage::Query,
            t0: t,
            wanted_at: l.wanted_at,
            frames: 0,
            rebuilds: 0,
            map_origin: get_local_world_origin(Some(&center)),
            cx: 0.0,
            cy: 0.0,
            cz: 0.0,
            sync_ms: 0.0,
            hits: 0,
            h: 0,
            n: 0,
            kept: 0,
            placed: 0,
            k: 0,
            picks: Vec::new(),
            absorbed: 0,
            biggest: 0,
        });
    }
    let j = l.job.as_mut().expect("a job");
    j.frames += 1;
    // The live index moved under the hits: back to the query.
    if j.stage > Stage::Query && poi_index_stats().rebuilds != j.rebuilds {
        j.stage = Stage::Query;
    }

    let mut committed = false;
    budget.run(
        std::iter::from_fn(|| layer().job.as_ref().map(|j| j.stage)),
        |stage| {
            match stage {
                Stage::Query => query_stage(),
                Stage::File => file_stage(),
                Stage::Offer => offer_stage(),
                Stage::Select => select_stage(viewport_w, viewport_h),
                Stage::Place => place_stage(viewport_w, viewport_h),
                Stage::Build => build_stage(),
                Stage::Commit => {
                    commit_stage(t);
                    committed = true;
                    layer().job = None;
                }
            }
            true
        },
        None,
    );
    committed
}

fn query_stage() {
    let l = layer();
    let j = l.job.as_mut().expect("a job");
    let t0 = now();
    // Everything is measured from the camera, as the chunks are: the disc the
    // index holds, the reach a candidate is inside, the tier it is filed at.
    // Measured from the pivot instead, a wide zoom followed by a tilt down to
    // the ground filed the places under the camera at the tier of the pivot's
    // distance while the fade drew them as if near.
    let eye = view().eye_local;
    (j.cx, j.cy, j.cz) = (eye.x, eye.y, eye.z);
    let mut centre = PVF3::default();
    local_world_to_canonical(&mut centre, &[j.cx, j.cy, j.cz], j.map_origin);

    // Only asks; the rebuild itself is the poi_index step's.
    sync_poi_index(&centre, j.map_origin, local_pos_from_absolute, poi_flat_pos);
    j.sync_ms = now() - t0;
    j.rebuilds = poi_index_stats().rebuilds;
    j.hits = query_poi_index(j.cx, j.cy, j.cz, fade_end_u(), &mut l.hits);
    j.h = 0;
    l.nodes.begin();
    l.batch.reset();
    j.stage = Stage::File;
}

/// Files the next FILE_UNIT hits under the lattice node each belongs to at its
/// own tier. Nothing here is a POI — a node is an address and a count.
fn file_stage() {
    let l = layer();
    let j = l.job.as_mut().expect("a job");
    let far_end = fade_end_u();
    let far_end2 = far_end * far_end;
    let end = j.hits.min(j.h + FILE_UNIT);
    while j.h < end {
        let i = l.hits[j.h as usize];
        j.h += 1;
        // The exact reach, isotropic, on the frame-local lanes the index already
        // holds. Both sides carry a real z: entries are grounded at build time
        // and the camera has its own height.
        let (dx, dy, dz) = (poi_x(i) - j.cx, poi_y(i) - j.cy, poi_z(i) - j.cz);
        let d2 = dx * dx + dy * dy + dz * dz;
        if d2 > far_end2 {
            continue;
        }
        // Indexed before its tile arrived, so its z is the bake's flat zero and
        // drawing it would put the badge underground.
        if !poi_grounded(i) {
            continue;
        }
        // Its own tier, from its own distance: a perspective view holds near
        // and far at once.
        l.nodes.add(poi_cell_x(i), poi_cell_y(i), poi_cell_z(i), tier_shamt(d2.sqrt(), i), i, d2);
    }
    if j.h >= j.hits {
        j.n = 0;
        j.stage = Stage::Offer;
    }
}

/// Offers the next OFFER_UNIT nodes to the screen, each measured.
fn offer_stage() {
    let l = layer();
    let j = l.job.as_mut().expect("a job");
    let end = l.nodes.count.min(j.n + OFFER_UNIT);
    while j.n < end {
        let n = j.n as usize;
        j.n += 1;
        let shamt = l.nodes.shamt[n] as u32;
        let members = l.nodes.members[n];
        let rep = l.nodes.first[n];

        // The badge stands at the centre of the cell, on all three axes, with
        // the half following the tier: exact, and every member of the cell
        // resolves to it, which is what stops the badge moving when the camera
        // does.
        let nx = l.nodes.centre(l.nodes.key_x[n], shamt);
        let ny = l.nodes.centre(l.nodes.key_y[n], shamt);
        let nz = l.nodes.centre(l.nodes.key_z[n], shamt);

        let logical_w = if members > 1 {
            cluster_badge_logical_w(members)
        } else {
            let Some(page) = poi_page(rep) else { continue };
            poi_badge_logical_w(page, poi_rec(rep))
        };
        l.batch.push(n as u32, nx, ny, nz, logical_w, BADGE_COLLIDE_H, BADGE_CENTER_Y, poi_tiebreak(rep));
    }
    if j.n >= l.nodes.count {
        j.stage = Stage::Select;
    }
}

/// The frustum pass and the sort, one unit; then placement a range at a time.
fn select_stage(viewport_w: f64, viewport_h: f64) {
    let l = layer();
    let j = l.job.as_mut().expect("a job");
    j.k = 0;
    j.picks.clear();
    j.absorbed = 0;
    j.biggest = 0;
    select_begin(&mut l.batch, viewport_w, viewport_h);
    j.kept = 0;
    j.placed = 0;
    j.stage = Stage::Place;
}

fn place_stage(viewport_w: f64, viewport_h: f64) {
    let l = layer();
    let j = l.job.as_mut().expect("a job");
    j.placed += PLACE_UNIT;
    j.kept = select_place(&mut l.batch, MAX_RENDERED, viewport_w, viewport_h, &mut l.kept, j.placed);
    if select_remaining() == 0 {
        j.stage = Stage::Build;
    }
}

/// Resolves the next BUILD_UNIT kept nodes into picks and draws the badges that
/// are not yet on the sheet.
fn build_stage() {
    let l = layer();
    let j = l.job.as_mut().expect("a job");
    let end = j.kept.min(j.k + BUILD_UNIT);
    while j.k < end {
        let i = l.kept[j.k as usize] as usize;
        j.k += 1;
        // Two counts, added rather than either replacing the other: what the
        // lattice already gathered, and what the screen gathered on top of it.
        let node = l.batch.src[i];
        let Some(mut pick) = pick_of(&l.nodes, node, l.batch.absorbed[i], j.map_origin) else { continue };
        j.absorbed += pick.members - 1;
        j.biggest = j.biggest.max(pick.members);
        // Drawn now, into the sheet, so the commit only points at rectangles.
        if pick.members > 1 {
            pick.slot = cluster_badge_slot(pick.members);
            pick.logical_w = cluster_badge_logical_w(pick.members);
        } else {
            let rep = l.nodes.first[node as usize];
            let Some(page) = poi_page(rep) else { continue };
            pick.slot = poi_badge_slot(page, poi_rec(rep));
            pick.logical_w = poi_badge_logical_w(page, poi_rec(rep));
        }
        j.picks.push(pick);
    }
    if j.k >= j.kept {
        j.stage = Stage::Commit;
    }
}

/// Floats a badge takes in the core's block (crates/pxl/kernel/src/layer/billboard/field.rs).
const BADGE_FLOATS: usize = 10;

/// Writes the whole placement in one step into the core, which holds the badges
/// against the placement's origin until the next commit.
fn commit_stage(t: f64) {
    let l = layer();
    let j = l.job.as_ref().expect("a job");
    let sheet = billboard_sheet();
    let mut lane: Vec<f32> = Vec::with_capacity(j.picks.len() * BADGE_FLOATS);
    for pick in &j.picks {
        // NO_SLOT when the atlas is full. Nothing is drawn for it this pass,
        // which is the honest answer — the alternative is a rectangle belonging
        // to some other badge.
        if pick.slot == NO_SLOT {
            continue;
        }
        let s = pick.slot as u32;
        lane.extend_from_slice(&[
            pick.local_x as f32,
            pick.local_y as f32,
            pick.local_z as f32,
            pick.logical_w as f32,
            BADGE_LOGICAL_H as f32,
            sheet.u0(s) as f32,
            sheet.v0(s) as f32,
            sheet.u1(s) as f32,
            sheet.v1(s) as f32,
            (BADGE_CENTER_Y / BADGE_LOGICAL_H) as f32,
        ]);
    }
    let origin = j.map_origin;
    let set = badge_set();
    place_billboard_set(set, [ivec::to_real(ivec::X(origin)), ivec::to_real(ivec::Y(origin)), ivec::to_real(ivec::Z(origin))]);
    write_billboard_set(set, &lane);
    // What changed has to ask for the frame that shows it.
    request_world_tick();

    let s = &mut l.stats;
    s.absorbed = j.absorbed;
    s.biggest = j.biggest;
    s.ms = t - j.t0;
    s.sync_ms = j.sync_ms;
    s.max_sync_ms = s.max_sync_ms.max(j.sync_ms);
    // A worst case, not a percentile: the question this answers is whether one
    // run can ever cost a frame, which is a max question.
    s.max_ms = s.max_ms.max(s.ms);
    s.max_hits = s.max_hits.max(j.hits);
    s.wait_ms = t - j.wanted_at;
    s.max_wait_ms = s.max_wait_ms.max(s.wait_ms);
    s.frames = j.frames;
    s.max_frames = s.max_frames.max(j.frames);
    s.runs += 1;
    s.hits = j.hits;
    s.candidates = l.batch.count;
    s.kept = j.kept;
}

/// One node turned into the record drawn — built as many times as are drawn,
/// not once per candidate. The anchor is a lattice address plus the half its
/// tier is worth.
fn pick_of(nodes: &CellNodes, node: u32, absorbed_extra: u32, map_origin: IVec) -> Option<PoiPick> {
    let n = node as usize;
    let shamt = nodes.shamt[n] as u32;
    let base = ivec::add(
        map_origin,
        ivec::gather((nodes.key_x[n] as i64) << shamt, (nodes.key_y[n] as i64) << shamt, (nodes.key_z[n] as i64) << shamt),
    );
    let mut anchor = PVF3::default();
    SPAN_CENTER(&mut anchor, base, shamt);
    let [lx, ly, lz] = local_pos_from_absolute(&anchor, map_origin);
    Some(PoiPick {
        local_x: lx,
        local_y: ly,
        local_z: lz,
        members: nodes.members[n] + absorbed_extra,
        page_idx_node: node,
        slot: NO_SLOT,
        logical_w: 0.0,
    })
}

/// Every input a placement depends on asks for one; the step throttles. Terrain
/// arriving is what grounds an entry indexed before its tile did, so the
/// response to it is to rebuild the index and re-place.
pub fn poi_layer_watch_step() {
    // The view as well as the centre: a rotate or a zoom changes what is in
    // frustum without moving the centre. Pages coming and going ask for
    // placement where they change (poi/store.rs, terrain/store.rs).
    let view_state = camera_store().view_state;
    let map_center = map_store().map_center;
    let show_poi = settings_store().show_poi;
    let l = layer();
    let now_seen = (view_state, map_center, show_poi);
    if l.seen == Some(now_seen) {
        return;
    }
    l.seen = Some(now_seen);
    poi_place_request();
    request_world_tick();
}

/// The "poi_index" step of the content phase: the index is rebuilt a range of
/// records per step, and while that runs the previous build keeps answering —
/// so the step that finishes a rebuild is the moment to place again.
pub fn poi_index_step(ctx: &mut TickContext) {
    if poi_index_sweep(&ctx.budget) {
        poi_place_request();
    }
    if poi_index_stats().sweeping > 0 {
        ctx.invalidate();
    }
}

/// The "poi_place" step of the content phase, after the index's so a finished
/// rebuild is placed against in the same frame.
pub fn poi_place_tick_step(ctx: &mut TickContext) {
    poi_layer_watch_step();
    let v = view();
    poi_place_step(&ctx.budget, now(), map_store().map_center, settings_store().show_poi, v.width, v.height);
    if poi_place_busy() {
        ctx.invalidate();
    }
}
