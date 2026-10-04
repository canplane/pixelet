// pxl-geo-bake/terrain/pipeline/ground.rs

//! Rasterizes features into the page's packed ground cells, rows
//! south to north. Bounds are `i64` so a negative one skips a loop rather
//! than wrapping in `u32`.

use pxl_spec::bake::format::{Ground, GroundChannelKey};
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

/// A page's land against the sea: none of it, all of it, or the land
/// polygons of a shore, each its outer ring and holes.
pub enum Coast {
    Sea,
    Land,
    Shore(Vec<(Vec<P>, Vec<Vec<P>>)>),
}

pub fn bake_ground(features: &[Feature], coast: &Coast) -> Ground {
    let mut cells = vec![with_channel(0, GroundChannelKey::LAND, cube_types::land::sea); PAGE_AREA];
    match coast {
        Coast::Sea => {}
        Coast::Land => cells.fill(with_channel(0, GroundChannelKey::LAND, cube_types::land::bare)),
        Coast::Shore(land) => {
            let mut target = Target {
                cells: &mut cells,
                key: GroundChannelKey::LAND,
            };
            for (points, holes) in land {
                render_polygon(points, cube_types::land::bare, &mut target, holes, Mask::None);
            }
        }
    }
    let shore = matches!(coast, Coast::Shore(_));

    // Stable: equal ranks keep row order, which decides what paints over what.
    let mut ordered: Vec<&Feature> = features.iter().collect();
    ordered.sort_by_key(|f| f.rank);
    for f in ordered {
        let mask = match f.cell.key {
            GroundChannelKey::LAND | GroundChannelKey::WATER if shore => Mask::Sea,
            _ => Mask::None,
        };
        let target = &mut Target { cells: &mut cells, key: f.cell.key };
        if f.closed {
            render_polygon(&f.points, f.cell.value, target, &f.holes, mask);
        } else {
            render_linear_feat(&f.points, f.cell.value, target, f.width, f.cut, mask);
        }
    }

    Ground { cells: cells.into() }
}

#[inline]
fn put(target: &mut Target, mask: Mask, at: usize, value: u8) {
    if mask.blocks(target.cells, at) {
        return;
    }
    target.cells[at] = with_channel(target.cells[at], target.key, value);
}

fn render_polygon(points: &[P], value: u8, target: &mut Target, holes: &[Vec<P>], mask: Mask) {
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

#[cfg(test)]
#[path = "ground.test.rs"]
mod test;
