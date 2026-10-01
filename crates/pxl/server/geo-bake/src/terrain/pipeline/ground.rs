// pxl-geo-bake/terrain/pipeline/ground.rs

//! Rasterize normalized features into the ground cells of the current tile.
//!
//! - input features are interpreted relative to the current L3 page base
//! - raster rows follow south-to-north world ordering
//! - the raster contract is driven by the baked ground constants
//!
//! A page is one array of packed cells and a feature paints one channel's
//! field of it (pxl-spec bake/pack.rs), so nothing here holds a plane: the
//! page leaves this function in the shape the wire carries and the client
//! holds. This is the one place that touches every cell of a page.
//!
//! Row and column bounds are computed in `f64` and carried as `i64`, not as
//! the unsigned indices they end up being: a bound below zero or past the
//! page leaves a loop that simply does not run, where the same expression in
//! `u32` would wrap into a very long one.

use pxl_spec::bake::format::{Ground, GroundChannelKey};
use pxl_spec::bake::model::feature::Feature;
use pxl_spec::bake::pack::{channel_of, with_channel, GroundCell};
use pxl_spec::bake::way;
use pxl_spec::cell::cube::cube_types;
use pxl_spec::space::pyramid::{GROUND, PAGE_CELLS};
use pxl_spec::space::v3::V3;

const PAGE_AREA: usize = (PAGE_CELLS * PAGE_CELLS) as usize;
const SPAN: i64 = PAGE_CELLS as i64;

/// A cell is painted by what covers its center, (x + ½, y + ½) — the
/// center the elevation is sampled at (pxl-spec bake/constant.rs) and the
/// one a deck's columns are read at (bake/deck.rs), so a way on the ground,
/// the same way on a bridge and the height under both stand on one grid.
const PX_CENTER: f64 = 0.5;

fn cell_idx(x: i64, y: i64) -> usize {
    GROUND.cell_index(x as u32, y as u32) as usize
}

/// Whether a pass stops at sea. Water and land overlays must not paint over
/// true sea cells, and the land channel is the mask for both — including for
/// a land feature, which reads the field it is writing as it writes it.
#[derive(Clone, Copy, PartialEq)]
enum Mask {
    None,
    Sea,
}

impl Mask {
    #[inline]
    fn blocks(&self, cells: &[GroundCell], at: usize) -> bool {
        *self == Mask::Sea && channel_of(cells[at], GroundChannelKey::LAND) == cube_types::land::sea
    }
}

/// A channel of the page being painted: the cells, and which field of them
/// this pass writes.
struct Target<'a> {
    cells: &'a mut [GroundCell],
    key: GroundChannelKey,
}

pub fn bake_ground(elements: &[Feature], has_coast_row: bool) -> Ground {
    let mut cells = vec![0 as GroundCell; PAGE_AREA];
    // The land field of every cell, the others untouched: nothing has been
    // painted yet, so the rest of the word is void.
    let fill_land = |cells: &mut Vec<GroundCell>, t: u8| cells.fill(with_channel(0, GroundChannelKey::LAND, t));

    let sea_feats: Vec<&Feature> = elements.iter().filter(|f| f.extra.is_ocean).collect();
    let mut other_feats: Vec<&Feature> = elements.iter().filter(|f| !f.extra.is_ocean).collect();
    // Stable: features of equal rank keep the order their rows arrived in,
    // and that order decides what paints over what.
    other_feats.sort_by_key(|f| f.extra.rank);
    let has_sea = !sea_feats.is_empty();

    // Inverse sea pass: initialize the land channel with the sea/land base first.
    if has_sea {
        fill_land(&mut cells, cube_types::land::sea);
        let mut land = Target {
            cells: &mut cells,
            key: GroundChannelKey::LAND,
        };
        for f in &sea_feats {
            // Only closed polygons carve land out of the sea base.
            if f.shape.is_closed {
                render_polygon(&f.shape.points, cube_types::land::bare, &mut land, f.shape.holes.as_deref(), Mask::None);
            }
        }
    } else if has_coast_row {
        // No sea feature reached here, but coast/adapter.rs's from_coast_row
        // did see a coastline row for this page and dropped it (covers_page):
        // a single land polygon covers the page's full envelope, so painting
        // bare directly is equivalent to carving it, without paying for the
        // scanline rasterization.
        fill_land(&mut cells, cube_types::land::bare);
    } else {
        // No coastline row touched this page's envelope at all — there is no
        // land anywhere nearby, so this is open ocean, not "no data yet".
        // Before this branch existed, this case fell through to the bare
        // default above and every open-ocean page baked as flat dry land.
        fill_land(&mut cells, cube_types::land::sea);
    }

    // Render categorized ground features.
    for feat in &other_feats {
        if feat.extra.is_unseen || feat.extra.is_deck {
            continue;
        }
        let Some(cfg) = feat.cell_cfg else { continue };

        // Land and water stop at sea; a structure or a road does not.
        let mask = match cfg.ch_key {
            GroundChannelKey::LAND | GroundChannelKey::WATER if has_sea => Mask::Sea,
            _ => Mask::None,
        };
        paint(feat, cfg.cell_type, &mut Target { cells: &mut cells, key: cfg.ch_key }, mask);
    }

    Ground { cells: cells.into() }
}

fn paint(feat: &Feature, value: u8, target: &mut Target, mask: Mask) {
    if feat.shape.is_closed {
        render_polygon(&feat.shape.points, value, target, feat.shape.holes.as_deref(), mask);
    } else {
        render_linear_feat(&feat.shape.points, value, target, feat.computed.width, mask);
    }
}

// ---- Rasterizer core ----

#[inline]
fn put(target: &mut Target, mask: Mask, at: usize, value: u8) {
    if mask.blocks(target.cells, at) {
        return;
    }
    target.cells[at] = with_channel(target.cells[at], target.key, value);
}

/// Rasterize a polygon ring set with optional hole rings.
fn render_polygon(points: &[V3], value: u8, target: &mut Target, holes: Option<&[Vec<V3>]>, mask: Mask) {
    if points.len() < 3 {
        return;
    }

    const NO_HOLES: &[Vec<V3>] = &[];
    let inner_list: &[Vec<V3>] = holes.unwrap_or(NO_HOLES);

    // The rows whose centers the outer ring spans.
    let (min_y, max_y) = points.iter().fold((f64::INFINITY, f64::NEG_INFINITY), |(lo, hi), p| (lo.min(p[1]), hi.max(p[1])));
    let start_row = (min_y - PX_CENTER).ceil().max(0.0);
    let end_row = (max_y - PX_CENTER).floor().min((SPAN - 1) as f64);
    if !start_row.is_finite() || !end_row.is_finite() || start_row > end_row {
        return;
    }

    // The cells of a row whose centers lie in [a, b): half-open, so two
    // polygons that share an edge paint each cell along it once between them
    // and leave none — the top-left rule a GPU fills triangles by.
    let first = |a: f64| (a - PX_CENTER).ceil().clamp(0.0, SPAN as f64) as i64;
    let mut xs: Vec<f64> = Vec::new();

    // Single even-odd scanline fill over outer + holes, along each row's
    // centers.
    for y in start_row as i64..=end_row as i64 {
        let yc = y as f64 + PX_CENTER;
        xs.clear();
        find_intersections(points, yc, &mut xs);
        for hole in inner_list {
            find_intersections(hole, yc, &mut xs);
        }
        xs.sort_by(f64::total_cmp);

        // The odd-intersection case: the span from the left edge to the first
        // crossing is filled, and the rest pair up from there.
        let mut i = 0usize;
        if !xs.len().is_multiple_of(2) {
            for x in 0..first(xs[0]) {
                put(target, mask, cell_idx(x, y), value);
            }
            i = 1;
        }

        while i + 1 < xs.len() {
            for x in first(xs[i])..first(xs[i + 1]) {
                put(target, mask, cell_idx(x, y), value);
            }
            i += 2;
        }
    }
}

/// Rasterize an open polyline: every cell whose center the way covers
/// (pxl-spec bake/way.rs), segment by segment — the distance to a segment
/// rounds its ends, so a bend is as wide as a straight and no cap is drawn.
fn render_linear_feat(points: &[V3], value: u8, target: &mut Target, width_cell: f64, mask: Mask) {
    let reach = way::reach(width_cell);
    for w in points.windows(2) {
        let (a, b) = ([w[0][0], w[0][1]], [w[1][0], w[1][1]]);
        let cells = |k: usize| {
            let lo = (a[k].min(b[k]) - reach - PX_CENTER).ceil().max(0.0) as i64;
            let hi = (a[k].max(b[k]) + reach - PX_CENTER).floor().min((SPAN - 1) as f64) as i64;
            lo..=hi
        };
        for y in cells(1) {
            for x in cells(0) {
                let at = [x as f64 + PX_CENTER, y as f64 + PX_CENTER];
                if way::covers(width_cell, distance_to_segment(at, a, b)) {
                    put(target, mask, cell_idx(x, y), value);
                }
            }
        }
    }
}

/// How far `p` lies from the segment `a`–`b`.
fn distance_to_segment(p: [f64; 2], a: [f64; 2], b: [f64; 2]) -> f64 {
    let (dx, dy) = (b[0] - a[0], b[1] - a[1]);
    let len2 = dx * dx + dy * dy;
    let t = if len2 > 0.0 { (((p[0] - a[0]) * dx + (p[1] - a[1]) * dy) / len2).clamp(0.0, 1.0) } else { 0.0 };
    (p[0] - a[0] - dx * t).hypot(p[1] - a[1] - dy * t)
}

/// Collect scanline edge intersections in tile pixel space. `j` trails `i` by
/// one around the ring.
fn find_intersections(nodes: &[V3], ys: f64, out: &mut Vec<f64>) {
    if nodes.is_empty() {
        return;
    }
    let mut j = nodes.len() - 1;
    for i in 0..nodes.len() {
        let yi = nodes[i][1];
        let yj = nodes[j][1];
        // Half-open in y, so a vertex on the scanline is crossed once and a
        // level edge never.
        if (yi <= ys && yj > ys) || (yj <= ys && yi > ys) {
            let t = (ys - yi) / (yj - yi);
            out.push(nodes[i][0] + t * (nodes[j][0] - nodes[i][0]));
        }
        j = i;
    }
}
