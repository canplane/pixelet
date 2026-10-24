// pxl-kernel/scene/place.rs

//! Where places, markers and the chat's line are drawn: a place as Carto
//! draws it at the zoom of Carto's the view is at where it stands, read from
//! the tile that says its ground at that zoom; a marker someone put up, near
//! enough, or where more than one stands in a marker's cell (`marker_cell`),
//! their count at the cell's middle; a line said, over its speaker's head
//! (`billboard/chat.rs`), judged as a marker is. A place stands at its ground
//! leaf cell's center on the surface a ray down meets first, a marker at its
//! own cell's.

mod selection;
use selection::select_visible_billboards;

use crate::billboard::badge::{self, Label};
use crate::billboard::batch::BillboardBatch;
use crate::billboard::chat::{shown_line, speaker};
use crate::billboard::field::{billboard_instance, BillboardSet};
use crate::chunk::Chunk;
use crate::layer::object::preview::draft;
use crate::layer::place::carto_zoom;
use crate::query::{at_zoom, Ray, World};
use crate::sched::tick::TickContext;
use crate::setting::store::settings_store;
use crate::view::frame::pose::ndc_to_px;
use pxl_render::gpu::sheet::shelf::NO_SLOT;
use pxl_spec::bake::constant::EntryAt;
use pxl_spec::block::object::{marker_cell_of, marker_shows, object_value_is_marker};
use pxl_spec::space::coord::v3::{self, V3};
use xpute_guest::global::Global;
use xpute_kit::status::bug::OrBug;
use xpute_kit::status::errno::Errno;

use fnv::{FnvHashMap, FnvHashSet};
use pxl_spec::bake::place_theme::{DOT_ZOOM, FILL, ICON_ZOOM, LABEL_ZOOM, TEXT};
use pxl_spec::bake::tile::place::{stands, symbol_of, Placed, PLACE_FIRST, PLACE_LATTICE, SPACING_CELLS};
use pxl_spec::space::camera::grid::px_per_unit;
use pxl_spec::space::coord::q3::{self, Q3};
use pxl_spec::space::coord::world::{point, REACH};
use pxl_spec::space::coord::zoom::{cube, units, Zoom};
use pxl_spec::space::hierarchy::tree::{ancestor_of, first_descendant_of};
use pxl_spec::space::layer::lattice::GROUND;
use pxl_spec::space::layer::tile::{tile_at, tile_rows, Tile};

#[derive(Default)]
struct Placement {
    batch: BillboardBatch,
    kept: Vec<u32>,
    set: BillboardSet,
    drawn: Vec<u32>,
    /// Nearest the eye first: screen box, and where a tap on it goes.
    hits: Vec<([f64; 4], Went)>,
    /// Kept so a settled turn asks the heap for nothing; a chunk's in a
    /// run, its markers in a run a cell.
    found: Vec<Shown>,
    /// The drawn chunks `found` was read from: a chunk drawn still keeps
    /// what was read from it, so an eye move casts rays only in the chunks it
    /// enters.
    found_for: Option<Found>,
    /// The same, read this turn.
    now: Found,
    /// Places, ground or objects moved since `found` was read.
    moved: bool,
    /// The place tiles whose answer moved, beside the ground's
    /// (query/changed.rs): a chunk they touch is read again.
    moved_places: Vec<Tile>,
    /// The chunks this turn would read, by how far from the eye.
    entering: Vec<(f64, Chunk)>,
    labeled: Vec<(Key, Label, Q3)>,
    /// Last turn's `labeled`, its buffer reused.
    spare: Vec<(Key, Label, Q3)>,
    /// Labels not ready yet, kept to become so.
    pending: Vec<Label>,
    /// What was drawn last by its holder's key, and the labels the badges
    /// keep: made each run, their room kept.
    before: FnvHashMap<Key, Label>,
    keep: FnvHashSet<Label>,
    /// What the billboard set holds, and the buffer the next run lays out
    /// into, swapped as `labeled` and `spare` are.
    instances: Vec<f32>,
    spare_instances: Vec<f32>,
    /// What the last run read, compared by value: equal, with nothing moved
    /// and nothing left waiting, the labels it laid out stand.
    last: Option<Seen>,
    /// The last run left a label not laid out or not painted, which a later
    /// turn has, or a chunk not read yet: the next runs again.
    waiting: bool,
}

/// Everything `step` reads, by value, as the demand's `Seen` is: the eye and
/// its view (the observation holds its frustum), the grid's τ, the places
/// setting, the spacing names wrap to, the marker being written and the
/// chat's line and its speaker. A new input goes here or the step never sees
/// it change.
#[derive(Clone, PartialEq)]
struct Seen {
    o: pxl_spec::space::camera::eye::Observation,
    tau: pxl_spec::space::camera::grid::Tau,
    places: bool,
    spacing: f64,
    drafted: Option<(EntryAt, Label, Q3)>,
    line: Option<Label>,
    speaker: Option<(Q3, Q3)>,
}

/// The name a holder calls what it shows by, from one run to the next, and
/// none of what is painted (`badge.rs`): a marker by its entry, the markers
/// of a marker's cell by the cell, a place by its tile and its index there,
/// the chat's line by its speaker, who is one.
#[derive(Clone, Copy, PartialEq, Eq, Hash)]
enum Key {
    Pin(EntryAt),
    Cell((Zoom, [i32; 3])),
    Place(Tile, u32),
    Line,
}

impl Key {
    /// Where a tap on what it shows, standing at `stands`, goes: a marker
    /// to its entry, anything else but a line to where it stands.
    fn went(self, stands: Q3) -> Option<Went> {
        match self {
            Key::Pin(at) => Some(Went::Marker(at, stands)),
            Key::Cell(_) | Key::Place(..) => Some(Went::To(stands)),
            // A line is no place to go to.
            Key::Line => None,
        }
    }
}

static PLACEMENT: Global<Placement> = Global::new();

/// What a drawn chunk was read to show: by its holder's key, its label, and
/// where it stands; a marker with its cell.
#[derive(Clone)]
struct Shown {
    chunk: Chunk,
    key: Key,
    label: Label,
    stands: Q3,
    crowd: Option<Crowd>,
}

impl Shown {
    fn cell(&self) -> Option<(Zoom, [i32; 3])> {
        self.crowd.as_ref().map(|c| c.cell)
    }
}

/// A marker's cell (`marker_cell`) and, where it holds more than one, where
/// their count stands and the count.
#[derive(Clone)]
struct Crowd {
    cell: (Zoom, [i32; 3]),
    count: Option<(Q3, Label)>,
}

/// What `found` was read from: the drawn chunks, and whether places were on.
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

/// The markers put up in `chunk`, by the chunk their base is in, a run a
/// marker's cell: a cell nests in its chunk, so no run is another chunk's too.
fn markers(chunk: Chunk, found: &mut Vec<Shown>) {
    let cell_zoom = chunk.cell_zoom();
    if !marker_shows(cell_zoom) {
        return;
    }
    let region = chunk.region();
    let world = World::now();
    let from = found.len();
    world.placed_in(&region, &mut |e| {
        if !object_value_is_marker(e.state) {
            return;
        }
        let (stands, at) = marker_at(e.id());
        if (0..3).all(|k| (region.min[k]..region.max[k]).contains(&at[k])) {
            let cell = marker_cell_of(cell_zoom, e.id());
            found.push(Shown {
                chunk,
                key: Key::Pin(e.id()),
                label: Label::pill(world.words(e).unwrap_or_default()),
                stands,
                crowd: Some(Crowd { cell, count: None }),
            });
        }
    });
    let detail = GROUND.read_zoom(cell_zoom);
    let mine = &mut found[from..];
    mine.sort_unstable_by_key(Shown::cell);
    for run in mine.chunk_by_mut(|a, b| a.cell() == b.cell()).filter(|run| run.len() > 1) {
        let (Some((z, at)), Key::Pin(one)) = (run[0].cell(), run[0].key) else {
            continue;
        };
        let count = Some((crowd_at(z, at, one.zoom(), detail), Label::count(run.len())));
        run.iter_mut().filter_map(|s| s.crowd.as_mut()).for_each(|c| c.count.clone_from(&count));
    }
}

/// Where the count of a cell's markers stands: at its middle, half a marker
/// at `marker` over what is drawn there first within it, or over its floor.
fn crowd_at(zoom: Zoom, at: [i32; 3], marker: Zoom, detail: Zoom) -> Q3 {
    let c = cube(zoom, at);
    let [x, y, _] = c.center();
    let top = surface(&point(x, y, c.min[2]), (c.max[2], c.min[2]), detail).unwrap_or(c.min[2]);
    point(x, y, top + units(marker) / 2.0)
}

/// A marker at its own cell's center, half a cell over the floor it stands
/// on, where the dot under its pill is drawn, and its base.
fn marker_at(at: EntryAt) -> (Q3, V3) {
    let c = cube(at.zoom(), at.at());
    let [x, y, z] = c.center();
    (point(x, y, z), [x, y, c.min[2]])
}

/// The height a ray down over `at` from `high` first meets what is drawn,
/// above `low`; none where nothing there is known.
fn surface(at: &Q3, (high, low): (f64, f64), zoom: Zoom) -> Option<f64> {
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
/// its billboard's region is it. Not the τ in force, which coarsens while
/// the view moves and would lay every name out again; whole pixels, so only
/// the view's size or angle does.
fn spacing_px() -> f64 {
    let v = crate::view::frame::pose::view();
    let drawn = v.height * crate::view::frame::resolution::display_ratio();
    let cell = crate::view::lens::cell_angle(crate::view::lens::tau()) * px_per_unit(drawn, crate::param::FOV);
    libm::rint(f64::from(SPACING_CELLS) * cell)
}

pub fn step(ctx: &mut TickContext, frame: &crate::scene::frame::Frame) {
    let p = PLACEMENT.get(Placement::default);
    if crate::view::observation::eye().is_none() {
        if p.set.free(&mut crate::kernel().draw) {
            ctx.invalidate();
        }
        // Nothing is drawn now: what comes back is laid out afresh.
        (p.last, p.waiting, p.found_for) = (None, false, None);
        p.moved_places.clear();
        p.instances.clear();
        p.hits.clear();
        badge::retain(&FnvHashSet::default());
        return;
    }
    // No observation yet, the source's reach not declared: laid out afresh
    // once there is one, so the tiles that moved meanwhile are not kept.
    let Some(o) = crate::view::observation::observation() else {
        (p.last, p.found_for) = (None, None);
        p.moved_places.clear();
        return;
    };
    let tau = crate::view::lens::tau();
    let draft = draft();
    // A waiting marker is shown wherever it is.
    let drafted = draft.map(|d| (d.at, Label::pill(&d.words), marker_at(d.at).0));
    let seen = Seen {
        o,
        tau,
        places: settings_store().place,
        spacing: spacing_px(),
        drafted,
        line: shown_line(),
        speaker: speaker(),
    };
    if !p.waiting && !p.moved && p.last.as_ref() == Some(&seen) {
        return;
    }
    p.last = Some(seen.clone());
    // The eye's cubes in view (view/observation.rs `cubes`), as decor's are.
    let now = &mut p.now;
    now.drawn.clear();
    crate::view::observation::cubes(&o, tau, &mut |(zoom, at)| {
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
    // touches (query/changed.rs), while places did not turn: a chunk left or
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
        before.binary_search(c).is_ok() && !crate::query::changed::within([[r.min[0], r.min[1]], [r.max[0], r.max[1]]], moved_places)
    };
    let mut found = core::mem::take(&mut p.found);
    found.retain(|s| now.drawn.binary_search(&s.chunk).is_ok() && stands(&s.chunk));
    let entering = &mut p.entering;
    entering.clear();
    entering.extend(now.drawn.iter().filter(|c| !stands(c)).map(|&c| (v3::len(&v3::sub(&c.region().center(), &o.eye)), c)));
    entering.sort_unstable_by(|a, b| a.0.total_cmp(&b.0));
    let read = ctx
        .budget
        .run(
            entering.iter(),
            |&(_, chunk)| {
                markers(chunk, &mut found);
                if places {
                    places_in(chunk, &mut found);
                }
                true
            },
            None,
        )
        .visited as usize;
    let unread = entering.len() - read;
    // Not read, so not what `found` was read from.
    let later = &mut entering[read..];
    later.sort_unstable_by_key(|e| e.1);
    now.drawn.retain(|c| later.binary_search_by_key(c, |e| e.1).is_err());
    p.moved_places.clear();
    if p.found_for.as_ref() != Some(&p.now) {
        let now = core::mem::take(&mut p.now);
        p.now = p.found_for.replace(now).unwrap_or_default();
    }
    p.batch.reset();
    let drawn_before = core::mem::take(&mut p.labeled);
    let mut labeled = core::mem::take(&mut p.spare);
    labeled.clear();
    p.pending.clear();
    // The line is its speaker's, judged where it stands, as a marker by the chunk drawing it there.
    let line = seen.line.clone();
    let said = line
        .clone()
        .zip(seen.speaker)
        .filter(|&(_, (feet, _))| marker_shows(crate::query::cell_at(o.eye, tau, crate::view::frame::coord::to_world(&feet))))
        .map(|(label, (_, over))| (Key::Line, label, over));
    let is_drafted = |key: &Key| matches!(key, Key::Pin(at) if Some(*at) == draft.map(|d| d.at));
    // A cell's markers but the waiting one: one is itself, more their count.
    let shown = found.chunk_by(|a, b| a.cell().is_some() && a.cell() == b.cell()).filter_map(|run| {
        let mut left = run.iter().filter(|s| !is_drafted(&s.key));
        let first = left.next()?;
        let n = 1 + left.count();
        Some(match &first.crowd {
            Some(Crowd { cell, count: Some((at, count)) }) if n > 1 => (Key::Cell(*cell), if n == run.len() { count.clone() } else { Label::count(n) }, *at),
            _ => (first.key, first.label.clone(), first.stands),
        })
    });
    let spacing = seen.spacing;
    // What was drawn last by its holder's key, read once: a marker's words,
    // a count, a place's look or wrap or the line changed, and what was
    // drawn stays until the new one is ready to show.
    let before = &mut p.before;
    before.clear();
    before.extend(drawn_before.iter().map(|(k, l, _)| (*k, l.clone())));
    let mut unlaid = false;
    let drafted = seen.drafted.clone().map(|(at, label, stands)| (Key::Pin(at), label, stands));
    // Laid out last, so a line wins a tie with a badge (the sort's tiebreak).
    for (key, label, center) in drafted.into_iter().chain(shown).chain(said) {
        let (label, region) = label.spaced(spacing);
        let label = match before.get(&key) {
            Some(before) if *before != label && !badge::ready(&label) => {
                p.pending.push(label);
                before.clone()
            }
            _ => label,
        };
        // Its words are laid out by a later turn.
        let Some([width, height, anchor]) = badge::size(&label) else {
            unlaid = true;
            continue;
        };
        let local = crate::view::frame::coord::to_world(&center);
        p.batch.push(local[0], local[1], local[2], [width, height, region], anchor, labeled.len() as f64);
        labeled.push((key, label, center));
    }
    // The line's slot is kept while it is said, wherever its speaker is.
    p.keep.clear();
    p.keep.extend(labeled.iter().map(|l| l.1.clone()).chain(p.pending.iter().cloned()).chain(line));
    let freed = badge::retain(&p.keep);
    // Painted at render, or laid out by the host, neither of which asks a
    // turn: one looks again, while a label is on its way (`badge::coming`),
    // or once a slot let go may make room for one that had none.
    let pending = p.pending.iter().any(badge::coming) || freed && !p.pending.is_empty();
    p.waiting = pending || unlaid || unread > 0;
    if p.waiting {
        crate::sched::edge::request_world_tick();
    }
    p.found = found;
    select_visible_billboards(&mut p.batch, &mut p.kept);
    p.drawn.clear();
    for &i in &p.kept {
        if badge::slot(&labeled[i as usize].1) == NO_SLOT {
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
    p.hits.extend(
        p.drawn
            .iter()
            .rev()
            .filter_map(|&i| Some((p.batch.screen[i as usize], labeled[i as usize].0.went(labeled[i as usize].2)?))),
    );
    let mut instances = core::mem::take(&mut p.spare_instances);
    instances.clear();
    // f32 instances, so about the sea under the eye rather than the world's origin.
    let about = crate::view::frame::pose::sea_under_eye();
    for &i in &p.drawn {
        let i = i as usize;
        let at = [p.batch.local_x[i] - about[0], p.batch.local_y[i] - about[1], p.batch.local_z[i] - about[2]];
        let label = &labeled[i].1;
        let [w, h, anchor] = badge::size(label).or_bug(Errno::ENOTRECOVERABLE);
        instances.extend_from_slice(&billboard_instance(at, [w, h], badge::slot(label) as u32, anchor / h, f64::from(p.batch.region_px[i])));
    }
    // Written, and a frame asked, only where what is drawn changed.
    if instances != p.instances {
        if let Some(k) = crate::laid() {
            p.set.write(&mut k.draw, about, &instances);
        }
        ctx.invalidate();
    }
    let drawn_instances = core::mem::replace(&mut p.instances, instances);
    (p.labeled, p.spare, p.spare_instances) = (labeled, drawn_before, drawn_instances);
}

/// The billboards the last step laid out, into the frame's list.
pub fn draw(draw: &mut pxl_render::frame::draw::DrawList, eye: V3) {
    PLACEMENT.get(Placement::default).set.draw(draw, eye);
}

/// Where a tap on a billboard goes: to its place; a marker someone put up is
/// its entry's, which the tool held decides what to do with.
#[derive(Clone, Copy)]
pub enum Went {
    To(Q3),
    Marker(EntryAt, Q3),
}

/// `ndc` in [−1, 1], y up. The badge that took the tap, if any.
pub fn tap(ndc_x: f64, ndc_y: f64) -> Option<Went> {
    let [x, y] = ndc_to_px([ndc_x, ndc_y]);
    let hits = &PLACEMENT.get(Placement::default).hits;
    hits.iter().find(|(r, _)| (r[0]..=r[2]).contains(&x) && (r[1]..=r[3]).contains(&y)).map(|&(_, went)| went)
}

/// The places standing in `chunk`, read where its cells are tiles' cells,
/// each read from the tile that says it exactly; of them what Carto shows
/// at the zoom those cells are drawn at, as it shows them (`point_of`).
fn places_in(chunk: Chunk, found: &mut Vec<Shown>) {
    let zoom = carto_zoom(chunk.cell_zoom());
    if zoom < PLACE_FIRST as i32 {
        return;
    }
    let t = PLACE_LATTICE.read_zoom(chunk.cell_zoom());
    let (z, column) = chunk.column();
    // The tile over the column, or its tiles under it: `n` a side from the first.
    let ([x0, y0], n) = if t <= z {
        (ancestor_of(column, z - t), 1)
    } else {
        (first_descendant_of(column, t - z), 1 << (t - z))
    };
    let tiles = (y0..y0 + n).filter(|y| tile_rows(t).contains(y)).flat_map(|y| (x0..x0 + n).map(move |x| tile_at(t, [x, y])));
    let (h0, h1) = chunk.z_range();
    let detail = GROUND.read_zoom(chunk.cell_zoom());
    World::now().places_in(tiles, &mut |src, place, h| {
        let Some(label) = point_of(&h, zoom) else { return };
        let ground = stands(src, h.at);
        if Chunk::holding(chunk.zoom(), &ground.mid()).column() != chunk.named().column() {
            return;
        }
        // Counted only by the chunk the top of what stands there is in, so a
        // column's several chunks do not repeat it.
        let Some(top) = surface(&ground.mid(), (REACH.max[2], REACH.min[2]), detail) else { return };
        if top - 1.0 < h0 || top - 1.0 >= h1 {
            return;
        }
        found.push(Shown {
            chunk,
            key: Key::Place(src, place),
            label,
            stands: ground.center(top),
            crowd: None,
        });
    });
}

/// A place as Carto shows it where its zoom is `zoom`, if it does: from the
/// zoom it is shown from, as a dot or its icon by its category's zooms,
/// named from its category's label zoom, its name wrapped where it is laid
/// out (`Label::spaced`).
fn point_of(h: &Placed, zoom: i32) -> Option<Label> {
    let k = usize::from(h.place.category.checked_sub(1)?);
    let from = |z: u8| z != 0 && zoom >= i32::from(z);
    let (icon, dot) = (from(ICON_ZOOM[k]), from(DOT_ZOOM[k]));
    if zoom < h.from as i32 || !(icon || dot) {
        return None;
    }
    let named = from(LABEL_ZOOM[k]) && !h.place.name.is_empty();
    Some(Label::Point {
        name: named.then(|| h.place.name.into()),
        wrap: 0,
        icon,
        symbol: symbol_of(h.place.category),
        fill: FILL[k],
        text: TEXT[k],
    })
}
