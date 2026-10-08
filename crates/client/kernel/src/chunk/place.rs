// pxl-kernel/chunk/place.rs

//! Where the places counts are drawn: one badge per place-lattice cell under each
//! drawn chunk, read from the tile that settles that ground,
//! standing at a leaf cell's center on the surface a ray down meets first.

pub mod badge;
mod selection;
use selection::select_visible_billboards;

use crate::billboard::batch::BillboardBatch;
use crate::billboard::field::billboard_instance;
use crate::billboard::sheet::{new_billboard_set, place_billboard_set, remove_billboard_set, write_billboard_set, BillboardOrder};
use crate::chunk::Chunk;
use crate::layer::object::preview::draft;
use crate::layer::place::{rank_of, Places, ALL_UNITS_PER_PX, MAJOR_UNITS_PER_PX};
use crate::layer::Layer;
use crate::layer::{Answer, Query};
use crate::param::PLACE_MAJOR_RANK;
use crate::query::{Detail, Ray, World};
use crate::sched::tick::TickContext;
use crate::setting::store::settings_store;
use crate::view::demand::Observed as _;
use crate::view::frame::pose::ndc_to_px;
use crate::view::frame::pose::units_per_px_at;
use badge::Label;
use core::hash::Hasher;
use pxl_render::gpu::sheet::shelf::NO_SLOT;
use pxl_spec::bake::constant::EntryAt;
use pxl_spec::cell::object::object_value_is_marker;
use pxl_spec::space::coord::v3::V3;
use pxl_spec::space::coord::world::nearest;
use xpute_guest::global::Global;

use pxl_spec::bake::place_tile::{PlaceTile, PLACE_LATTICE};
use pxl_spec::space::coord::pvf3::pack;
use pxl_spec::space::coord::pvf3::pvf3::Pvf3;
use pxl_spec::space::coord::v3::len3;
use pxl_spec::space::coord::world::REACH;
use pxl_spec::space::coord::zoom::{base_of, count, cube, Zoom};
use pxl_spec::space::layer::lattice::GROUND;
use pxl_spec::space::layer::tile::{Cell, LEAF_ZOOM};
use std::collections::HashSet;

#[derive(Default)]
struct Placement {
    batch: BillboardBatch,
    kept: Vec<u32>,
    set: Option<u32>,
    drawn: Vec<u32>,
    /// Nearest the eye first: screen box, label, and where it stands.
    hits: Vec<([f64; 4], Label, Pvf3)>,
    /// Kept so a settled turn asks the heap for nothing. Each with the scale
    /// it shows at and where in its places cell that scale is read.
    found: Vec<(Cell, Label, Cell, f64, Shown, V3)>,
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
    /// Markers whose new words are not ready yet, kept to become so.
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

/// Every place, or only the prominent: what a cell shows by the units a
/// logical pixel spans there.
#[derive(Clone, Copy, PartialEq)]
enum Shown {
    All,
    Major,
}

fn shown_at(units_per_px: f64) -> Option<Shown> {
    if units_per_px <= ALL_UNITS_PER_PX {
        Some(Shown::All)
    } else if units_per_px <= MAJOR_UNITS_PER_PX {
        Some(Shown::Major)
    } else {
        None
    }
}

/// The markers put up in `chunk`, by the chunk their base is in.
fn markers(chunk: Chunk, read_zoom: Zoom, found: &mut Vec<(Cell, Label, Cell, f64, Shown, V3)>) {
    let region = chunk.region();
    let objects = &crate::kernel().objects;
    let Some(cells) = objects.read(chunk.node(), chunk.cell_zoom()).value() else { return };
    cells.entries(&mut |e| {
        if !object_value_is_marker(e.state) {
            return;
        }
        let marker = marker_at(e.id(), objects.words(e).unwrap_or_default(), read_zoom);
        if (0..3).all(|k| (region.min[k]..region.max[k]).contains(&marker.5[k])) {
            found.push(marker);
        }
    });
}

/// A marker at its cell's center over the floor it stands on, its words by
/// their hash, so words changed under one entry are another badge.
fn marker_at(at: EntryAt, words: &str, read_zoom: Zoom) -> (Cell, Label, Cell, f64, Shown, V3) {
    let c = cube(at.zoom(), at.at());
    let base = [(c.min[0] + c.max[0]) / 2.0, (c.min[1] + c.max[1]) / 2.0, c.min[2]];
    let ground = GROUND.cell_of(LEAF_ZOOM, &base_of(at.zoom(), at.at()));
    let mut h = fnv::FnvHasher::default();
    h.write(words.as_bytes());
    let label = Label::Marker { at, words: h.finish() };
    (PLACE_LATTICE.cell_of(read_zoom, &ground.mid()), label, ground, base[2], Shown::All, base)
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

pub fn step(ctx: &mut TickContext) {
    let p = PLACEMENT.get(Placement::default);
    if crate::view::observation::eye().is_none() {
        if let Some(set) = p.set.take() {
            remove_billboard_set(set);
            ctx.invalidate();
        }
        p.hits.clear();
        badge::retain(&HashSet::new());
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
        let read_zoom = PLACE_LATTICE.read_zoom(chunk.cell_zoom());
        markers(chunk, read_zoom, &mut found);
        if !places {
            continue;
        }
        let Answer::Exact(Places { tile, from: src }) = PlaceTile::residency().read(chunk.node(), chunk.cell_zoom()) else {
            continue;
        };
        // Where the source cell is wider than the chunk, its place counts only if it
        // stands on the chunk's ground.
        let (side, cell) = (chunk.column().0, PLACE_LATTICE.cell_zoom(src.zoom));
        let m = if cell >= side { count(cell, side) } else { 0 };
        let [x0, y0] = PLACE_LATTICE.cell_of(src.zoom, &Pvf3 { i: chunk.corner(), f: 0 }).in_tile();
        let (h0, h1) = chunk.z_range();
        for ty in y0..y0 + m.max(1) {
            let (lo, hi) = (PLACE_LATTICE.cell_index(x0, ty), PLACE_LATTICE.cell_index(x0 + m.max(1), ty));
            let from = tile.cells.partition_point(|c| c.cell < lo);
            for c in tile.cells[from..].iter().take_while(|c| c.cell < hi) {
                let cell = PLACE_LATTICE.cell_at(src, c.cell);
                // Near, a place or a count; farther, only a prominent enough place.
                // A count stands at its cell's middle.
                let all = match (c.count, tile.shown(c).next()) {
                    (1, Some((place, _, at))) => (Label::Place { tile: src, place }, cell.inner_at(GROUND, LEAF_ZOOM, at)),
                    _ => (Label::Count(c.count, c.partial), GROUND.cell_of(LEAF_ZOOM, &cell.mid())),
                };
                let major = tile
                    .shown(c)
                    .map(|(i, p, at)| (i, rank_of(p.kind), at))
                    .filter(|s| s.1 <= PLACE_MAJOR_RANK)
                    .min_by_key(|s| s.1)
                    .map(|(place, _, at)| (Label::Place { tile: src, place }, cell.inner_at(GROUND, LEAF_ZOOM, at)));
                let mid = cell.mid();
                let mid = pack::unpack(mid.i, mid.f);
                for (shown, (label, ground)) in [(Shown::All, all)].into_iter().chain(major.map(|m| (Shown::Major, m))) {
                    if m == 0 && Chunk::holding(chunk.zoom(), &ground.mid()).column() != chunk.named().column() {
                        continue;
                    }
                    // Counted only by the chunk the top of what stands there is in, so a
                    // column's several chunks do not repeat it.
                    let Some(top) = top_at(&ground.mid(), read_zoom) else { continue };
                    if top - 1.0 < h0 || top - 1.0 >= h1 {
                        continue;
                    }
                    found.push((PLACE_LATTICE.cell_of(read_zoom, &ground.mid()), label, ground, top, shown, [mid[0], mid[1], top]));
                }
            }
        }
    }
    found.sort_unstable_by_key(|f| f.0);
    p.batch.reset();
    let drawn_before = core::mem::take(&mut p.labeled);
    let mut labeled = core::mem::take(&mut p.spare);
    labeled.clear();
    p.pending.clear();
    let eye = crate::view::observation::eye().unwrap_or_default();
    // A waiting marker stands over whatever stands at its place, with the words it has so far.
    let draft = draft();
    let drafted = draft.map(|d| marker_at(d.at, &d.words, LEAF_ZOOM));
    let is_drafted = |label: &Label| matches!(label, Label::Marker { at, .. } if Some(*at) == draft.map(|d| d.at));
    for &(_, label, stands, top, shown, at) in drafted.iter().chain(found.iter().filter(|f| !is_drafted(&f.1))) {
        // Found once while it stands, so its lap is chosen as the eye moves.
        let d = len3(&[nearest(at[0], eye[0]) - eye[0], at[1] - eye[1], at[2] - eye[2]]);
        if shown_at(units_per_px_at(d)) != Some(shown) {
            continue;
        }
        // A marker's words changed: what was drawn stays until the new words are ready to show.
        let label = match label {
            Label::Marker { at: e, .. } => match drawn_before.iter().find(|d| matches!(d.0, Label::Marker { at, .. } if at == e)) {
                Some(&(before, _)) if before != label && !badge::ready(label) => {
                    p.pending.push(label);
                    before
                }
                _ => label,
            },
            _ => label,
        };
        let Some(width) = badge::width(label) else { continue };
        let center = stands.center(top);
        let local = crate::view::frame::coord::to_world(&center);
        p.batch.push(local[0], local[1], local[2], width, badge::ANCHOR, labeled.len() as f64);
        labeled.push((label, center));
    }
    badge::retain(&labeled.iter().map(|l| l.0).chain(p.pending.iter().copied()).collect());
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
    // Farthest first: the pass writes no depth, so the nearer must draw last.
    let batch = &p.batch;
    p.drawn.sort_by(|&a, &b| {
        batch.d2[b as usize]
            .total_cmp(&batch.d2[a as usize])
            .then(batch.tiebreak[a as usize].total_cmp(&batch.tiebreak[b as usize]))
    });
    p.hits.clear();
    p.hits.extend(p.drawn.iter().rev().map(|&i| (p.batch.screen[i as usize], labeled[i as usize].0, labeled[i as usize].1)));
    let mut instances = core::mem::take(&mut p.instances);
    instances.clear();
    // f32 instances, so about the sea under the eye rather than the world's origin.
    let about = crate::view::frame::pose::sea_under_eye();
    for &i in &p.drawn {
        let i = i as usize;
        let at = [p.batch.local_x[i] - about[0], p.batch.local_y[i] - about[1], p.batch.local_z[i] - about[2]];
        instances.extend_from_slice(&billboard_instance(
            at,
            [p.batch.w_px[i] as f64, badge::HEIGHT],
            badge::slot(labeled[i].0) as u32,
            badge::ANCHOR / badge::HEIGHT,
        ));
    }
    if p.set.is_some() || !instances.is_empty() {
        let set = *p.set.get_or_insert_with(|| new_billboard_set(BillboardOrder::Place));
        place_billboard_set(set, about);
        write_billboard_set(set, &instances);
        ctx.invalidate();
    }
    (p.labeled, p.spare, p.instances) = (labeled, drawn_before, instances);
}

/// Where a badge's tap goes: to its place, or into a count, so it comes
/// apart into its places; a marker someone put up is its entry's, which the tool
/// held decides what to do with.
pub enum Went {
    To(Pvf3),
    Into(Pvf3),
    Marker(EntryAt, Pvf3),
}

/// `ndc` in [−1, 1], y up. The badge that took the tap, if any.
pub fn tap(ndc_x: f64, ndc_y: f64) -> Option<Went> {
    let [x, y] = ndc_to_px([ndc_x, ndc_y]);
    let hits = &PLACEMENT.get(Placement::default).hits;
    let &(_, label, stands) = hits.iter().find(|(r, _, _)| (r[0]..=r[2]).contains(&x) && (r[1]..=r[3]).contains(&y))?;
    Some(match label {
        Label::Count(..) => Went::Into(stands),
        Label::Marker { at, .. } => Went::Marker(at, stands),
        Label::Place { .. } => Went::To(stands),
    })
}
