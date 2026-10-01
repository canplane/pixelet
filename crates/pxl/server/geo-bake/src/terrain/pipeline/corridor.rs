// pxl-geo-bake/terrain/pipeline/corridor.rs

//! The gap between two ways that are one strip of road on the ground: a
//! sidewalk drawn apart from its road, the two carriageways of a road drawn
//! as two (pxl-spec bake/model/feature.rs `Joins`).
//!
//! osm2streets, which builds street geometry from OSM, zips a sidewalk into
//! its road with a buffer lane between them — the physical separation that
//! made a mapper draw it apart — and gives a planted buffer or a verge a
//! typical 2 m (osm2lanes `typical_lane_widths`). A gap that wide is real
//! and stays the land it is. A gap no wider is the buffer itself, narrower
//! than the cell it would have to be drawn in, and left unpainted it is a
//! sliver of land between two ways that folds, a level up, into a broken
//! line; so it is paved as the way that joins across it.

use pxl_spec::bake::constant::XY_CELL_PER_M;
use pxl_spec::bake::format::{CellConfig, GroundChannelKey};
use pxl_spec::bake::model::feature::{Feature, FeatureShape, Joins};
use pxl_spec::bake::way;
use pxl_spec::cell::cube::cube_types;
use std::collections::HashMap;

/// The widest gap paved, in metres: osm2lanes' typical buffer.
const BUFFER_M: f64 = 2.0;

/// How far apart two ways' headings may be and still run side by side, as
/// Arnis takes a dual carriageway's.
const SIDE_HEADING_COS: f64 = 0.939_692_620_785_908_4; // cos 20°

/// Grid cells the partner index is bucketed in, in page cells.
const BUCKET: f64 = 16.0;

/// A segment of a way a joiner may join, in page cells.
struct Seg {
    feat: usize,
    a: [f64; 2],
    b: [f64; 2],
}

/// The paved gaps of a page's ways, as features to paint under them.
pub fn corridor_fills(feats: &[Feature]) -> Vec<Feature> {
    let line = |f: &Feature| !f.shape.is_closed && !f.extra.is_unseen && !f.extra.is_deck && f.cell_cfg.is_some_and(|c| c.ch_key == GroundChannelKey::TRANSPORT);
    let gap = BUFFER_M * XY_CELL_PER_M;
    let mut buckets: HashMap<(i64, i64), Vec<Seg>> = HashMap::new();
    let mut widest = 0.0f64;
    for (i, f) in feats.iter().enumerate().filter(|(_, f)| line(f)) {
        widest = widest.max(way::reach(f.computed.width));
        for w in f.shape.points.windows(2) {
            let (a, b) = ([w[0][0], w[0][1]], [w[1][0], w[1][1]]);
            let key = (((a[0] + b[0]) / 2.0 / BUCKET).floor() as i64, ((a[1] + b[1]) / 2.0 / BUCKET).floor() as i64);
            buckets.entry(key).or_default().push(Seg { feat: i, a, b });
        }
    }

    let mut out = Vec::new();
    for (i, f) in feats.iter().enumerate().filter(|(_, f)| line(f)) {
        let Some(joins) = f.extra.joins else { continue };
        let cfg = f.cell_cfg.unwrap();
        let partner = |g: &Feature| match joins {
            Joins::Road => g.extra.joins != Some(Joins::Road) && g.cell_cfg.is_some_and(|c| c.cell_type != cube_types::transport::pedestrian),
            Joins::Carriageway => g.extra.joins == Some(Joins::Carriageway) && g.cell_cfg == f.cell_cfg,
        };
        let reach = way::reach(f.computed.width);
        // Each point a cell apart along the way, and where across the gap it
        // is paved to, if anywhere.
        let mut strip: Vec<([f64; 2], [f64; 2])> = Vec::new();
        let flush = |strip: &mut Vec<([f64; 2], [f64; 2])>, out: &mut Vec<Feature>| {
            for pair in strip.windows(2) {
                let ((p0, q0), (p1, q1)) = (pair[0], pair[1]);
                out.push(fill(f, cfg, [p0, p1, q1, q0]));
            }
            strip.clear();
        };
        for w in f.shape.points.windows(2) {
            let (a, b) = ([w[0][0], w[0][1]], [w[1][0], w[1][1]]);
            let len = (b[0] - a[0]).hypot(b[1] - a[1]);
            if len == 0.0 {
                continue;
            }
            let dir = [(b[0] - a[0]) / len, (b[1] - a[1]) / len];
            let steps = len.ceil() as usize;
            for k in 0..=steps {
                let t = k as f64 / steps as f64;
                let p = [a[0] + (b[0] - a[0]) * t, a[1] + (b[1] - a[1]) * t];
                let search = reach + widest + gap;
                let (bx, by) = ((p[0] / BUCKET).floor() as i64, (p[1] / BUCKET).floor() as i64);
                let span = (search / BUCKET).ceil() as i64;
                let mut best: Option<(f64, [f64; 2])> = None;
                for x in bx - span..=bx + span {
                    for y in by - span..=by + span {
                        for s in buckets.get(&(x, y)).into_iter().flatten() {
                            let g = &feats[s.feat];
                            if s.feat == i || !partner(g) {
                                continue;
                            }
                            let (dx, dy) = (s.b[0] - s.a[0], s.b[1] - s.a[1]);
                            let slen = dx.hypot(dy);
                            if slen == 0.0 || (dir[0] * dx + dir[1] * dy).abs() / slen < SIDE_HEADING_COS {
                                continue;
                            }
                            let u = (((p[0] - s.a[0]) * dx + (p[1] - s.a[1]) * dy) / (slen * slen)).clamp(0.0, 1.0);
                            let q = [s.a[0] + dx * u, s.a[1] + dy * u];
                            let d = (p[0] - q[0]).hypot(p[1] - q[1]);
                            let between = d - reach - way::reach(g.computed.width);
                            if between > 0.0 && between <= gap && best.is_none_or(|(bd, _)| d < bd) {
                                best = Some((d, q));
                            }
                        }
                    }
                }
                match best {
                    Some((_, q)) => strip.push((p, q)),
                    None => flush(&mut strip, &mut out),
                }
            }
        }
        flush(&mut strip, &mut out);
    }
    out
}

/// The quad between two points along a way and where each is paved to, as a
/// feature that paints just under the way.
fn fill(f: &Feature, cfg: CellConfig, quad: [[f64; 2]; 4]) -> Feature {
    let mut extra = f.extra;
    extra.joins = None;
    extra.rank = f.extra.rank.saturating_sub(1);
    Feature {
        id: f.id.clone(),
        center: [quad[0][0], quad[0][1], 0.0],
        shape: FeatureShape {
            points: quad.iter().map(|p| [p[0], p[1], 0.0]).collect(),
            is_closed: true,
            holes: None,
        },
        computed: f.computed,
        category: f.category.clone(),
        extra,
        cell_cfg: Some(cfg),
    }
}
