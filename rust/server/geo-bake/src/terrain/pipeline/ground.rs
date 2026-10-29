// pxl-geo-bake/terrain/pipeline/ground.rs

//! Rasterizes features into the tile's ground cells, rows south to north.
//! Bounds are `i64` so a negative one skips a loop rather than wrapping in
//! `u32`.

use crate::terrain::CELL_U;
use pxl_spec::asset::cube::ground;
use pxl_spec::bake::tile::terrain::{Ground, GroundId};
use pxl_spec::space::layer::lattice::{GROUND, TILE_CELLS};
use pxl_spec_core::lattice::CELL_CENTER;
use pxl_spec_core::way;

use crate::terrain::pipeline::feature::{Feature, Layer, P};

const TILE_AREA: usize = (TILE_CELLS * TILE_CELLS) as usize;
const SPAN: i64 = TILE_CELLS as i64;

fn cell_idx(x: i64, y: i64) -> usize {
    GROUND.cell_index(x as u32, y as u32) as usize
}

/// Landcover and water do not paint the sea: the cells the coast left water,
/// fixed before any feature paints, since a salt pond paints water too.
type Sea<'a> = Option<&'a [bool]>;

/// A tile's land against the sea: none of it, all of it, or the land
/// polygons of a shore, each its outer ring and holes.
pub enum Coast {
    Sea,
    Land,
    Shore(Vec<(Vec<P>, Vec<Vec<P>>)>),
}

pub fn bake_ground(feats: &[Feature], coast: &Coast) -> Ground {
    let mut cells = vec![ground::water; TILE_AREA];
    match coast {
        Coast::Sea => {}
        Coast::Land => cells.fill(ground::BASE),
        Coast::Shore(land) => {
            for (points, holes) in land {
                render_polygon(points, ground::BASE, &mut cells, holes, None);
            }
        }
    }
    let sea: Option<Vec<bool>> = (!matches!(coast, Coast::Land))
        .then(|| cells.iter().map(|&c| c == ground::water).collect());

    let mut ordered: Vec<&Feature> = feats.iter().collect();
    ordered.sort_by(|a, b| a.rank.cmp(&b.rank));
    for f in ordered {
        let mask = match f.rank.layer {
            Layer::Landcover | Layer::LandcoverLine | Layer::WaterLine | Layer::Water => {
                sea.as_deref()
            }
            _ => None,
        };
        if f.closed {
            render_polygon(&f.points, f.cell, &mut cells, &f.holes, mask);
        } else {
            render_linear_feat(&f.points, f.cell, &mut cells, f.width / CELL_U, f.cut, mask);
        }
    }

    Ground {
        cells: cells.into(),
    }
}

#[inline]
fn put<T: Copy>(cells: &mut [T], mask: Sea, at: usize, val: T) {
    if mask.is_some_and(|sea| sea[at]) {
        return;
    }
    cells[at] = val;
}

/// Every cell of a tile of the ground's lattice whose center the polygon covers.
pub(crate) fn render_polygon<T: Copy>(
    points: &[P],
    val: T,
    cells: &mut [T],
    holes: &[Vec<P>],
    mask: Sea,
) {
    if points.len() < 3 {
        return;
    }

    let (min_y, max_y) = points
        .iter()
        .fold((f64::INFINITY, f64::NEG_INFINITY), |(lo, hi), p| {
            (lo.min(p[1]), hi.max(p[1]))
        });
    let start_row = (min_y - f64::from(CELL_CENTER)).ceil().max(0.0);
    let end_row = (max_y - f64::from(CELL_CENTER))
        .floor()
        .min((SPAN - 1) as f64);
    if !start_row.is_finite() || !end_row.is_finite() || start_row > end_row {
        return;
    }

    // Half-open, so cells along a shared edge are painted exactly once (the GPU
    // top-left rule).
    let first = |a: f64| (a - f64::from(CELL_CENTER)).ceil().clamp(0.0, SPAN as f64) as i64;
    let mut xs: Vec<f64> = Vec::new();

    for y in start_row as i64..=end_row as i64 {
        let yc = y as f64 + f64::from(CELL_CENTER);
        xs.clear();
        find_intersections(points, yc, &mut xs);
        for hole in holes {
            find_intersections(hole, yc, &mut xs);
        }
        xs.sort_by(f64::total_cmp);

        let mut i = 0usize;
        if !xs.len().is_multiple_of(2) {
            for x in 0..first(xs[0]) {
                put(cells, mask, cell_idx(x, y), val);
            }
            i = 1;
        }

        while i + 1 < xs.len() {
            for x in first(xs[i])..first(xs[i + 1]) {
                put(cells, mask, cell_idx(x, y), val);
            }
            i += 2;
        }
    }
}

/// Every cell whose center the way covers along a segment (pxl-spec-core
/// way.rs `on_way`, the rule a column is crossed by), its line cut square
/// where `cut` says at its first and last.
fn render_linear_feat(
    points: &[P],
    val: GroundId,
    cells: &mut [GroundId],
    width_cell: f64,
    cut: [bool; 2],
    mask: Sea,
) {
    let (width, center) = (width_cell as f32, f64::from(CELL_CENTER));
    let reach = f64::from(way::reach(width));
    let last = points.len().saturating_sub(2);
    for (s, w) in points.windows(2).enumerate() {
        let (a, b) = ([w[0][0], w[0][1]], [w[1][0], w[1][1]]);
        let span = |k: usize| {
            let lo = (a[k].min(b[k]) - reach - center).ceil().max(0.0) as i64;
            let hi = (a[k].max(b[k]) + reach - center)
                .floor()
                .min((SPAN - 1) as f64) as i64;
            lo..=hi
        };
        let ends = [s == 0 && cut[0], s == last && cut[1]];
        for y in span(1) {
            for x in span(0) {
                let at = [x as f64 + center, y as f64 + center].map(|v| v as f32);
                if way::on_way(a.map(|v| v as f32), b.map(|v| v as f32), width, ends, at).1 {
                    put(cells, mask, cell_idx(x, y), val);
                }
            }
        }
    }
}

fn find_intersections(nodes: &[P], ys: f64, out: &mut Vec<f64>) {
    if nodes.is_empty() {
        return;
    }
    let mut j = nodes.len() - 1;
    for i in 0..nodes.len() {
        let yi = nodes[i][1];
        let yj = nodes[j][1];
        // Half-open in y, so a vertex on the scanline is crossed once and a flat
        // edge never.
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
