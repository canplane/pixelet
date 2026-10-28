// pxl-geo-bake/terrain/pipeline/separation.rs

//! A tile's separation from the solve's rows. A tile keeps whole segments within
//! a line's footprint (pxl-spec bake/separation.rs `footprint`, `runs_within`),
//! each point quantized once in the world's grid of leaf cells and the tile's
//! integer origin then taken off, so tiles sharing a line hold the same
//! numbers for its points: what the fold takes each segment once by.

use crate::terrain::CELL_U;
use pxl_spec::bake::separation::{self, Builder, Separation, POINT_SCALE};
use pxl_spec::space::coord::adapter::epsg3857::{get_tile_bbox, u_from_mer};
use pxl_spec::space::layer::lattice::TILE_CELLS;
use pxl_spec_core::param::tile::{LEAF_ZOOM, TOP_ZOOM};
use xpute_kit::math::scalar::round;

use crate::separation::Line;
use crate::terrain::pipeline::overture::policy::segment_type;

/// All of them: a tile whose lines a record's room does not hold is refused
/// where it is encoded rather than cut, since a missing line is a gap in a
/// road. Each line holds a point where it crosses a tile's edge
/// (`crossings`).
pub fn bake_separation(lines: &[Line], tile: [i32; 2]) -> Separation {
    let mut separation = Builder::default();
    // The world's corner, where its grid of leaf cells starts; a point in that
    // grid, in 1/POINT_SCALE of a cell, the tile's own whole cells taken off.
    let corner = get_tile_bbox(0, [0, 0])[0];
    let grid = |m: f64, k: usize| round(u_from_mer(m - corner[k]) / CELL_U * POINT_SCALE);
    let origin = [0, 1].map(|k| f64::from(tile[k]) * f64::from(TILE_CELLS) * POINT_SCALE);
    debug_assert!({
        let min = get_tile_bbox(LEAF_ZOOM, tile)[0];
        [0, 1].iter().all(|&k| grid(min[k], k) == origin[k])
    });
    for row in lines {
        let kind = segment_type(&row.subtype, row.class.as_deref());
        let width = row.width_u / CELL_U;
        let width_code = separation::width_byte(width);
        let world: Vec<Point> = row
            .points
            .iter()
            .zip(&row.coarsest)
            .map(|(p, &c)| ([grid(p[0], 0), grid(p[1], 1)], p[2], c))
            .collect();
        let local: Vec<([f64; 2], i16, u8)> = crossings(&world)
            .into_iter()
            .map(|(q, top, c)| {
                (
                    [0, 1].map(|k| (q[k] - origin[k]) / POINT_SCALE),
                    round(top) as i16,
                    c,
                )
            })
            .collect();
        let at: Vec<[f64; 2]> = local.iter().map(|p| p.0).collect();
        for [a, b] in separation::runs_within(&at, separation::footprint(width)) {
            separation.push(
                local[a..=b].iter().copied(),
                width_code,
                kind,
                row.form,
                row.clear,
            );
        }
    }
    separation.finish()
}

/// A line's point in the world's grid, in 1/POINT_SCALE of a leaf cell, with
/// its surface and the coarsest zoom it is held at.
type Point = ([f64; 2], f64, u8);

/// The coarsest zoom a point on a tile edge is held at, the edge `m` tiles
/// of the leaves from the world's: one over the coarsest whose tiles have
/// it, so every zoom holds where its line crosses each edge of the tiles of
/// the zoom under it, and a segment a zoom holds lies in one tile of the
/// zoom under it — whose copy of the line holds every point between its
/// ends, which is what a fold of its 4×4 children needs (pxl-spec
/// bake/separation.rs `fold_separation`).
fn edge_coarsest(m: i64) -> u8 {
    let first = LEAF_ZOOM.saturating_sub(if m == 0 {
        LEAF_ZOOM
    } else {
        m.trailing_zeros().min(LEAF_ZOOM)
    });
    first.saturating_sub(1).max(TOP_ZOOM) as u8
}

/// The line through `points` with a point where it crosses each edge of the
/// leaves' tiles, an edge's own coordinate exact and the other and the
/// surface rounded (half to even) on the segment the grid's two points
/// make, so every tile holding the line crosses the same; a point that
/// lies on an edge is held as one there is.
fn crossings(points: &[Point]) -> Vec<Point> {
    let edge = f64::from(TILE_CELLS) * POINT_SCALE;
    let on = |q: [f64; 2], c: u8| {
        let mut c = c;
        for v in q {
            if (v / edge).fract() == 0.0 {
                c = c.min(edge_coarsest((v / edge) as i64));
            }
        }
        c
    };
    let mut out: Vec<Point> = Vec::with_capacity(points.len());
    let put = |out: &mut Vec<Point>, p: Point| match out.last_mut() {
        Some(last) if last.0 == p.0 => last.2 = last.2.min(p.2),
        _ => out.push(p),
    };
    for (i, &(a, ta, ca)) in points.iter().enumerate() {
        put(&mut out, (a, ta, on(a, ca)));
        let Some(&(b, tb, _)) = points.get(i + 1) else {
            break;
        };
        let mut cut: Vec<(f64, Point)> = Vec::new();
        for k in 0..2 {
            let (lo, hi) = (a[k].min(b[k]), a[k].max(b[k]));
            for m in (lo / edge).floor() as i64 + 1..=(hi / edge).ceil() as i64 - 1 {
                let x = m as f64 * edge;
                let t = (x - a[k]) / (b[k] - a[k]);
                let mut q = [0.0; 2];
                q[k] = x;
                q[1 - k] = round(a[1 - k] + t * (b[1 - k] - a[1 - k]));
                cut.push((t, (q, round(ta + t * (tb - ta)), edge_coarsest(m))));
            }
        }
        cut.sort_by(|x, y| x.0.total_cmp(&y.0));
        for (_, (q, top, c)) in cut {
            put(&mut out, (q, top, on(q, c)));
        }
    }
    out
}
