// pxl-geo-bake/deck/approach.rs

//! Where a structure's end goes on at the ground: its own way past the flag,
//! or the ways the source's connector at a segment's end joins, each stretch
//! only to the next flag or its segment's end. Along one, the road's distance
//! from the ground falls from what the end holds to none, at `PREFERRED_SLOPE`
//! or steeper where the stretch is shorter, so the road follows the ground.
//! Where another structure's end is across the stretch, each takes what it
//! needs, and where the two need more than it holds, both steepen to meet the
//! ground at one point, each taking it in the share of the change it makes.

use std::collections::HashMap;

use pxl_spec::space::adapter::epsg3857::{from_db, scale_at};

use crate::deck::param::{PREFERRED_SLOPE, STEP_M};
use crate::deck::piece::End;
use crate::deck::relate::Way;
use crate::terrain::pipeline::overture::between::{cell_of, flag_ranges, without};
use crate::terrain::pipeline::overture::policy::{DECK_FLAGS, UNSEEN_FLAGS};

/// A structure's end with nothing of its own form past it.
pub struct Free<'a> {
    pub end: &'a End,
    pub gers_id: &'a str,
    /// Where on its way, and whether its way goes on past it the way it runs.
    pub at: f64,
    pub onward: bool,
}

/// A stretch of ground way leaving an end: the way, where it leaves and where
/// it ends as fractions of it, its length, the free end across it if one is,
/// and its samples in metres from the end.
pub struct Branch {
    pub end: usize,
    pub way: usize,
    pub from: f64,
    pub to: f64,
    pub length_m: f64,
    pub across: Option<usize>,
    pub at: Vec<(f64, [f64; 2])>,
}

#[derive(serde::Deserialize)]
struct Connector {
    at: f64,
    connector_id: String,
}

fn connectors(way: &Way) -> Vec<Connector> {
    way.row.connectors.as_deref().and_then(|c| serde_json::from_str(c).ok()).unwrap_or_default()
}

/// Bit-identical fractions: a connector at a segment's end is at 0 or 1 exactly.
const SAME_AT: f64 = 1e-9;

/// The stretches leaving each free end, sampled where `inside` holds.
pub fn branches(ways: &[Way], free: &[Free], inside: impl Fn([f64; 2]) -> bool) -> Vec<Branch> {
    let by_id = |id: &str| ways.binary_search_by(|w| w.row.gers_id.as_str().cmp(id)).ok();
    let mut joined: HashMap<String, Vec<(usize, f64)>> = HashMap::new();
    for (w, way) in ways.iter().enumerate() {
        for c in connectors(way) {
            joined.entry(c.connector_id).or_default().push((w, c.at));
        }
    }
    let ends: HashMap<&End, usize> = free.iter().enumerate().map(|(e, f)| (f.end, e)).collect();

    let mut out = Vec::new();
    for (e, f) in free.iter().enumerate() {
        let Some(own) = by_id(f.gers_id) else { continue };
        // where the end leaves onto ground, and the way it goes
        let mut leaves: Vec<(usize, f64, f64)> = vec![(own, f.at, if f.onward { 1.0 } else { -1.0 })];
        if matches!(f.end, End::Vertex(..)) {
            let at_end = connectors(&ways[own]).into_iter().filter(|c| (c.at - f.at).abs() < SAME_AT);
            for (w, at) in at_end.flat_map(|c| joined.get(&c.connector_id).cloned().unwrap_or_default()).filter(|&(w, _)| w != own) {
                leaves.extend([(w, at, 1.0), (w, at, -1.0)]);
            }
        }
        for (w, from, dir) in leaves {
            let way = &ways[w];
            let (len, flags) = (way.line.length(), way.row.flags.as_deref());
            let ground = without(&[[0.0, 1.0]], &flag_ranges(flags, &[DECK_FLAGS, UNSEEN_FLAGS].concat(), cell_of(len)));
            let Some(&[lo, hi]) = ground.iter().find(|r| (r[0]..=r[1]).contains(&(from + dir * SAME_AT))) else { continue };
            let to = if dir > 0.0 { hi } else { lo };
            let far = match to {
                t if t > 0.0 && t < 1.0 => End::Cut(way.row.gers_id.clone(), bits(t)),
                t => {
                    let p = way.line.points[if t > 0.0 { way.line.points.len() - 1 } else { 0 }];
                    End::Vertex(bits(p[0]), bits(p[1]))
                }
            };
            let start = way.line.at(from * len);
            let per_m = scale_at(from_db(start[1]));
            let length_m = (to - from).abs() * len / per_m;
            let mut at = Vec::new();
            let mut d = 0.0;
            loop {
                let p = way.line.at((from + dir * d * per_m / len) * len);
                if !inside(p) {
                    break;
                }
                at.push((d, p));
                if d >= length_m {
                    break;
                }
                d = (d + STEP_M).min(length_m);
            }
            if at.len() >= 2 {
                out.push(Branch { end: e, way: w, from, to, length_m, across: ends.get(&far).copied(), at });
            }
        }
    }
    out
}

fn bits(v: f64) -> u64 {
    (v + 0.0).to_bits()
}

/// Where along its way, as a fraction, a branch's road meets the ground: as
/// far as its end needs, or, where its end and the one `across` it need more
/// than the stretch holds, the one point they share, each taking it in the
/// share of the change it makes. Reckoned from the stretch's lower fraction,
/// so both ends find the same bits.
pub fn meets(b: &Branch, sep: f64, across: Option<f64>) -> f64 {
    let (s, length) = (sep.abs(), b.length_m);
    match across {
        Some(o) if s + o.abs() > PREFERRED_SLOPE * length => {
            let (lo, hi, s_lo, s_hi) = if b.from < b.to { (b.from, b.to, s, o.abs()) } else { (b.to, b.from, o.abs(), s) };
            lo + (hi - lo) * s_lo / (s_lo + s_hi)
        }
        _ => b.from + (b.to - b.from) * (s / PREFERRED_SLOPE).min(length) / length,
    }
}

/// The road along a branch from the end's `top` to where it meets the ground
/// at fraction `to`, the ground under each sample given; the meeting point
/// itself is the caller's, one point however many roads end on it.
pub fn profile(b: &Branch, top: f64, ground: &[f64], to: f64) -> Vec<[f64; 3]> {
    let sep = top - ground[0];
    let reach = b.length_m * (to - b.from) / (b.to - b.from);
    if reach <= 0.0 {
        return Vec::new();
    }
    let slope = sep.abs() / reach;
    b.at.iter().zip(ground).take_while(|((d, _), _)| *d < reach).map(|(&(d, p), &z)| [p[0], p[1], z + sep.signum() * (sep.abs() - slope * d)]).collect()
}

#[cfg(test)]
#[path = "approach.test.rs"]
mod test;
