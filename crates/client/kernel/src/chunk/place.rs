// pxl-kernel/chunk/place.rs

//! Where places, markers and the chat's line are drawn: a place as Carto
//! draws it at the zoom of Carto's the view is at where it stands, read from
//! the tile that says its ground at that zoom; a marker someone put up, near
//! enough; a line said, over its speaker's head (`billboard/chat.rs`), judged
//! as a marker is. A place and a marker stand at a leaf cell's center on the
//! surface a ray down meets first.

mod selection;
use selection::select_visible_billboards;

use crate::billboard::badge::{self, Label, Look};
use crate::billboard::batch::BillboardBatch;
use crate::billboard::chat::{shown_line, speaker};
use crate::billboard::field::billboard_instance;
use crate::billboard::sheet::{new_billboard_set, place_billboard_set, remove_billboard_set, write_billboard_set};
use crate::chunk::Chunk;
use crate::layer::object::preview::draft;
use crate::layer::place::carto_zoom;
use crate::layer::Base;
use crate::layer::Query;
use crate::query::{Detail, Ray, World};
use crate::sched::tick::TickContext;
use crate::setting::store::settings_store;
use crate::view::demand::Observed as _;
use crate::view::frame::pose::ndc_to_px;
use core::hash::Hasher;
use pxl_render::gpu::sheet::shelf::NO_SLOT;
use pxl_spec::bake::constant::EntryAt;
use pxl_spec::block::object::{object_value_is_marker, MARKER_LATTICE};
use pxl_spec::space::coord::v3::V3;
use xpute_guest::global::Global;
use xpute_kit::status::bug::OrBug;
use xpute_kit::status::errno::Errno;

use fnv::FnvHashSet;
use pxl_spec::bake::tile::place::{stands, PlaceTile, PLACE_FIRST, PLACE_LATTICE, SPACING_CELLS};
use pxl_spec::space::camera::grid::px_per_unit;
use pxl_spec::space::coord::pvf3::pack;
use pxl_spec::space::coord::pvf3::pvf3::Pvf3;
use pxl_spec::space::coord::world::REACH;
use pxl_spec::space::coord::zoom::{base_of, cube, Zoom};
use pxl_spec::space::hierarchy::tree::{ancestor_of, first_descendant_of};
use pxl_spec::space::layer::lattice::GROUND;
use pxl_spec::space::layer::tile::{tile_at, tile_rows, LEAF_ZOOM};

#[derive(Default)]
struct Placement {
    batch: BillboardBatch,
    kept: Vec<u32>,
    set: Option<u32>,
    drawn: Vec<u32>,
    /// Nearest the eye first: screen box, label, and where it stands.
    hits: Vec<([f64; 4], Label, Pvf3)>,
    /// Kept so a settled turn asks the heap for nothing. Each with where it
    /// stands and Carto's zoom its chunk's cells are drawn at.
    found: Vec<(Label, Pvf3, i32)>,
    /// The drawn chunks `found` was read from: an eye move that changes them
    /// not, and nothing it reads moved, casts no ray.
    found_for: Option<Found>,
    /// The same, read this turn.
    now: Found,
    /// Places, ground or objects moved since `found` was read.
    moved: bool,
    mounted: Vec<(Chunk, bool, bool)>,
    labeled: Vec<(Label, Pvf3)>,
    /// Last turn's `labeled`, its buffer reused.
    spare: Vec<(Label, Pvf3)>,
    /// Markers and lines not ready yet, kept to become so.
    pending: Vec<Label>,
    instances: Vec<f32>,
}
static PLACEMENT: Global<Placement> = Global::new();

/// What `found` was read from: the drawn chunks, and whether places were on.
#[derive(Default, PartialEq)]
struct Found {
    drawn: Vec<Chunk>,
    places: bool,
}

pub fn moved() {
    PLACEMENT.get(Placement::default).moved = true;
}

/// The markers put up in `chunk`, by the chunk their base is in.
fn markers(chunk: Chunk, found: &mut Vec<(Label, Pvf3, i32)>) {
    let region = chunk.region();
    let objects = &crate::kernel().objects;
    let Some(cells) = objects.read(chunk.node(), chunk.cell_zoom()).value() else { return };
    cells.entries(&mut |e| {
        if !object_value_is_marker(e.state) {
            return;
        }
        let (label, stands, at) = marker_at(e.id(), objects.words(e).unwrap_or_default());
        if (0..3).all(|k| (region.min[k]..region.max[k]).contains(&at[k])) {
            found.push((label, stands, MARKER_LATTICE.tile_zoom_of_grid(chunk.cell_zoom())));
        }
    });
}

/// A marker at its cell's center, half a cell over the floor it stands on
/// (`Cell::center`), where the dot under its pill is drawn, and its base; its
/// words by their hash, so words changed under one entry are another badge.
fn marker_at(at: EntryAt, words: &str) -> (Label, Pvf3, V3) {
    let c = cube(at.zoom(), at.at());
    let base = [(c.min[0] + c.max[0]) / 2.0, (c.min[1] + c.max[1]) / 2.0, c.min[2]];
    let ground = GROUND.cell_of(LEAF_ZOOM, &base_of(at.zoom(), at.at()));
    let mut h = fnv::FnvHasher::default();
    h.write(words.as_bytes());
    let label = Label::Marker { at, words: h.finish() };
    (label, ground.center(base[2]), base)
}

/// None where nothing over it is known.
fn top_at(at: &Pvf3, zoom: Zoom) -> Option<f64> {
    let xy = pack::unpack(at.i, at.f);
    let (high, low) = (REACH.max[2], REACH.min[2]);
    let ray = Ray {
        origin: [xy[0], xy[1], high],
        direction: [0.0, 0.0, -1.0],
        reach: high - low,
    };
    let hit = World::now().first_hit(&ray, Detail::Zoom(zoom)).value().flatten()?;
    Some(high - hit.t)
}

/// Whether two labels show one thing, which the older shows while the newer
/// is not ready: a marker under one entry, and the chat's line.
fn one_thing(a: Label, b: Label) -> bool {
    match (a, b) {
        (Label::Marker { at, .. }, Label::Marker { at: other, .. }) => at == other,
        (Label::Chat { .. }, Label::Chat { .. }) => true,
        _ => false,
    }
}

/// The cell a place keeps to itself, as wide as the camera grid's cells are
/// at the view's own τ: its name wraps to it and its billboard's region is it.
/// Not the τ in force, which coarsens while the view moves and would lay
/// every name out again; whole pixels, so only the view's size or angle does.
fn spacing_px() -> f64 {
    let v = crate::view::frame::pose::view();
    let cell = crate::view::sight::cell_angle(crate::chunk::quality::target()) * px_per_unit(v.height, v.fov);
    libm::round(f64::from(SPACING_CELLS) * cell)
}

pub fn step(ctx: &mut TickContext) {
    let p = PLACEMENT.get(Placement::default);
    if crate::view::observation::eye().is_none() {
        if let Some(set) = p.set.take() {
            remove_billboard_set(set);
            ctx.invalidate();
        }
        p.hits.clear();
        badge::retain(&FnvHashSet::default());
        return;
    }
    let _tiles = PlaceTile::demand();
    if crate::chunk::domain::current().is_none() {
        return;
    }
    let sched = &crate::kernel().sched;
    sched.mounted(&mut p.mounted);
    p.now.drawn.clear();
    p.now.drawn.extend(p.mounted.iter().filter(|m| m.1).map(|m| m.0));
    p.now.drawn.sort_unstable();
    // Places are a setting; what someone placed is always shown.
    p.now.places = settings_store().place;
    let places = p.now.places;
    let stale = core::mem::take(&mut p.moved) | (p.found_for.as_ref() != Some(&p.now));
    if stale {
        let now = core::mem::take(&mut p.now);
        p.now = p.found_for.replace(now).unwrap_or_default();
    }
    let mut found = core::mem::take(&mut p.found);
    if stale {
        found.clear();
    }
    let read: &[(Chunk, bool, bool)] = if stale { &p.mounted } else { &[] };
    for &(chunk, _, _) in read.iter().filter(|m| m.1) {
        markers(chunk, &mut found);
        if places {
            places_in(chunk, &mut found);
        }
    }
    p.batch.reset();
    let drawn_before = core::mem::take(&mut p.labeled);
    let mut labeled = core::mem::take(&mut p.spare);
    labeled.clear();
    p.pending.clear();
    // A waiting marker stands over whatever stands at its place, with the words it has so far.
    let draft = draft();
    // A waiting marker is shown wherever it is.
    let drafted = draft.map(|d| marker_at(d.at, &d.words)).map(|(label, stands, _)| (label, stands, i32::MAX));
    // The line is its speaker's, judged where it stands, as a marker by the chunk drawing it there.
    let line = shown_line();
    let said = line.zip(speaker()).and_then(|(label, (feet, over))| Some((label, over, MARKER_LATTICE.tile_zoom_of_grid(sched.cells_at(&feet)?))));
    let is_drafted = |label: &Label| matches!(label, Label::Marker { at, .. } if Some(*at) == draft.map(|d| d.at));
    let spacing = spacing_px();
    // Laid out last, so a line wins a tie with a badge (the sort's tiebreak).
    for &(label, center, zoom) in drafted.iter().chain(found.iter().filter(|f| !is_drafted(&f.0))).chain(said.iter()) {
        let Some(label) = label.at(zoom, spacing) else { continue };
        // A marker's words or the line changed: what was drawn stays until the new one is ready to show.
        let label = match drawn_before.iter().find(|d| one_thing(d.0, label)) {
            Some(&(before, _)) if before != label && !badge::ready(label) => {
                p.pending.push(label);
                before
            }
            _ => label,
        };
        let Some([width, height, anchor]) = badge::size(label) else { continue };
        let region = if matches!(label, Label::Place { .. }) { spacing } else { width };
        let local = crate::view::frame::coord::to_world(&center);
        p.batch.push(local[0], local[1], local[2], [width, height, region], anchor, labeled.len() as f64);
        labeled.push((label, center));
    }
    // The line's slot is kept while it is said, wherever its speaker is.
    badge::retain(&labeled.iter().map(|l| l.0).chain(p.pending.iter().copied()).chain(line).collect());
    // Painted at render, which asks for no turn: one looks again.
    if !p.pending.is_empty() {
        crate::sched::edge::request_world_tick();
    }
    p.found = found;
    select_visible_billboards(&mut p.batch, &mut p.kept);
    p.drawn.clear();
    for &i in &p.kept {
        if badge::slot(labeled[i as usize].0) == NO_SLOT {
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
    // A line is no place to go to.
    p.hits.extend(p.drawn.iter().rev().map(|&i| (p.batch.screen[i as usize], labeled[i as usize].0, labeled[i as usize].1)).filter(|h| !matches!(h.1, Label::Chat { .. })));
    let mut instances = core::mem::take(&mut p.instances);
    instances.clear();
    // f32 instances, so about the sea under the eye rather than the world's origin.
    let about = crate::view::frame::pose::sea_under_eye();
    for &i in &p.drawn {
        let i = i as usize;
        let at = [p.batch.local_x[i] - about[0], p.batch.local_y[i] - about[1], p.batch.local_z[i] - about[2]];
        let [w, h, anchor] = badge::size(labeled[i].0).or_bug(Errno::ENOTRECOVERABLE);
        instances.extend_from_slice(&billboard_instance(at, [w, h], badge::slot(labeled[i].0) as u32, anchor / h, f64::from(p.batch.region_px[i])));
    }
    if p.set.is_some() || !instances.is_empty() {
        let set = *p.set.get_or_insert_with(new_billboard_set);
        place_billboard_set(set, about);
        write_billboard_set(set, &instances);
        ctx.invalidate();
    }
    (p.labeled, p.spare, p.instances) = (labeled, drawn_before, instances);
}

/// Where a tap on a billboard goes: to its place; a marker someone put up is
/// its entry's, which the tool held decides what to do with.
pub enum Went {
    To(Pvf3),
    Marker(EntryAt, Pvf3),
}

/// `ndc` in [−1, 1], y up. The badge that took the tap, if any.
pub fn tap(ndc_x: f64, ndc_y: f64) -> Option<Went> {
    let [x, y] = ndc_to_px([ndc_x, ndc_y]);
    let hits = &PLACEMENT.get(Placement::default).hits;
    let &(_, label, stands) = hits.iter().find(|(r, _, _)| (r[0]..=r[2]).contains(&x) && (r[1]..=r[3]).contains(&y))?;
    Some(match label {
        Label::Marker { at, .. } => Went::Marker(at, stands),
        Label::Place { .. } | Label::Chat { .. } => Went::To(stands),
    })
}

/// The places standing in `chunk`, read where its cells are tiles' cells,
/// each read from the tile that says it exactly; of them what Carto shows
/// at the zoom those cells are drawn at, as it shows them (`Label::at`).
fn places_in(chunk: Chunk, found: &mut Vec<(Label, Pvf3, i32)>) {
    let zoom = carto_zoom(chunk.cell_zoom());
    if zoom < i32::from(PLACE_FIRST) {
        return;
    }
    let t = PLACE_LATTICE.read_zoom(chunk.cell_zoom());
    let (z, column) = chunk.column();
    let tiles: Vec<_> = if t <= z {
        vec![tile_at(t, ancestor_of(column, u32::from(z - t)))]
    } else {
        let n = 1i64 << (t - z);
        let [x0, y0] = first_descendant_of(column, u32::from(t - z));
        (y0..y0 + n).filter(|y| tile_rows(t).contains(y)).flat_map(|y| (x0..x0 + n).map(move |x| tile_at(t, [x, y]))).collect()
    };
    let (h0, h1) = chunk.z_range();
    let held = PlaceTile::residency();
    for tile in tiles {
        let Some(src) = held.source(tile) else { continue };
        let Some(places) = held.get(src) else { continue };
        for (place, h) in places.iter() {
            if i32::from(h.from) > zoom {
                continue;
            }
            let ground = stands(src, h.at);
            if Chunk::holding(chunk.zoom(), &ground.mid()).column() != chunk.named().column() {
                continue;
            }
            // Counted only by the chunk the top of what stands there is in, so a
            // column's several chunks do not repeat it.
            let Some(top) = top_at(&ground.mid(), GROUND.read_zoom(chunk.cell_zoom())) else { continue };
            if top - 1.0 < h0 || top - 1.0 >= h1 {
                continue;
            }
            found.push((
                Label::Place {
                    tile: src,
                    place,
                    look: Look::default(),
                    wrap: 0,
                },
                ground.center(top),
                zoom,
            ));
        }
    }
}
