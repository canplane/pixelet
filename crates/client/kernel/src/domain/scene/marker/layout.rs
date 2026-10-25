// pxl-kernel/domain/scene/marker/layout.rs

//! Where the markers are drawn, each by its own source: a place as Carto
//! draws it, read from the tile that says its ground at the zoom the view
//! is at there (`place.rs`); a marker someone put up, near enough
//! (`pin.rs`); a line said, hung over its speaker's head where its own rule
//! shows it (`line.rs`). Each source hands the layout an `Instance`, and the
//! layout asks each the same: its design, its size, its slot and whether it
//! is ready (`design.rs`), and where a tap on it goes.

use super::selection::select_visible;
use super::{line, pin, place};

use crate::domain::chunk::Chunk;
use crate::domain::query::{at_zoom, Ray, World};
use crate::domain::scene::marker::batch::Batch;
use crate::domain::scene::marker::design::{self, Design};
use crate::domain::scene::marker::set::{self, Set};
use crate::domain::view::frame::pose::ndc_to_px;
use crate::domain::view::setting::store::settings_store;
use crate::sys::mem::section;
use crate::sys::sched::tick::TickContext;
use pxl_render::gpu::sheet::shelf::NO_SLOT;
use pxl_spec::bake::constant::EntryAt;
use pxl_spec::space::coord::v3::{self, V3};
use xpute_guest::global::Global;
use xpute_kit::status::bug::OrBug;
use xpute_kit::status::errno::Errno;

use fnv::{FnvHashMap, FnvHashSet};
use pxl_spec::bake::tile::place::SPACING_CELLS;
use pxl_spec::space::camera::grid::px_per_unit;
use pxl_spec::space::coord::q3::{self, Q3};
use pxl_spec::space::coord::zoom::Zoom;
use pxl_spec::space::layer::tile::Tile;

#[derive(Default)]
struct Placement {
    batch: Batch,
    kept: Vec<u32>,
    set: Set,
    drawn: Vec<u32>,
    /// Nearest the eye first: screen box, and where a tap on it goes.
    hits: Vec<([f64; 4], Went)>,
    /// Kept so a settled turn asks the heap for nothing; a chunk's in a
    /// run, as each source reads it (`pin.rs`, `place.rs`).
    pins: Vec<pin::Pin>,
    places: Vec<(Chunk, Instance)>,
    /// The drawn chunks `pins` and `places` were read from: a chunk drawn still keeps
    /// what was read from it, so an eye move casts rays only in the chunks it
    /// enters.
    found_for: Option<Found>,
    /// The same, read this turn.
    now: Found,
    /// Places, ground or objects moved since they were read.
    moved: bool,
    /// The place tiles whose answer moved, beside the ground's
    /// (domain/query/changed.rs): a chunk they touch is read again.
    moved_places: Vec<Tile>,
    /// The chunks this turn would read, by how far from the eye.
    entering: Vec<(f64, Chunk)>,
    sized: Vec<Instance>,
    /// Last turn's `sized`, its buffer reused.
    spare: Vec<Instance>,
    /// Designs not ready yet, kept to become so, each by the index in
    /// `sized` of the instance showing its old drawing meanwhile.
    pending: Vec<(u32, Design)>,
    /// What was drawn last by its holder's key, and the designs the markers
    /// keep: made each run, their room kept.
    before: FnvHashMap<Key, Design>,
    keep: FnvHashSet<Design>,
    /// What the marker set holds, and the buffer the next run lays out
    /// into, swapped as `sized` and `spare` are.
    instances: Vec<f32>,
    spare_instances: Vec<f32>,
    /// What the last run read, compared by value: equal, with nothing moved
    /// and nothing left waiting, the designs it laid out stand.
    last: Option<Seen>,
    /// The last run left a design not laid out or not painted, which a later
    /// turn has, or a chunk not read yet: the next runs again.
    waiting: bool,
}

/// Everything `step` reads, by value, as the demand's `Seen` is: the eye and
/// its view (the observation holds its frustum), the grid's τ, the places
/// setting, the spacing names wrap to, the marker being written and the
/// chat's line and where it hangs. A new input goes here or the step never sees
/// it change.
#[derive(Clone, PartialEq)]
struct Seen {
    o: pxl_spec::space::camera::eye::Observation,
    tau: pxl_spec::space::camera::grid::Tau,
    places: bool,
    spacing: f64,
    drafted: Option<(EntryAt, u64)>,
    said: line::Said,
}

/// The name a holder calls what it shows by, from one run to the next, and
/// none of what is painted (`design.rs`): a marker someone put up by its
/// own (`pin::Held`), a place by its tile and its index there, the chat's line
/// by its speaker, of whom there is one. The layout only tells one from
/// another, to keep an instance's old drawing until its new one is painted;
/// where a tap goes is the instance's own (`Instance::tap`).
#[derive(Clone, Copy, PartialEq, Eq, Hash)]
pub(super) enum Key {
    Pin(pin::Held),
    Place(Tile, u32),
    Line,
}

/// What a source hands the layout: by its holder's key, the design it paints,
/// where it stands, and where a tap on it goes, none where it takes none.
#[derive(Clone)]
pub(super) struct Instance {
    pub(super) key: Key,
    pub(super) design: Design,
    pub(super) stands: Q3,
    pub(super) tap: Option<Went>,
}

static PLACEMENT: Global<Placement> = Global::new();

/// What `pins` and `places` were read from: the drawn chunks, and whether
/// places were on.
#[derive(Default, PartialEq)]
struct Found {
    drawn: Vec<Chunk>,
    places: bool,
}

pub fn moved(places: &[Tile]) {
    let p = PLACEMENT.get(Placement::default);
    p.moved = true;
    p.moved_places.extend_from_slice(places);
}

/// The height a ray down over `at` from `high` first meets what is drawn,
/// above `low`; none where nothing there is known.
pub(super) fn surface(at: &Q3, (high, low): (f64, f64), zoom: Zoom) -> Option<f64> {
    let xy = q3::unpack(at);
    let ray = Ray {
        origin: [xy[0], xy[1], high],
        direction: [0.0, 0.0, -1.0],
        reach: high - low,
    };
    let hit = World::now().first_hit(&ray, at_zoom(zoom)).value().flatten()?;
    Some(high - hit.t)
}

/// The cell a place keeps to itself, as wide as the camera grid's cells are
/// at the view's own τ, its drawn pixels counted as the CSS pixels a name is
/// laid out in, so it is one width at every ratio: its name wraps to it and
/// its marker's region is it. Not the τ in force, which coarsens while
/// the view moves and would lay every name out again; whole pixels, so only
/// the view's size or angle does.
fn spacing_px() -> f64 {
    let v = crate::domain::view::frame::pose::view();
    let drawn = v.height * crate::domain::view::frame::resolution::display_ratio();
    let cell = crate::domain::view::lens::cell_angle(crate::domain::view::lens::tau()) * px_per_unit(drawn, crate::domain::view::param::FOV);
    libm::rint(f64::from(SPACING_CELLS) * cell)
}

pub fn step(ctx: &mut TickContext, frame: &crate::domain::scene::frame::Frame) {
    let p = PLACEMENT.get(Placement::default);
    if crate::domain::view::observation::eye().is_none() {
        if p.set.free(crate::domain::query::model::draw_list()) {
            ctx.invalidate();
        }
        // Nothing is drawn now: what comes back is laid out afresh.
        (p.last, p.waiting, p.found_for) = (None, false, None);
        p.moved_places.clear();
        p.instances.clear();
        p.hits.clear();
        design::retain(&FnvHashSet::default());
        return;
    }
    // No observation yet, the source's reach not declared: laid out afresh
    // once there is one, so the tiles that moved meanwhile are not kept.
    let Some(o) = crate::domain::view::observation::observation() else {
        (p.last, p.found_for) = (None, None);
        p.moved_places.clear();
        return;
    };
    let tau = crate::domain::view::lens::tau();
    let seen = Seen {
        o,
        tau,
        places: settings_store().place,
        spacing: spacing_px(),
        drafted: pin::drafted(),
        said: line::said(o.eye, tau),
    };
    if !p.waiting && !p.moved && p.last.as_ref() == Some(&seen) {
        return;
    }
    p.last = Some(seen.clone());
    // The eye's cubes in view (domain/view/observation.rs `cubes`), as decor's are.
    let now = &mut p.now;
    now.drawn.clear();
    crate::domain::view::observation::cubes(&o, tau, &mut |(zoom, at)| {
        let chunk = Chunk::indexed(zoom, at);
        let b = chunk.region();
        if frame.sees_box(b.min, b.max) {
            now.drawn.push(chunk);
        }
    });
    now.drawn.sort_unstable();
    // Places are a setting; what someone placed is always shown.
    now.places = settings_store().place;
    let places = now.places;
    // What was read stands for a chunk still drawn whose columns no change
    // touches (domain/query/changed.rs), while places did not turn: a chunk left or
    // touched drops its run, and only one entered or touched is read, as
    // many a turn as a pass of the tick's budget reads and at least one,
    // nearest the eye first; the rest are not read yet, so a later turn
    // reads them.
    p.moved = false;
    let before = match &p.found_for {
        Some(f) if f.places == places => f.drawn.as_slice(),
        _ => &[],
    };
    let moved_places = &p.moved_places;
    let stands = |c: &Chunk| {
        let r = c.region();
        before.binary_search(c).is_ok() && !crate::domain::query::changed::within([[r.min[0], r.min[1]], [r.max[0], r.max[1]]], moved_places)
    };
    let (mut pins, mut read_places) = (core::mem::take(&mut p.pins), core::mem::take(&mut p.places));
    pins.retain(|s| now.drawn.binary_search(&s.chunk).is_ok() && stands(&s.chunk));
    read_places.retain(|(c, _)| now.drawn.binary_search(c).is_ok() && stands(c));
    let entering = &mut p.entering;
    entering.clear();
    entering.extend(now.drawn.iter().filter(|c| !stands(c)).map(|&c| (v3::len(&v3::sub(&c.region().center(), &o.eye)), c)));
    entering.sort_unstable_by(|a, b| a.0.total_cmp(&b.0));
    let read = ctx
        .budget
        .run(
            entering.iter(),
            |&(_, chunk)| {
                pin::read(chunk, &mut pins);
                if places {
                    place::read(chunk, &mut read_places);
                }
                true
            },
            None,
        )
        .visited as usize;
    let unread = entering.len() - read;
    // Not read, so not what they were read from.
    let later = &mut entering[read..];
    later.sort_unstable_by_key(|e| e.1);
    now.drawn.retain(|c| later.binary_search_by_key(c, |e| e.1).is_err());
    p.moved_places.clear();
    if p.found_for.as_ref() != Some(&p.now) {
        let now = core::mem::take(&mut p.now);
        p.now = p.found_for.replace(now).unwrap_or_default();
    }
    p.batch.reset();
    let drawn_before = core::mem::take(&mut p.sized);
    let mut sized = core::mem::take(&mut p.spare);
    sized.clear();
    p.pending.clear();
    let shown = pin::shown(&pins).chain(place::shown(&read_places)).chain(seen.said.shown());
    let spacing = seen.spacing;
    // What was drawn last by its holder's key, read once: what a holder
    // shows changed, and what was drawn stays until the new one is ready to
    // show.
    let before = &mut p.before;
    before.clear();
    before.extend(drawn_before.iter().map(|s| (s.key, s.design.clone())));
    let mut unlaid = false;
    // The line is laid out last, so it wins a tie with another marker (the sort's
    // tiebreak).
    for s in shown {
        let (design, region) = s.design.spaced(spacing);
        let (design, held) = match before.get(&s.key) {
            Some(before) if *before != design && !design::ready(&design) => (before.clone(), Some(design)),
            _ => (design, None),
        };
        // Its words are laid out by a later turn.
        let Some([width, height, anchor]) = design::size(&design) else {
            unlaid = true;
            continue;
        };
        let local = crate::domain::view::frame::coord::to_world(&s.stands);
        p.pending.extend(held.map(|held| (sized.len() as u32, held)));
        p.batch.push(local[0], local[1], local[2], [width, height, region], anchor, sized.len() as f64);
        sized.push(Instance { design, ..s });
    }
    // The line's slot is kept while it is said, wherever its speaker is.
    p.keep.clear();
    p.keep
        .extend(sized.iter().map(|s| s.design.clone()).chain(p.pending.iter().map(|w| w.1.clone())).chain(seen.said.kept()));
    let freed = design::retain(&p.keep);
    (p.pins, p.places) = (pins, read_places);
    select_visible(&mut p.batch, &mut p.kept);
    // A design held for an instance seen asks its slot now, after the slots
    // let go are freed, so it is painted at render. Where the sheet has no
    // room for it, the hold is let go: the instance shows the new design,
    // which waits, and the next run frees the old one's slot and asks again.
    // One whose words are not laid out yet stays held and asks nothing: it
    // asks the turn its words come (`design::coming`).
    let mut refused = false;
    for &i in &p.kept {
        let Ok(k) = p.pending.binary_search_by_key(&i, |w| w.0) else { continue };
        if design::size(&p.pending[k].1).is_none() {
            continue;
        }
        if design::slot(&p.pending[k].1) == NO_SLOT {
            sized[i as usize].design = p.pending[k].1.clone();
            refused = true;
        }
    }
    // Painted at render, or laid out by the host, neither of which asks a
    // turn: one looks again, while a design is on its way (`design::coming`),
    // once a slot let go may make room for one that had none, or once a
    // hold let go frees the old slot.
    let pending = p.pending.iter().any(|w| design::coming(&w.1)) || freed && !p.pending.is_empty();
    p.waiting = pending || refused || unlaid || unread > 0;
    if p.waiting {
        crate::sys::sched::edge::request_world_tick();
    }
    p.drawn.clear();
    for &i in &p.kept {
        if design::slot(&sized[i as usize].design) == NO_SLOT {
            continue;
        }
        p.drawn.push(i);
    }
    // Farthest first: where two are both seen, the nearer is drawn over.
    let batch = &p.batch;
    p.drawn.sort_by(|&a, &b| {
        batch.d2[b as usize]
            .total_cmp(&batch.d2[a as usize])
            .then(batch.tiebreak[a as usize].total_cmp(&batch.tiebreak[b as usize]))
    });
    p.hits.clear();
    p.hits.extend(p.drawn.iter().rev().filter_map(|&i| Some((p.batch.screen[i as usize], sized[i as usize].tap?))));
    let mut instances = core::mem::take(&mut p.spare_instances);
    instances.clear();
    // f32 instances, so about the sea under the eye rather than the world's origin.
    let about = crate::domain::view::frame::pose::sea_under_eye();
    for &i in &p.drawn {
        let i = i as usize;
        let at = [p.batch.local_x[i] - about[0], p.batch.local_y[i] - about[1], p.batch.local_z[i] - about[2]];
        let design = &sized[i].design;
        let [w, h, anchor] = design::size(design).or_bug(Errno::ENOTRECOVERABLE);
        instances.extend_from_slice(&set::instance(at, [w, h], design::slot(design) as u32, anchor / h, f64::from(p.batch.region_px[i])));
    }
    // Written, and a frame asked, only where what is drawn changed.
    if instances != p.instances {
        if section::map().is_some() {
            p.set.write(crate::domain::query::model::draw_list(), about, &instances);
        }
        ctx.invalidate();
    }
    let drawn_instances = core::mem::replace(&mut p.instances, instances);
    (p.sized, p.spare, p.spare_instances) = (sized, drawn_before, drawn_instances);
}

/// The markers the last step laid out, into the frame's list.
pub fn draw(draw: &mut pxl_render::frame::draw::DrawList, eye: V3) {
    PLACEMENT.get(Placement::default).set.draw(draw, eye);
}

/// Where a tap on a marker goes: to its place; a marker someone put up is
/// its entry's, which the tool held decides what to do with.
#[derive(Clone, Copy)]
pub enum Went {
    To(Q3),
    Marker(EntryAt, Q3),
}

/// `ndc` in [−1, 1], y up. The marker that took the tap, if any.
pub fn tap(ndc_x: f64, ndc_y: f64) -> Option<Went> {
    let [x, y] = ndc_to_px([ndc_x, ndc_y]);
    let hits = &PLACEMENT.get(Placement::default).hits;
    hits.iter().find(|(r, _)| (r[0]..=r[2]).contains(&x) && (r[1]..=r[3]).contains(&y)).map(|&(_, went)| went)
}
