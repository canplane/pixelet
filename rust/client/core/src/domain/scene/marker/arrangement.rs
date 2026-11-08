// pxl-core/domain/scene/marker/arrangement.rs

//! The markers' arrangement: where each marker is drawn. Each source reads
//! its markers over the bricks of the eye's clip levels, the bricks decor is
//! generated over (domain/view/observation.rs `bricks`). It shows each
//! marker at the zoom of the camera grid's cell at the marker's own point. A
//! place shows as Carto draws it at that zoom (`place.rs`). A pin someone put
//! up shows when it is near enough (`pin.rs`). A line said hangs over its
//! speaker's head while its speaker shows it (`line.rs`). A brick is only
//! what a source is read over. Each source hands the arrangement an
//! `Instance`, and the arrangement asks the same of every instance: its
//! design, its size, its slot and whether it is ready (`design.rs`), and
//! where a tap on it goes.

use super::selection::select_visible;
use super::{line, pin, place};

use crate::domain::preview::Draft;
use crate::domain::query::{at_zoom, Ray, World};
use crate::domain::scene::marker::candidate::Candidates;
use crate::domain::scene::marker::design::{self, Design};
use crate::domain::scene::marker::set::{self, Set};
use crate::domain::setting::store::settings;
use crate::domain::view::frame::screen::{ndc_to_px, units_per_px_at};
use crate::sys::sched::tick::TickContext;
use pxl_render::frame::shelf::NO_SLOT;
use pxl_spec::space::coord::v3;
use pxl_spec::space::layer::octant::Octant;
use xpute_core::global::Global;
use xpute_kit::status::bug::OrBug;
use xpute_kit::status::errno::Errno;

use crate::domain::query::changes;
use crate::domain::scene::frame::Frame;
use crate::domain::scene::marker::param::SPACING_PX;
use crate::domain::scene::model;
use crate::domain::view::frame::{coord, pose};
use crate::domain::view::observation;
use crate::sys::sched::edge;
use fnv::{FnvHashMap, FnvHashSet};
use pxl_engine::clipmap::BRICK_SHIFT;
use pxl_render::gpu::draw::DrawList;
use pxl_spec::space::camera::eye::Observation;
use pxl_spec::space::coord::aabb::Aabb;
use pxl_spec::space::coord::q3::{self, Q3};
use pxl_spec::space::coord::world::BOUNDS;
use pxl_spec::space::coord::zoom::{units, Zoom};
use pxl_spec::space::hierarchy::tree::Node;
use pxl_spec::space::layer::quadrant::Quadrant;

/// A brick of the eye's clip levels that markers are read over, named by its
/// zoom and place. Its X runs on round the world without wrapping, as the
/// view's frame does (domain/view/observation.rs `bricks`).
pub(super) type Brick = Node<2>;

/// The zoom of a brick's cells: the camera grid's zoom at the brick's
/// nearest point, the finest the brick draws.
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
    /// Each drawn marker's screen box and where a tap on it goes, nearest
    /// the eye first.
    hits: Vec<([f64; 4], MarkerHit)>,
    /// The pins and places read so far, a brick's together, as each source
    /// reads them (`pin.rs`, `place.rs`). These buffers, like the others
    /// here, are kept between turns so a turn where nothing changed
    /// allocates nothing.
    pins: Vec<pin::Pin>,
    places: Vec<place::Place>,
    /// What the sources show this run.
    shown: Vec<Instance>,
    /// The drawn bricks `pins` and `places` were read from. A brick still
    /// drawn keeps what was read from it, so an eye that moves casts rays
    /// only in the bricks it enters.
    found_for: Option<Read>,
    /// The same, for the bricks read this turn.
    now: Read,
    /// Whether places, ground or objects changed since they were read.
    changed: bool,
    /// The place tiles whose answer changed, besides the ground's changes
    /// (domain/query/changes.rs). A brick they touch is read again.
    changed_places: Vec<Quadrant>,
    /// The bricks this turn would read, with their distance from the eye and
    /// their columns.
    entering: Vec<(f64, Brick, Aabb)>,
    sized: Vec<Instance>,
    /// Last turn's `sized`, whose buffer is reused.
    spare: Vec<Instance>,
    /// The designs not ready yet, each with the index in `sized` of the
    /// instance that shows its old drawing until it is.
    pending: Vec<(u32, Design)>,
    /// What was drawn last, by its holder's key, and the designs the markers
    /// keep. Both are rebuilt each run.
    before: FnvHashMap<Key, Design>,
    keep: FnvHashSet<Design>,
    /// What the marker set holds, and the buffer the next run lays out
    /// into. They are swapped as `sized` and `spare` are.
    instances: Vec<u32>,
    spare_instances: Vec<u32>,
    /// What the last run read, compared by value. While it is equal, nothing
    /// changed and nothing is left waiting, the last layout stands and the
    /// step does nothing.
    last: Option<Inputs>,
    /// Whether the last run left a design not laid out or not painted, which
    /// a later turn brings, or a brick not read yet. If so, the next turn
    /// runs again.
    waiting: bool,
}

/// Everything `step` reads, compared by value, as the demand's `Key` is: the
/// eye and its view (the observation holds its frustum), the places setting,
/// the pin being written, and the chat's line with where it hangs. An input
/// the step reads must be added here, or the step never sees it change.
#[derive(Clone, PartialEq)]
struct Inputs {
    o: Observation,
    places: bool,
    drafted: Option<(Octant, u64)>,
    bubble: line::Bubble,
}

/// The name a holder gives what it shows, stable from one run to the next
/// and independent of what is painted (`design.rs`). A pin is named by its
/// holder (`pin::Holder`), a place by its tile and its index there, and the
/// chat's line by its speaker, of whom there is one. The arrangement uses the
/// key only to tell instances apart, so it can keep an instance's old drawing
/// until its new one is painted. Where a tap goes is the instance's own
/// (`Instance::tap`).
#[derive(Clone, Copy, PartialEq, Eq, Hash)]
pub(super) enum Key {
    Pin(pin::Holder),
    Place(Quadrant, u32),
    Line,
}

/// What a source hands the arrangement: its holder's key, the design it
/// paints, where it stands, and where a tap on it goes, or None where it
/// takes no tap.
#[derive(Clone)]
pub(super) struct Instance {
    pub(super) key: Key,
    pub(super) design: Design,
    pub(super) stands: Q3,
    pub(super) tap: Option<MarkerHit>,
}

static ARRANGEMENT: Global<Arrangement> = Global::new();

/// What `pins` and `places` were read from: the drawn bricks, and whether
/// places were on. A brick is read over its whole columns, so what was read
/// stays valid while no change touches those columns, whatever heights the
/// world spans in them.
#[derive(Default, PartialEq)]
struct Read {
    bricks: Vec<Brick>,
    places: bool,
}

/// The farthest a marker is drawn past where it stands, in pixels, which the
/// cull of a brick must leave room for. The bound is twice the width a place
/// keeps and its name wraps to (`SPACING_PX`), which is more than a pin's
/// longest pill or a place's name lines over its icon.
fn extent_px(spacing: f64) -> f64 {
    2.0 * spacing
}

pub fn on_changed(places: &[Quadrant]) {
    let p = ARRANGEMENT.get(Arrangement::default);
    p.changed = true;
    p.changed_places.extend_from_slice(places);
}

/// The height at which a ray down over `at`, from `high`, first meets what is
/// drawn at `zoom` above `low`, or None where nothing there is known.
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

/// Lays out the markers for `frame`. `draft` is the marker tool's draft as
/// the turn hands it (turn.rs `scene_tick`).
pub fn step(ctx: &mut TickContext, frame: &Frame, draft: Option<&Draft>) {
    let p = ARRANGEMENT.get(Arrangement::default);
    if pose::eye().is_none() {
        if p.set.free(model::draw_list()) {
            ctx.invalidate();
        }
        // Nothing is drawn now, so what comes back is laid out afresh.
        (p.last, p.waiting, p.found_for) = (None, false, None);
        p.changed_places.clear();
        p.instances.clear();
        p.hits.clear();
        design::retain(&FnvHashSet::default());
        return;
    }
    // With no observation yet, the source's reach is not declared. The
    // markers are laid out afresh once there is one, so the tiles that
    // changed meanwhile are not kept.
    let Some(o) = observation::observation() else {
        (p.last, p.found_for) = (None, None);
        p.changed_places.clear();
        return;
    };
    let seen = Inputs {
        o,
        places: settings().place,
        drafted: pin::draft_key(draft),
        bubble: line::bubble(o.eye),
    };
    if !p.waiting && !p.changed && p.last.as_ref() == Some(&seen) {
        return;
    }
    p.last = Some(seen.clone());
    // The bricks of the eye's clip levels that are in view
    // (domain/view/observation.rs `bricks`), as decor's are. Each is tested
    // as the box the world stands in over it, grown by how far a marker
    // draws past it at the box's far side. No brick past where the rays end
    // is taken, since there the sky's Earth stands for the ground.
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
    // Places are a setting, while what someone placed always shows.
    now.places = settings().place;
    let places = now.places;
    // What was read stays for a brick that is still drawn and whose columns
    // no change touches (domain/query/changes.rs), as long as the places
    // setting did not turn. A brick left or touched drops what was read from
    // it. Only bricks entered or touched are read, nearest the eye first, as
    // many a turn as one pass of the tick's budget reads and at least one.
    // The rest are not read yet, so a later turn reads them.
    p.changed = false;
    let before = match &p.found_for {
        Some(f) if f.places == places => f.bricks.as_slice(),
        _ => &[],
    };
    let changed_places = &p.changed_places;
    let columns = |s: Brick| region(s, [BOUNDS.min[2], BOUNDS.max[2]]);
    // Whether each drawn brick keeps what was read from it, asked once for
    // each brick however many pins and places it holds.
    let stood: Vec<bool> = now
        .bricks
        .iter()
        .map(|&s| {
            let r = columns(s);
            before.binary_search(&s).is_ok()
                && !changes::within([[r.min[0], r.min[1]], [r.max[0], r.max[1]]], changed_places)
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
    // The bricks not read yet are not among those the markers were read
    // from.
    let later = &mut entering[read..];
    later.sort_unstable_by_key(|e| e.1);
    now.bricks
        .retain(|d| later.binary_search_by_key(d, |e| e.1).is_err());
    p.changed_places.clear();
    if p.found_for.as_ref() != Some(&p.now) {
        let now = core::mem::take(&mut p.now);
        p.now = p.found_for.replace(now).unwrap_or_default();
    }
    p.candidates.reset();
    let drawn_before = core::mem::take(&mut p.sized);
    let mut sized = core::mem::take(&mut p.spare);
    sized.clear();
    p.pending.clear();
    // Each marker shows at the zoom of the grid's cell at its own point.
    let mut shown = core::mem::take(&mut p.shown);
    shown.clear();
    pin::show(&mut pins, o.eye, draft, &mut shown);
    place::show(&mut read_places, o.eye, &mut shown);
    shown.extend(seen.bubble.instance());
    let spacing = SPACING_PX;
    // What was drawn last, by its holder's key. When what a holder shows
    // changes, the old drawing stays until the new one is ready to show.
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
        // The design's text is not laid out yet. A later turn lays it out.
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
    // A held design of a visible instance asks for its slot now, after the
    // dropped designs' slots are freed, so the render paints it. Where the
    // atlas has no room for it, the hold is given up: the instance shows the
    // new design, which waits, and the next run frees the old design's slot
    // and asks again. A design whose text is not laid out yet stays held and
    // asks nothing. It asks on the turn its text arrives (`design::coming`).
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
    // Neither the render's painting nor the client's layout asks for a turn,
    // so the arrangement asks for one itself: while a design is on its way
    // (`design::coming`), when a freed slot may make room for a design that
    // had none, or when a hold given up frees the old slot.
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
    // Each instance holds its stand as Q3, with nothing of the eye, so the
    // instances change only when the selection, the order or a design
    // changes.
    for &i in &p.draw_order {
        let i = i as usize;
        let design = &sized[i].design;
        let [w, h, anchor] = design::size(design).or_bug(Errno::ENOTRECOVERABLE);
        instances.extend_from_slice(&set::instance(
            sized[i].stands,
            [w, h],
            design::slot(design) as u32,
            anchor / h,
            f64::from(p.candidates.region_px[i]),
        ));
    }
    // The set is written, and a frame asked for, only when what is drawn
    // changed.
    if instances != p.instances {
        p.set.write(model::draw_list(), &instances);
        ctx.invalidate();
    }
    let drawn_instances = core::mem::replace(&mut p.instances, instances);
    (p.sized, p.spare, p.spare_instances) = (sized, drawn_before, drawn_instances);
}

pub fn draw(draw: &mut DrawList) {
    ARRANGEMENT.get(Arrangement::default).set.draw(draw);
}

/// What a tap on a scene marker hit: a place to go to, or a pin's entry,
/// which the tool held decides what to do with.
#[derive(Clone, Copy)]
pub enum MarkerHit {
    To(Q3),
    Pin(Octant, Q3),
}

/// The marker that took a tap at `ndc_x`, `ndc_y`, in [−1, 1] with y up, or
/// None.
pub fn tap(ndc_x: f64, ndc_y: f64) -> Option<MarkerHit> {
    let [x, y] = ndc_to_px([ndc_x, ndc_y]);
    let hits = &ARRANGEMENT.get(Arrangement::default).hits;
    hits.iter()
        .find(|(r, _)| (r[0]..=r[2]).contains(&x) && (r[1]..=r[3]).contains(&y))
        .map(|&(_, went)| went)
}
