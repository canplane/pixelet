// pxl-geo-bake/terrain/pipeline/corridor.rs

//! Paves the gap between two ways that are one strip of road (a sidewalk drawn
//! apart from its road, two carriageways). A gap a cell or wider stays land,
//! as osm2streets keeps a buffer; a narrower one would rasterize as a broken
//! thread of land that folds into a broken road, so it is paved.

use std::collections::HashMap;

use pxl_spec::bake::way::{self, PATHS};

use crate::terrain::pipeline::feature::{Feature, Joins, Layer, Rank};

pub(crate) const GAP: f64 = 1.0;

/// As Arnis takes a dual carriageway's.
const SIDE_HEADING_COS: f64 = 0.939_692_620_785_908_4; // cos 20°

const BUCKET: f64 = 16.0;

struct Seg {
    feat: usize,
    a: [f64; 2],
    b: [f64; 2],
}

pub fn corridor_fills(feats: &[Feature]) -> Vec<Feature> {
    let line = |f: &Feature| !f.closed && f.rank.layer == Layer::Road;
    let gap = GAP;
    let mut buckets: HashMap<(i64, i64), Vec<Seg>> = HashMap::new();
    let mut widest = 0.0f64;
    for (i, f) in feats.iter().enumerate().filter(|(_, f)| line(f)) {
        widest = widest.max(way::reach(f.width));
        for w in f.points.windows(2) {
            let (a, b) = (w[0], w[1]);
            let key = (((a[0] + b[0]) / 2.0 / BUCKET).floor() as i64, ((a[1] + b[1]) / 2.0 / BUCKET).floor() as i64);
            buckets.entry(key).or_default().push(Seg { feat: i, a, b });
        }
    }

    let mut out = Vec::new();
    for (i, f) in feats.iter().enumerate().filter(|(_, f)| line(f)) {
        let Some(joins) = f.joins else { continue };
        let partner = |other: &Feature| match joins {
            // A sidewalk paves to a road, never to another path.
            Joins::Road => other.joins != Some(Joins::Road) && !PATHS.contains(&other.cell),
            Joins::Carriageway => other.joins == Some(Joins::Carriageway) && other.cell == f.cell,
        };
        let reach = way::reach(f.width);
        let mut strip: Vec<([f64; 2], [f64; 2])> = Vec::new();
        let flush = |strip: &mut Vec<([f64; 2], [f64; 2])>, out: &mut Vec<Feature>| {
            for pair in strip.windows(2) {
                let ((p0, q0), (p1, q1)) = (pair[0], pair[1]);
                out.push(fill(f, [p0, p1, q1, q0]));
            }
            strip.clear();
        };
        for w in f.points.windows(2) {
            let (a, b) = (w[0], w[1]);
            let len = libm::hypot(b[0] - a[0], b[1] - a[1]);
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
                // The nearest partner; an equal distance falls to its rank, then the point, so no row order picks it.
                let mut best: Option<(f64, &Rank, [f64; 2])> = None;
                for x in bx - span..=bx + span {
                    for y in by - span..=by + span {
                        for s in buckets.get(&(x, y)).into_iter().flatten() {
                            let g = &feats[s.feat];
                            if s.feat == i || !partner(g) {
                                continue;
                            }
                            let (dx, dy) = (s.b[0] - s.a[0], s.b[1] - s.a[1]);
                            let slen = libm::hypot(dx, dy);
                            if slen == 0.0 || (dir[0] * dx + dir[1] * dy).abs() / slen < SIDE_HEADING_COS {
                                continue;
                            }
                            let u = (((p[0] - s.a[0]) * dx + (p[1] - s.a[1]) * dy) / (slen * slen)).clamp(0.0, 1.0);
                            let q = [s.a[0] + dx * u, s.a[1] + dy * u];
                            let d = libm::hypot(p[0] - q[0], p[1] - q[1]);
                            let between = d - reach - way::reach(g.width);
                            let nearer = |&(bd, br, bq): &(f64, &Rank, [f64; 2])| {
                                d.total_cmp(&bd)
                                    .then_with(|| g.rank.cmp(br))
                                    .then_with(|| q[0].total_cmp(&bq[0]))
                                    .then_with(|| q[1].total_cmp(&bq[1]))
                                    .is_lt()
                            };
                            if between > 0.0 && between < gap && best.as_ref().is_none_or(nearer) {
                                best = Some((d, &g.rank, q));
                            }
                        }
                    }
                }
                match best {
                    Some((_, _, q)) => strip.push((p, q)),
                    None => flush(&mut strip, &mut out),
                }
            }
        }
        flush(&mut strip, &mut out);
    }
    out
}

fn fill(f: &Feature, quad: [[f64; 2]; 4]) -> Feature {
    Feature {
        points: quad.to_vec(),
        holes: Vec::new(),
        closed: true,
        width: f.width,
        cut: f.cut,
        cell: f.cell,
        rank: Rank {
            order: f.rank.order - 1,
            ..f.rank.clone()
        },
        joins: None,
    }
}
