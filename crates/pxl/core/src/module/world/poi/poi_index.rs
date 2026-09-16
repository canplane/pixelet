// pxl_core/module/world/poi/poi_index.rs

//! The loaded POI set, indexed by where it is, so a camera tick asks a
//! question instead of reading everything.
//!
//! POI is static. It changes when a page arrives or leaves and at no other
//! time, which is the axis this indexes along: build once per page set, and
//! per tick do one radius query into a buffer that already exists. That is the
//! store/index split [issue/20260910-storage-model-precedents.md] arrives at —
//! the store partitions by what does not move, the index by where things are.
//!
//! Positions are held in the local render frame rather than as PVF3, because
//! that is the frame the query is asked in. The frame's origin is L4-aligned
//! and so only shifts every 4096u; when it does, this rebuilds, which is the
//! same trigger a page change already is.
//!
//! Grounded positions, resolved once at build time. The reach test is a
//! sphere and map_center sits on the terrain surface, so an entry held at the
//! bake's flat z is an entry the query cannot find on a mountain — and doing
//! the lookup here costs one pass over the disc per rebuild instead of one per
//! candidate per tick.

use core::cell::Cell;

use pxl_shared::bake::format::PoiPage;
use pxl_shared::spec::space::indexer::{FROM_L3_IDX, L3_IDX, L3_RANGE};
use pxl_shared::spec::space::pvf3::ivec::{self, IVec};
use pxl_shared::spec::space::pvf3::pvf3::PVF3;
use pxl_shared::spec::space::xy::TO_XY;
use xpute_core::ds::map::{I64Keys, IndexedMap};
use xpute_core::ds::spatial_hash::SpatialHash;

use super::poi_badge::evict_poi_badges;
use super::store::poi_pages;
use crate::global::{now, Global};
use crate::module::world::billboard::distance_fade::fade_end_u;
use crate::module::world::sched::tick::FrameBudget;
use crate::module::world::terrain::elevation::with_ele;

/// How far out this keeps POI, as a multiple of the query reach.
///
/// The index holds a disc rather than the whole resident page set, and this
/// is what makes a fixed capacity a statement about geometry instead of a hope
/// about the data. A query is valid as long as everything within the reach of
/// the camera is present, so a disc of the residency around the point it was
/// built at stays valid until the camera moves the margin — which is where the
/// rebuild trigger below comes from. One and a half rather than two: with the
/// reach at 640u the disc is 960u, still inside a page.
const RESIDENCY_OVER_REACH: f64 = 1.5;

fn residency() -> f64 {
    fade_end_u() * RESIDENCY_OVER_REACH
}

const PAGE_U: f64 = L3_RANGE as f64;

/// Pages to sweep around the centre one: as many as the disc can reach. Not
/// every resident page: sweeping all of them would put the whole resident
/// world through a structure whose capacity is meant to be constant.
fn page_radius() -> i64 {
    (residency() / PAGE_U).ceil() as i64
}

/// Capacity is the design, not a guess at the data: a fixed ceiling is what
/// keeps the live object count independent of how much world has been
/// visited. Overflow is reported through `stats()` rather than absorbed. The
/// densest page in the current bake is 12544 POI in one 1024u page, which puts
/// a 960u disc at 35k; this is the next power of two above that.
const CAPACITY: u32 = 65536;

/// A prime above CAPACITY, 2^17 - 1.
const BUCKETS: u32 = 131071;

/// The cell is the query radius at the reference projection. One cell per
/// radius keeps a query to a handful of buckets and a POI in exactly one —
/// Teschner measured that ratio as the optimum.
const CELL: f64 = 640.0;

/// One set of lanes and the hash over them. Two exist: the one queries read
/// and the one a rebuild fills, swept a range of records per step from the
/// frame's content phase, so it can span several frames — and while it does,
/// the previous build keeps answering.
///
/// Entry `i` is the same `i` across every lane and is the id the spatial index
/// was given, so a query answers in indices and nothing between the store and
/// the renderer is ever an object. A record is named by its page and its index
/// in that page's lanes.
struct Lanes {
    index: SpatialHash,
    page_idx: Vec<IVec>,
    rec: Vec<u32>,
    /// Local render-frame position, which is what was indexed — grounded.
    pos_x: Vec<f64>,
    pos_y: Vec<f64>,
    pos_z: Vec<f64>,
    /// 0 while this entry's terrain had not arrived when it was indexed.
    /// Indexed anyway so a query still finds it, but not drawable: the
    /// alternative is drawing it at the bake's z=0, which is underground.
    grounded_ok: Vec<u8>,
    /// The POI's cell, as signed integers relative to the frame origin: the
    /// implicit tree. A node at tier k is `cell >> k` on each axis, so a tier
    /// change costs a shift rather than a rebuild. Integers, exact, since the
    /// frame origin is L4-aligned and every tier's span divides 4096.
    cell_x: Vec<i32>,
    cell_y: Vec<i32>,
    cell_z: Vec<i32>,
    /// The id hash's low 32 bits as a fraction: a stable, even tie-break.
    tiebreak: Vec<f64>,
    /// The cube-ladder level the record's badge last showed, or NO_TIER.
    tier: Vec<u8>,
    dropped: u32,
}

impl Lanes {
    fn new() -> Lanes {
        let n = CAPACITY as usize;
        Lanes {
            index: SpatialHash::new(CAPACITY, BUCKETS, CELL, None),
            page_idx: Vec::with_capacity(n),
            rec: Vec::with_capacity(n),
            pos_x: Vec::with_capacity(n),
            pos_y: Vec::with_capacity(n),
            pos_z: Vec::with_capacity(n),
            grounded_ok: Vec::with_capacity(n),
            cell_x: Vec::with_capacity(n),
            cell_y: Vec::with_capacity(n),
            cell_z: Vec::with_capacity(n),
            tiebreak: Vec::with_capacity(n),
            tier: Vec::with_capacity(n),
            dropped: 0,
        }
    }

    fn count(&self) -> u32 {
        self.rec.len() as u32
    }

    fn clear(&mut self) {
        self.page_idx.clear();
        self.rec.clear();
        self.pos_x.clear();
        self.pos_y.clear();
        self.pos_z.clear();
        self.grounded_ok.clear();
        self.cell_x.clear();
        self.cell_y.clear();
        self.cell_z.clear();
        self.tiebreak.clear();
        self.tier.clear();
        self.dropped = 0;
    }
}

/// Converts an absolute position into the local render frame of `origin`.
pub type ToLocal = fn(&PVF3, IVec) -> [f64; 3];
/// A record's flat position (the bake's, before grounding), absolute.
pub type FlatPos = fn(&PoiPage, u32, IVec) -> PVF3;

/// The unit of work is SWEEP_UNIT records, not a page. A page of the densest
/// bake is over ten thousand records, and the budget's forward-progress rule
/// runs a step's first unit whole — so a page as the unit was a 45 ms step on a
/// phone inside a 2 ms budget. A record inside the disc costs a terrain lookup
/// to ground, so the range is small.
const SWEEP_UNIT: u32 = 64;

struct Job {
    pages_revision: u32,
    residency2: f64,
    map_origin: IVec,
    cx: f64,
    cy: f64,
    pages: Vec<(IVec, i64, i64)>,
    next: usize,
    rec: u32,
    built: bool,
    asked_at: f64,
    frames: u32,
    pages_changed: bool,
    to_local: ToLocal,
    flat_pos: FlatPos,
}

#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct PoiIndexStats {
    pub indexed: u32,
    pub dropped: u32,
    pub rebuilds: u32,
    /// Units the current rebuild has left — its pages, plus the table build —
    /// and 0 when none is running.
    pub sweeping: u32,
    /// The last finished rebuild: ask to swap, and the frames it spanned — the
    /// starvation figures for a content step.
    pub wait_ms: f64,
    pub max_wait_ms: f64,
    pub frames: u32,
    pub max_frames: u32,
}

struct PoiIndex {
    live: Lanes,
    back: Lanes,
    /// What `live` was built from; -1 before any build, and again when the
    /// index is invalidated.
    built_from: i64,
    built_origin: Option<IVec>,
    built_x: f64,
    built_y: f64,
    job: Option<Job>,
    rebuilds: u32,
    last_wait_ms: f64,
    max_wait_ms: f64,
    last_frames: u32,
    max_frames: u32,
    /// The ids seen this rebuild, by their 64-bit hash — a place near a page
    /// boundary is baked into both pages. What eviction asks against, too.
    seen: IndexedMap<I64Keys>,
}

static INDEX: Global<PoiIndex> = Global::new();

fn index() -> &'static mut PoiIndex {
    INDEX.get(|| PoiIndex {
        live: Lanes::new(),
        back: Lanes::new(),
        built_from: -1,
        built_origin: None,
        built_x: 0.0,
        built_y: 0.0,
        job: None,
        rebuilds: 0,
        last_wait_ms: 0.0,
        max_wait_ms: 0.0,
        last_frames: 0,
        max_frames: 0,
        seen: IndexedMap::new(I64Keys, CAPACITY, None),
    })
}

/// Asks for a rebuild if the page set or the frame origin changed, or if the
/// camera has left the disc the live build covers. Cheap enough to call every
/// tick. It only *asks*: the rebuild runs in `poi_index_sweep`, and the live
/// index keeps answering until the new one is whole. Returns true when the
/// live index already reflects these inputs.
///
/// A job already running is replaced only when the page set or the origin
/// moved — a camera that keeps walking during a rebuild would otherwise
/// restart it every frame and never finish.
pub fn sync_poi_index(map_center: &PVF3, map_origin: IVec, to_local: ToLocal, flat_pos: FlatPos) -> bool {
    let x = index();
    // The centre page, and where the camera sits inside it, in page-local real
    // units: a page's own POI already carry page-local centres.
    let center_page = TO_XY(L3_IDX(map_center.i));
    let [cx, cy, _] = to_local(map_center, FROM_L3_IDX(center_page));

    let (moved_x, moved_y) = (cx - x.built_x, cy - x.built_y);
    let margin = residency() - fade_end_u();
    let same_place = moved_x * moved_x + moved_y * moved_y <= margin * margin;
    let pages_revision = poi_pages().revision();
    let pages_changed = pages_revision as i64 != x.built_from;
    if !pages_changed && Some(map_origin) == x.built_origin && same_place {
        return true;
    }

    if x.job.as_ref().is_some_and(|j| j.pages_revision == pages_revision && j.map_origin == map_origin) {
        return false;
    }

    let radius = page_radius();
    let mut pages = Vec::new();
    for dy in -radius..=radius {
        for dx in -radius..=radius {
            pages.push((TO_XY(ivec::add(center_page, ivec::gather(dx, dy, 0))), dx, dy));
        }
    }
    // Nearest first, by distance from the target to each page's rectangle —
    // the same "from where the view is heading" rule as everything else the
    // tick schedules. The table only goes live whole, so this changes which
    // pages a sweep cut short by a job restart has already paid for.
    let page_gap2 = |&(_, dx, dy): &(IVec, i64, i64)| {
        let (x0, y0) = (dx as f64 * PAGE_U, dy as f64 * PAGE_U);
        let gx = if cx < x0 {
            x0 - cx
        } else if cx > x0 + PAGE_U {
            cx - x0 - PAGE_U
        } else {
            0.0
        };
        let gy = if cy < y0 {
            y0 - cy
        } else if cy > y0 + PAGE_U {
            cy - y0 - PAGE_U
        } else {
            0.0
        };
        gx * gx + gy * gy
    };
    // Stable: pages at the same gap — the four around a corner — are swept in
    // the order they were collected, so a rebuild reaches the same records in
    // the same order and a capacity refusal falls on the same one.
    pages.sort_by(|a, b| page_gap2(a).total_cmp(&page_gap2(b)));
    let r = residency();
    x.job = Some(Job {
        pages_revision,
        residency2: r * r,
        map_origin,
        cx,
        cy,
        pages,
        next: 0,
        rec: 0,
        built: false,
        asked_at: now(),
        frames: 0,
        pages_changed,
        to_local,
        flat_pos,
    });

    // The same id can arrive in two pages, since a place near a boundary is
    // baked into both. Deduping once per rebuild rather than per tick is the
    // whole rearrangement.
    x.seen.clear();
    x.back.clear();
    x.back.index.begin();
    false
}

/// One unit of the sweep: the next SWEEP_UNIT records of the page at `next`,
/// into `back`. Advances the cursors, on to the next page when this one is
/// done.
fn sweep_unit(x: &mut PoiIndex) {
    let Some(j) = x.job.as_mut() else { return };
    let lanes = &mut x.back;
    if j.next >= j.pages.len() {
        lanes.index.build();
        j.built = true;
        return;
    }
    let (page_idx, dx, dy) = j.pages[j.next];
    let Some(resident) = poi_pages().get(page_idx) else {
        j.next += 1;
        j.rec = 0;
        return;
    };
    let pois = &resident.page;

    // The camera, expressed in this page's own local frame.
    let (px, py) = (j.cx - dx as f64 * PAGE_U, j.cy - dy as f64 * PAGE_U);

    let end = pois.len.min(j.rec + SWEEP_UNIT);
    for r in j.rec..end {
        // Horizontal only, and deliberately: this runs before grounding, so
        // the only z available is the bake's flat zero while the camera's is a
        // real elevation. A cylinder contains the sphere the exact test uses,
        // so admitting a few extra costs a terrain lookup each and rejects
        // nothing that should have been kept.
        let (ox, oy) = (pois.x.get(r as usize) - px, pois.y.get(r as usize) - py);
        if ox * ox + oy * oy > j.residency2 {
            continue;
        }
        let id_hash = pois.id_hash.get(r as usize);
        if x.seen.find(id_hash as i64) >= 0 {
            continue;
        }
        // Seen before it is refused: a record the capacity turns away is still
        // one this rebuild has met, and leaving it unseen lets the same place
        // in again from the next page it was baked into — which evicts the
        // badge the first one is already drawing.
        x.seen.insert(id_hash as i64);
        if lanes.count() >= CAPACITY {
            lanes.dropped += 1;
            continue;
        }

        // Grounded here rather than per tick, which is the whole point of doing
        // it at build time — and what makes the index isotropic: the query is a
        // sphere in xyz, and map_center carries the terrain height under it.
        let flat = (j.flat_pos)(pois, r, page_idx);
        let mut ground = PVF3::default();
        let grounded = with_ele(&mut ground, flat.i, flat.f);

        // Not snapped here. Which cell a badge stands in depends on its tier
        // and its tier depends on its distance, so the snap belongs to the tick.
        let absolute = grounded.unwrap_or(flat);
        let [lx, ly, lz] = (j.to_local)(&absolute, j.map_origin);
        if !lanes.index.insert(lanes.count(), lx, ly, lz) {
            lanes.dropped += 1;
            continue;
        }

        lanes.page_idx.push(page_idx);
        lanes.rec.push(r);
        lanes.pos_x.push(lx);
        lanes.pos_y.push(ly);
        lanes.pos_z.push(lz);
        // Exact: both are integer lattice addresses, so this is a lattice
        // difference and not a rounded one.
        lanes.cell_x.push((ivec::X(absolute.i) - ivec::X(j.map_origin)) as i32);
        lanes.cell_y.push((ivec::Y(absolute.i) - ivec::Y(j.map_origin)) as i32);
        lanes.cell_z.push((ivec::Z(absolute.i) - ivec::Z(j.map_origin)) as i32);
        lanes.grounded_ok.push(grounded.is_some() as u8);
        lanes.tiebreak.push((id_hash & 0xffff_ffff) as f64 / 4_294_967_296.0);
        lanes.tier.push(NO_TIER as u8);
    }
    j.rec = end;
    if j.rec >= pois.len {
        j.next += 1;
        j.rec = 0;
    }
}

/// Advances the rebuild, a range of records per unit, for as long as the
/// budget allows. Returns true on the step that finishes it — the moment the
/// live index changed and whoever placed against the old one should place
/// again.
///
/// The swap is the only point at which queries see anything new, and it is
/// whole: the table is built, the lanes are complete, and the previous build
/// becomes the next rebuild's scratch.
pub fn poi_index_sweep(budget: &FrameBudget) -> bool {
    let x = index();
    let Some(j) = x.job.as_mut() else { return false };
    j.frames += 1;

    let done = Cell::new(false);
    budget.run(
        std::iter::from_fn(|| (!done.get()).then_some(())),
        |()| {
            let x = index();
            sweep_unit(x);
            done.set(x.job.as_ref().is_none_or(|j| j.built));
            true
        },
        None,
    );
    if !x.job.as_ref().is_some_and(|j| j.built) {
        return false;
    }

    let j = x.job.take().expect("a built job");
    std::mem::swap(&mut x.live, &mut x.back);
    x.built_from = j.pages_revision as i64;
    x.built_origin = Some(j.map_origin);
    x.built_x = j.cx;
    x.built_y = j.cy;
    x.rebuilds += 1;
    x.last_wait_ms = now() - j.asked_at;
    x.max_wait_ms = x.max_wait_ms.max(x.last_wait_ms);
    x.last_frames = j.frames;
    x.max_frames = x.max_frames.max(j.frames);

    // Only on a page change, not on every step the camera takes. A badge just
    // outside the disc is one the camera is walking towards, and evicting it
    // on each rebuild would rebuild its canvas on the way back — a page
    // leaving is the event that actually makes one unreachable.
    if j.pages_changed {
        evict_poi_badges(&x.seen);
    }
    true
}

/// Every indexed POI whose cell is within `radius` of a local-frame point, as
/// indices written into `out`. Broadphase, so the caller still applies its own
/// exact test.
pub fn query_poi_index(x: f64, y: f64, z: f64, radius: f64, out: &mut [u32]) -> u32 {
    index().live.index.query(x, y, z, radius, out)
}

/// A record's page, while it is resident.
pub fn poi_page(i: u32) -> Option<&'static PoiPage> {
    poi_pages().get(index().live.page_idx[i as usize]).map(|r| &r.page)
}

pub fn poi_page_idx(i: u32) -> IVec {
    index().live.page_idx[i as usize]
}

pub fn poi_rec(i: u32) -> u32 {
    index().live.rec[i as usize]
}

pub fn poi_x(i: u32) -> f64 {
    index().live.pos_x[i as usize]
}

pub fn poi_y(i: u32) -> f64 {
    index().live.pos_y[i as usize]
}

pub fn poi_z(i: u32) -> f64 {
    index().live.pos_z[i as usize]
}

pub fn poi_tiebreak(i: u32) -> f64 {
    index().live.tiebreak[i as usize]
}

pub fn poi_cell_x(i: u32) -> i32 {
    index().live.cell_x[i as usize]
}

pub fn poi_cell_y(i: u32) -> i32 {
    index().live.cell_y[i as usize]
}

pub fn poi_cell_z(i: u32) -> i32 {
    index().live.cell_z[i as usize]
}

pub fn poi_grounded(i: u32) -> bool {
    index().live.grounded_ok[i as usize] != 0
}

/// No tier shown yet.
pub const NO_TIER: u32 = 0xff;

pub fn poi_tier(i: u32) -> u32 {
    index().live.tier[i as usize] as u32
}

pub fn set_poi_tier(i: u32, level: u32) {
    index().live.tier[i as usize] = level as u8;
}

/// Forces the next sync to rebuild, and drops a rebuild in progress.
///
/// The trigger this exists for is terrain arriving: elevation is resolved at
/// build time, so an entry indexed before its tile landed carries the flat z
/// and is held back from drawing until a rebuild grounds it. A rebuild already
/// running was grounding against the tiles of the moment it started, so it has
/// to go too — left alone, the next sync saw a job for the same pages and
/// origin and deduped the request against it. A fresh load with every tile
/// arriving after the POI pages drew nothing until the first pan (2026-09-12).
pub fn invalidate_poi_index() {
    let x = index();
    x.built_from = -1;
    x.job = None;
}

pub fn poi_index_stats() -> PoiIndexStats {
    let x = index();
    PoiIndexStats {
        indexed: x.live.count(),
        dropped: x.live.dropped,
        rebuilds: x.rebuilds,
        sweeping: x.job.as_ref().map_or(0, |j| (j.pages.len() - j.next) as u32 + if j.built { 0 } else { 1 }),
        wait_ms: x.last_wait_ms,
        max_wait_ms: x.max_wait_ms,
        frames: x.last_frames,
        max_frames: x.max_frames,
    }
}
