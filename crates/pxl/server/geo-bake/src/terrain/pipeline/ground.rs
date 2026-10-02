// pxl-geo-bake/terrain/pipeline/ground.rs

//! Rasterizes normalized features into the page's packed ground cells, rows
//! south to north. Bounds are `i64` so a negative one skips a loop rather
//! than wrapping in `u32`.

use pxl_spec::bake::format::{Ground, GroundChannelKey};
use pxl_spec::bake::model::feature::Feature;
use pxl_spec::bake::pack::{channel_of, with_channel, GroundCell};
use pxl_spec::bake::way;
use pxl_spec::cell::cube::cube_types;
use pxl_spec::space::pyramid::{GROUND, PAGE_CELLS};
use pxl_spec::space::v3::V3;

const PAGE_AREA: usize = (PAGE_CELLS * PAGE_CELLS) as usize;
const SPAN: i64 = PAGE_CELLS as i64;

/// A cell is painted by what covers its center, the same center elevation
/// and decks are sampled at.
const PX_CENTER: f64 = 0.5;

fn cell_idx(x: i64, y: i64) -> usize {
    GROUND.cell_index(x as u32, y as u32) as usize
}

/// Water and land overlays must not paint over sea cells; the land channel
/// is the mask, even for a land feature writing it.
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

struct Target<'a> {
    cells: &'a mut [GroundCell],
    key: GroundChannelKey,
}

pub fn bake_ground(elements: &[Feature], has_coast_row: bool) -> Ground {
    let mut cells = vec![0 as GroundCell; PAGE_AREA];
    let fill_land = |cells: &mut Vec<GroundCell>, t: u8| cells.fill(with_channel(0, GroundChannelKey::LAND, t));

    let sea_feats: Vec<&Feature> = elements.iter().filter(|f| f.extra.is_ocean).collect();
    let mut other_feats: Vec<&Feature> = elements.iter().filter(|f| !f.extra.is_ocean).collect();
    // Stable: equal ranks keep row order, which decides what paints over what.
    other_feats.sort_by_key(|f| f.extra.rank);
    let has_sea = !sea_feats.is_empty();

    if has_sea {
        fill_land(&mut cells, cube_types::land::sea);
        let mut land = Target {
            cells: &mut cells,
            key: GroundChannelKey::LAND,
        };
        for f in &sea_feats {
            if f.shape.is_closed {
                render_polygon(&f.shape.points, cube_types::land::bare, &mut land, f.shape.holes.as_deref(), Mask::None);
            }
        }
    } else if has_coast_row {
        // A coast row covered the whole page: it is all land, no carve needed.
        fill_land(&mut cells, cube_types::land::bare);
    } else {
        // No coast row at all: open ocean. Falling through to bare once baked every
        // ocean page as dry land.
        fill_land(&mut cells, cube_types::land::sea);
    }

    for feat in &other_feats {
        if feat.extra.is_unseen || feat.extra.is_deck {
            continue;
        }
        let Some(cfg) = feat.cell_cfg else { continue };

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

#[inline]
fn put(target: &mut Target, mask: Mask, at: usize, value: u8) {
    if mask.blocks(target.cells, at) {
        return;
    }
    target.cells[at] = with_channel(target.cells[at], target.key, value);
}

fn render_polygon(points: &[V3], value: u8, target: &mut Target, holes: Option<&[Vec<V3>]>, mask: Mask) {
    if points.len() < 3 {
        return;
    }

    const NO_HOLES: &[Vec<V3>] = &[];
    let inner_list: &[Vec<V3>] = holes.unwrap_or(NO_HOLES);

    let (min_y, max_y) = points.iter().fold((f64::INFINITY, f64::NEG_INFINITY), |(lo, hi), p| (lo.min(p[1]), hi.max(p[1])));
    let start_row = (min_y - PX_CENTER).ceil().max(0.0);
    let end_row = (max_y - PX_CENTER).floor().min((SPAN - 1) as f64);
    if !start_row.is_finite() || !end_row.is_finite() || start_row > end_row {
        return;
    }

    // Half-open, so cells along a shared edge are painted exactly once (the GPU
    // top-left rule).
    let first = |a: f64| (a - PX_CENTER).ceil().clamp(0.0, SPAN as f64) as i64;
    let mut xs: Vec<f64> = Vec::new();

    for y in start_row as i64..=end_row as i64 {
        let yc = y as f64 + PX_CENTER;
        xs.clear();
        find_intersections(points, yc, &mut xs);
        for hole in inner_list {
            find_intersections(hole, yc, &mut xs);
        }
        xs.sort_by(f64::total_cmp);

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

/// Every cell whose center lies within the way's half width of a segment;
/// segment distance rounds the ends, so bends need no cap.
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

fn distance_to_segment(p: [f64; 2], a: [f64; 2], b: [f64; 2]) -> f64 {
    let (dx, dy) = (b[0] - a[0], b[1] - a[1]);
    let len2 = dx * dx + dy * dy;
    let t = if len2 > 0.0 { (((p[0] - a[0]) * dx + (p[1] - a[1]) * dy) / len2).clamp(0.0, 1.0) } else { 0.0 };
    (p[0] - a[0] - dx * t).hypot(p[1] - a[1] - dy * t)
}

fn find_intersections(nodes: &[V3], ys: f64, out: &mut Vec<f64>) {
    if nodes.is_empty() {
        return;
    }
    let mut j = nodes.len() - 1;
    for i in 0..nodes.len() {
        let yi = nodes[i][1];
        let yj = nodes[j][1];
        // Half-open in y, so a vertex on the scanline is crossed once and a level
        // edge never.
        if (yi <= ys && yj > ys) || (yj <= ys && yi > ys) {
            let t = (ys - yi) / (yj - yi);
            out.push(nodes[i][0] + t * (nodes[j][0] - nodes[i][0]));
        }
        j = i;
    }
}
