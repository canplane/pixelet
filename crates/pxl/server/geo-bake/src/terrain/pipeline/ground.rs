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
use pxl_spec::cell::cube::cube_types;
use pxl_spec::space::pyramid::{GROUND, PAGE_CELLS};
use pxl_spec::space::v3::V3;
use xpute_core::math::scalar::round;

const PAGE_AREA: usize = (PAGE_CELLS * PAGE_CELLS) as usize;
const SPAN: i64 = PAGE_CELLS as i64;

/// Ground rasterization uses cell-center sampling; the bias lives here rather
/// than sprinkled across the file. The contract must stay aligned with
/// elevation sampling and dense unpack semantics at the page boundary.
const PX_CENTER: f64 = 0.5;
const EPS: f64 = 1e-9;

/// Pixel-center snap for raster coordinates.
fn snap_px(x: f64) -> i64 {
    (x + PX_CENTER).floor() as i64
}

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
        if feat.extra.is_tunnel {
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

    // Bounding box in dense pixel rows, from the outer ring alone.
    let mut min_y = f64::INFINITY;
    let mut max_y = f64::NEG_INFINITY;
    for p in points {
        if p[1] < min_y {
            min_y = p[1];
        }
        if p[1] > max_y {
            max_y = p[1];
        }
    }

    let start_row = min_y.floor().max(0.0);
    let end_row = max_y.ceil().min((SPAN - 1) as f64);
    if !start_row.is_finite() || !end_row.is_finite() || start_row > end_row {
        return;
    }

    let mut xs: Vec<f64> = Vec::new();

    // Single even-odd scanline fill over outer + holes.
    for y in start_row as i64..=end_row as i64 {
        xs.clear();
        find_intersections(points, y, &mut xs);
        for hole in inner_list {
            find_intersections(hole, y, &mut xs);
        }
        // A typed array's sort is numeric ascending, unlike a plain array's.
        xs.sort_by(f64::total_cmp);

        // The odd-intersection case: the span from the left edge to the first
        // crossing is filled, and the rest pair up from there.
        let mut i = 0usize;
        if !xs.len().is_multiple_of(2) {
            let end_x = xs[0].floor().min((SPAN - 1) as f64);
            if end_x.is_finite() && end_x >= 0.0 {
                for x in 0..=end_x as i64 {
                    put(target, mask, cell_idx(x, y), value);
                }
            }
            i = 1;
        }

        while i + 1 < xs.len() {
            let start_x = xs[i].ceil().max(0.0);
            let end_x = xs[i + 1].floor().min((SPAN - 1) as f64);
            if start_x.is_finite() && end_x.is_finite() && start_x <= end_x {
                for x in start_x as i64..=end_x as i64 {
                    put(target, mask, cell_idx(x, y), value);
                }
            }
            i += 2;
        }
    }
}

#[inline]
fn set_pixel(target: &mut Target, mask: Mask, px: i64, py: i64, value: u8) {
    if !(0..SPAN).contains(&px) || !(0..SPAN).contains(&py) {
        return;
    }
    put(target, mask, cell_idx(px, py), value);
}

fn draw_circle(target: &mut Target, mask: Mask, cx: i64, cy: i64, r: i64, value: u8) {
    let r2 = r * r;
    for bx in -r..=r {
        for by in -r..=r {
            if bx * bx + by * by <= r2 {
                set_pixel(target, mask, cx + bx, cy + by, value);
            }
        }
    }
}

/// Rasterize an open polyline with a simple thickness kernel.
fn render_linear_feat(points: &[V3], value: u8, target: &mut Target, width_cell: f64, mask: Mask) {
    if points.len() < 2 {
        return;
    }

    let thickness = round(width_cell * 0.5).max(1.0) as i64;

    for i in 0..points.len() - 1 {
        let p1 = points[i];
        let p2 = points[i + 1];

        // A round cap at each end, centered on the pixel.
        draw_circle(target, mask, snap_px(p1[0]), snap_px(p1[1]), thickness, value);
        if i == points.len() - 2 {
            draw_circle(target, mask, snap_px(p2[0]), snap_px(p2[1]), thickness, value);
        }

        let dx = p2[0] - p1[0];
        let dy = p2[1] - p1[1];
        // Grid-cover sampling: the max-axis delta gives slope-independent
        // coverage, at twice the pixel density so no pixel is stepped over.
        let steps = (dx.abs().max(dy.abs()) * 2.0).ceil().max(1.0);
        if !steps.is_finite() {
            continue;
        }
        let steps = steps as i64;

        for s in 0..=steps {
            let t = s as f64 / steps as f64;
            let cx = snap_px(p1[0] + dx * t);
            let cy = snap_px(p1[1] + dy * t);

            for bx in -thickness..=thickness {
                for by in -thickness..=thickness {
                    set_pixel(target, mask, cx + bx, cy + by, value);
                }
            }
        }
    }
}

/// Collect scanline edge intersections in tile pixel space. `j` trails `i` by
/// one around the ring.
fn find_intersections(nodes: &[V3], y: i64, out: &mut Vec<f64>) {
    if nodes.is_empty() {
        return;
    }
    let mut j = nodes.len() - 1;
    for i in 0..nodes.len() {
        let yi = nodes[i][1];
        let yj = nodes[j][1];
        let dy = yj - yi;
        if dy.abs() >= EPS {
            let ys = y as f64 + EPS;
            if (yi <= ys && yj > ys) || (yj <= ys && yi > ys) {
                let t = (ys - yi) / dy;
                out.push(nodes[i][0] + t * (nodes[j][0] - nodes[i][0]));
            }
        }
        j = i;
    }
}
