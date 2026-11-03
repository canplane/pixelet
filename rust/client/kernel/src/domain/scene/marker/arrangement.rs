// pxl-kernel/domain/scene/marker/arrangement.rs

//! The markers' arrangement: where each is drawn, read by its own source
//! over the eye's clip levels' bricks, as decor is generated over them
//! (domain/view/observation.rs `bricks`), and shown at the zoom the camera
//! grid's cell at its own point is: a place as Carto draws it at that zoom
//! (`place.rs`); a pin someone put up, near enough (`pin.rs`); a line said,
//! hung over its speaker's head where its speaker shows it (`line.rs`). A
//! brick is only what a source is read over. Each source hands the
//! arrangement an `Instance`, and the arrangement asks each the same: its
//! design, its size, its slot and whether it is ready (`design.rs`), and
//! where a tap on it goes.

use super::selection::select_visible;
use super::{line, pin, place};

use crate::domain::query::{at_zoom, Ray, World};
use crate::domain::scene::marker::candidate::Candidates;
use crate::domain::scene::marker::design::{self, Design};
use crate::domain::scene::marker::set::{self, Set};
use crate::domain::view::frame::screen::{ndc_to_px, units_per_px_at};
use crate::domain::view::setting::store::settings_store;
use crate::sys::mem::section;
use crate::sys::sched::tick::TickContext;
use pxl_render::gpu::atlas::shelf::NO_SLOT;
use pxl_spec::space::coord::v3::{self, V3};
use pxl_spec::space::layer::octant::Octant;
use xpute_guest::global::Global;
use xpute_kit::status::bug::OrBug;
use xpute_kit::status::errno::Errno;

use crate::domain::query::changes;
use crate::domain::scene::frame::Frame;
use crate::domain::scene::model;
use crate::domain::view::frame::coord;
use crate::domain::view::frame::pose;
use crate::domain::view::observation;
use crate::sys::sched::edge;
use fnv::{FnvHashMap, FnvHashSet};
use pxl_engine::clipmap::BRICK_SHIFT;
use pxl_render::frame::draw::DrawList;
use pxl_spec::space::camera::eye::Observation;
use pxl_spec::space::coord::aabb::Aabb;
use pxl_spec::space::coord::q3::{self, Q3};
use pxl_spec::space::coord::world::BOUNDS;
use pxl_spec::space::coord::zoom::{units, Zoom};
use pxl_spec::space::hierarchy::tree::Node;
use pxl_spec::space::layer::quadrant::Quadrant;

/// A brick of the eye's clip levels the markers are read over, by its zoom
/// and place, X running on round the world as the view's frame does
/// (domain/view/observation.rs `bricks`).
pub(super) type Brick = Node<2>;

/// Its cells' zoom: the camera grid's at its nearest point, its finest.
pub(super) fn cell_zoom((zoom, _): Brick) -> Zoom {
    zoom + BRICK_SHIFT
}

/// The box over `brick` between `heights`.
pub(super) fn region((zoom, [x, y]): Brick, [low, high]: [f64; 2]) -> Aabb {
    let span = units(zoom);
    let (x0, y0) = (f64::from(x) * span, f64::from(y) * span);
    Aabb {
        min: [x0, y0, low],
        max: [x0 + span, y0 + span, high],
    }
}

#[derive(Default)]
struct Arrangement {
    candidates: Candidates,
    visible: Vec<u32>,
    set: Set,
    draw_order: Vec<u32>,
    /// Nearest the eye first: screen box, and where a tap on it goes.
    hits: Vec<([f64; 4], MarkerHit)>,
    /// Kept so a settled turn asks the heap for nothing; a brick's in a
    /// run, as each source reads it (`pin.rs`, `place.rs`).
    pins: Vec<pin::Pin>,
    places: Vec<place::Place>,
    /// What the sources show this run, its room kept.
    shown: Vec<Instance>,
    /// The drawn bricks `pins` and `places` were read from: a brick drawn
    /// still keeps what was read from it, so an eye move casts rays only in
    /// the bricks it enters.
    found_for: Option<Read>,
    /// The same, read this turn.
    now: Read,
    /// Places, ground or objects moved since they were read.
    moved: bool,
    /// The place tiles whose answer moved, beside the ground's
    /// (domain/query/changes.rs): a brick they touch is read again.
    moved_places: Vec<Quadrant>,
    /// The bricks this turn would read, by how far from the eye, with
    /// their columns.
    entering: Vec<(f64, Brick, Aabb)>,
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
    last: Option<Inputs>,
    /// The last run left a design not laid out or not painted, which a later
    /// turn has, or a brick not read yet: the next runs again.
    waiting: bool,
}

/// Everything `step` reads, by value, as the demand's `Key` is: the eye and
/// its view (the observation holds its frustum), the places
/// setting, the pin being written and the chat's line and where it hangs. A
/// new input goes here or the step never sees it change.
#[derive(Clone, PartialEq)]
struct Inputs {
    o: Observation,
    places: bool,
    drafted: Option<(Octant, u64)>,
    bubble: line::Bubble,
}

/// The name a holder calls what it shows by, from one run to the next, and
/// none of what is painted (`design.rs`): a pin by its own
/// (`pin::Holder`), a place by its tile and its index there, the chat's line
/// by its speaker, of whom there is one. The arrangement only tells one from
/// another, to keep an instance's old drawing until its new one is painted;
/// where a tap goes is the instance's own (`Instance::tap`).
#[derive(Clone, Copy, PartialEq, Eq, Hash)]
pub(super) enum Key {
    Pin(pin::Holder),
    Place(Quadrant, u32),
    Line,
}

/// What a source hands the arrangement: by its holder's key, the design it paints,
/// where it stands, and where a tap on it goes, none where it takes none.
#[derive(Clone)]
pub(super) struct Instance {
    pub(super) key: Key,
    pub(super) design: Design,
    pub(super) stands: Q3,
    pub(super) tap: Option<MarkerHit>,
}

static ARRANGEMENT: Global<Arrangement> = Global::new();

/// What `pins` and `places` were read from: the drawn bricks, and whether
/// places were on. What a brick holds is read over its whole columns, so
/// it stands while no change touches them, whatever heights the world
/// stands between in it.
#[derive(Default, PartialEq)]
struct Read {
    bricks: Vec<Brick>,
    places: bool,
}

/// The most a marker is drawn past where it stands, in pixels, which the
/// cull of a brick leaves it: a bound, twice a place's cell, the width its
/// name wraps to (`SPACING_PX`), past a pin's longest pill and a name's
/// lines over its icon.
fn extent_px(spacing: f64) -> f64 {
    2.0 * spacing
}

pub fn on_moved(places: &[Quadrant]) {
    let p = ARRANGEMENT.get(Arrangement::default);
    p.moved = true;
    p.moved_places.extend_from_slice(places);
}

/// The height a ray down over `at` from `high` first meets what is drawn,
/// above `low`; none where nothing there is known.
pub(super) fn surface(at: &Q3, (high, low): (f64, f64), zoom: Zoom) -> Option<f64> {
    let xy = q3::unpack(at);
    let ray = Ray {
        origin: [xy[0], xy[1], high],
        dir: [0.0, 0.0, -1.0],
        t_max: high - low,
    };
    let hit = World::now()
        .first_hit(&ray, at_zoom(zoom))
        .val()
        .flatten()?;
    Some(high - hit.t)
}

/// The width a place keeps to itself on the screen, in CSS pixels: its name
/// wraps to it and its marker's region is it. A marker is laid out as the
/// shell's text is, in CSS pixels, so the width is one on every screen, set
/// by eye to hold a name of a few words.
const SPACING_PX: f64 = 193.0;

pub fn step(ctx: &mut TickContext, frame: &Frame) {
    let p = ARRANGEMENT.get(Arrangement::default);
    if observation::eye().is_none() {
        if p.set.free(model::draw_list()) {
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
    let Some(o) = observation::observation() else {
        (p.last, p.found_for) = (None, None);
        p.moved_places.clear();
        return;
    };
    let seen = Inputs {
        o,
        places: settings_store().place,
        drafted: pin::draft_key(),
        bubble: line::bubble(o.eye),
    };
    if !p.waiting && !p.moved && p.last.as_ref() == Some(&seen) {
        return;
    }
    p.last = Some(seen.clone());
    // The eye's clip levels' bricks in view (domain/view/observation.rs
    // `bricks`), as decor's are: the box the world stands in over each,
    // grown by what a marker draws past it at the box's far side; none past
    // where the rays end, over ground the sky's Earth stands for.
    let now = &mut p.now;
    now.bricks.clear();
    let extent = extent_px(SPACING_PX);
    let drawn = observation::eye_draw_dist().unwrap_or(0.0);
    observation::bricks(&o, &mut |brick, heights| {
        let b = region(brick, heights);
        if o.measure(&b).near > drawn {
            return;
        }
        let far = v3::len(&core::array::from_fn(|k| {
            (b.min[k] - o.eye[k]).abs().max((b.max[k] - o.eye[k]).abs())
        }));
        let pad = extent * units_per_px_at(far);
        if frame.sees_box(b.min.map(|v| v - pad), b.max.map(|v| v + pad)) {
            now.bricks.push(brick);
        }
    });
    now.bricks.sort_unstable();
    // Places are a setting; what someone placed is always shown.
    now.places = settings_store().place;
    let places = now.places;
    // What was read stands for a brick still drawn whose columns no change
    // touches (domain/query/changes.rs), while places did not turn: a
    // brick left or touched drops its run, and only one entered or touched
    // is read, as many a turn as a pass of the tick's budget reads and at
    // least one, nearest the eye first; the rest are not read yet, so a
    // later turn reads them.
    p.moved = false;
    let before = match &p.found_for {
        Some(f) if f.places == places => f.bricks.as_slice(),
        _ => &[],
    };
    let moved_places = &p.moved_places;
    let columns = |s: Brick| region(s, [BOUNDS.min[2], BOUNDS.max[2]]);
    // Whether each drawn brick stands, asked once a brick however many pins
    // and places it holds.
    let stood: Vec<bool> = now
        .bricks
        .iter()
        .map(|&s| {
            let r = columns(s);
            before.binary_search(&s).is_ok()
                && !changes::within([[r.min[0], r.min[1]], [r.max[0], r.max[1]]], moved_places)
        })
        .collect();
    let stands = |s: &Brick| now.bricks.binary_search(s).is_ok_and(|k| stood[k]);
    let (mut pins, mut read_places) =
        (core::mem::take(&mut p.pins), core::mem::take(&mut p.places));
    pins.retain(|s| stands(&s.brick));
    read_places.retain(|s| stands(&s.brick));
    let entering = &mut p.entering;
    entering.clear();
    entering.extend(
        now.bricks
            .iter()
            .zip(&stood)
            .filter(|(_, &st)| !st)
            .map(|(&s, _)| {
                let r = columns(s);
                let middle = [(r.min[0] + r.max[0]) / 2.0, (r.min[1] + r.max[1]) / 2.0];
                (
                    libm::hypot(middle[0] - o.eye[0], middle[1] - o.eye[1]),
                    s,
                    r,
                )
            }),
    );
    entering.sort_unstable_by(|a, b| a.0.total_cmp(&b.0));
    let read = ctx
        .pass
        .run(
            entering.iter(),
            |&(_, brick, columns)| {
                pin::read(brick, &columns, &mut pins);
                if places {
                    place::read(brick, &mut read_places);
                }
                true
            },
            None,
        )
        .step_cnt as usize;
    let unread = entering.len() - read;
    // Not read, so not what they were read from.
    let later = &mut entering[read..];
    later.sort_unstable_by_key(|e| e.1);
    now.bricks
        .retain(|d| later.binary_search_by_key(d, |e| e.1).is_err());
    p.moved_places.clear();
    if p.found_for.as_ref() != Some(&p.now) {
        let now = core::mem::take(&mut p.now);
        p.now = p.found_for.replace(now).unwrap_or_default();
    }
    p.candidates.reset();
    let drawn_before = core::mem::take(&mut p.sized);
    let mut sized = core::mem::take(&mut p.spare);
    sized.clear();
    p.pending.clear();
    // Each shown at the zoom the grid's cell at its own point is.
    let mut shown = core::mem::take(&mut p.shown);
    shown.clear();
    pin::show(&mut pins, o.eye, &mut shown);
    place::show(&mut read_places, o.eye, &mut shown);
    shown.extend(seen.bubble.instance());
    let spacing = SPACING_PX;
    // What was drawn last by its holder's key, read once: what a holder
    // shows changed, and what was drawn stays until the new one is ready to
    // show.
    let before = &mut p.before;
    before.clear();
    before.extend(drawn_before.iter().map(|s| (s.key, s.design.clone())));
    let mut unlaid = false;
    // The line is laid out last, so it wins a tie with another marker (the sort's
    // tiebreak).
    for s in shown.drain(..) {
        let (design, region) = s.design.with_spacing(spacing);
        let (design, held) = match before.get(&s.key) {
            Some(before) if *before != design && !design::ready(&design) => {
                (before.clone(), Some(design))
            }
            _ => (design, None),
        };
        // Its text is laid out by a later turn.
        let Some([width, height, anchor]) = design::size(&design) else {
            unlaid = true;
            continue;
        };
        let local = coord::to_world(&s.stands);
        p.pending
            .extend(held.map(|held| (sized.len() as u32, held)));
        p.candidates.push(
            local[0],
            local[1],
            local[2],
            [width, height, region],
            anchor,
            sized.len() as f64,
        );
        sized.push(Instance { design, ..s });
    }
    // The line's slot is kept while it is said, wherever its speaker is.
    p.keep.clear();
    p.keep.extend(
        sized
            .iter()
            .map(|s| s.design.clone())
            .chain(p.pending.iter().map(|w| w.1.clone()))
            .chain(seen.bubble.design()),
    );
    let freed = design::retain(&p.keep);
    (p.pins, p.places, p.shown) = (pins, read_places, shown);
    select_visible(&mut p.candidates, &mut p.visible);
    // A design held for an instance seen asks its slot now, after the slots
    // let go are freed, so it is painted at render. Where the atlas has no
    // room for it, the hold is let go: the instance shows the new design,
    // which waits, and the next run frees the old one's slot and asks again.
    // One whose text is not laid out yet stays held and asks nothing: it
    // asks the turn its text comes (`design::coming`).
    let mut refused = false;
    for &i in &p.visible {
        let Ok(k) = p.pending.binary_search_by_key(&i, |w| w.0) else {
            continue;
        };
        if design::size(&p.pending[k].1).is_none() {
            continue;
        }
        if design::slot(&p.pending[k].1) == NO_SLOT {
            sized[i as usize].design = p.pending[k].1.clone();
            refused = true;
        }
    }
    // Painted at render, or laid out by the driver, neither of which asks a
    // turn: one looks again, while a design is on its way (`design::coming`),
    // once a slot let go may make room for one that had none, or once a
    // hold let go frees the old slot.
    let pending = p.pending.iter().any(|w| design::coming(&w.1)) || freed && !p.pending.is_empty();
    p.waiting = pending || refused || unlaid || unread > 0;
    if p.waiting {
        edge::request_world_tick();
    }
    p.draw_order.clear();
    for &i in &p.visible {
        if design::slot(&sized[i as usize].design) == NO_SLOT {
            continue;
        }
        p.draw_order.push(i);
    }
    // Farthest first: where two are both seen, the nearer is drawn over.
    let candidates = &p.candidates;
    p.draw_order.sort_by(|&a, &b| {
        candidates.d2[b as usize]
            .total_cmp(&candidates.d2[a as usize])
            .then(candidates.tiebreak[a as usize].total_cmp(&candidates.tiebreak[b as usize]))
    });
    p.hits.clear();
    p.hits.extend(
        p.draw_order
            .iter()
            .rev()
            .filter_map(|&i| Some((p.candidates.screen[i as usize], sized[i as usize].tap?))),
    );
    let mut instances = core::mem::take(&mut p.spare_instances);
    instances.clear();
    // f32 instances, so about the sea under the eye rather than the world's origin.
    let about = pose::sea_under_eye();
    for &i in &p.draw_order {
        let i = i as usize;
        let at = [
            p.candidates.local_x[i] - about[0],
            p.candidates.local_y[i] - about[1],
            p.candidates.local_z[i] - about[2],
        ];
        let design = &sized[i].design;
        let [w, h, anchor] = design::size(design).or_bug(Errno::ENOTRECOVERABLE);
        instances.extend_from_slice(&set::instance(
            at,
            [w, h],
            design::slot(design) as u32,
            anchor / h,
            f64::from(p.candidates.region_px[i]),
        ));
    }
    // Written, and a frame asked, only where what is drawn changed.
    if instances != p.instances {
        if section::map().is_some() {
            p.set.write(model::draw_list(), about, &instances);
        }
        ctx.invalidate();
    }
    let drawn_instances = core::mem::replace(&mut p.instances, instances);
    (p.sized, p.spare, p.spare_instances) = (sized, drawn_before, drawn_instances);
}

/// The markers the last step laid out, into the frame's list.
pub fn draw(draw: &mut DrawList, eye: V3) {
    ARRANGEMENT.get(Arrangement::default).set.draw(draw, eye);
}

/// What a tap on a scene marker hit: a place to go to; a pin, its entry's, which the tool held decides what to do with.
#[derive(Clone, Copy)]
pub enum MarkerHit {
    To(Q3),
    Pin(Octant, Q3),
}

/// `ndc` in [−1, 1], y up. The marker that took the tap, if any.
pub fn tap(ndc_x: f64, ndc_y: f64) -> Option<MarkerHit> {
    let [x, y] = ndc_to_px([ndc_x, ndc_y]);
    let hits = &ARRANGEMENT.get(Arrangement::default).hits;
    hits.iter()
        .find(|(r, _)| (r[0]..=r[2]).contains(&x) && (r[1]..=r[3]).contains(&y))
        .map(|&(_, went)| went)
}
