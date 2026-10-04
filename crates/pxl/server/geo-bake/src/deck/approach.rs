// pxl-geo-bake/deck/approach.rs

//! A bridge's approaches. Arnis takes a joined way as a ramp where it is an
//! embankment or at `layer` 1 or over; Overture has no embankment, so a ramp
//! is the joined stretch at level 1 or over.

use std::collections::HashMap;

use pxl_spec::bake::deck::Form;

use crate::deck::piece::Piece;
use crate::deck::source;
use crate::deck::structure::{end_of, key_of};
use crate::terrain::pipeline::overture::between::{flag_ranges, value_ranges, without, Line};
use crate::terrain::pipeline::overture::policy::{DECK_FLAGS, UNSEEN_FLAGS};
use crate::terrain::pipeline::overture::width::{width_m, WayShape};
use crate::wkb::wkb_to_geometries;

/// A slip road leaving at an angle is another way.
const APPROACH_TURN_COS: f64 = 0.5; // 60°

/// Ends only one piece reaches.
pub fn free_ends(pieces: &[Piece]) -> Vec<(usize, bool)> {
    let mut count: HashMap<(i64, i64), usize> = HashMap::new();
    for p in pieces {
        for last in [false, true] {
            *count.entry(key_of(if last { *p.points.last().unwrap() } else { p.points[0] })).or_default() += 1;
        }
    }
    (0..pieces.len())
        .flat_map(|i| [(i, false), (i, true)])
        .filter(|&(i, last)| {
            let p = &pieces[i];
            count[&key_of(if last { *p.points.last().unwrap() } else { p.points[0] })] == 1
        })
        .collect()
}

/// Each end's ramp: the joined way at the surface that carries on most nearly straight.
pub fn approaches(url: &str, pieces: &[Piece], ends: &[(usize, bool)]) -> Result<Vec<Piece>, postgres::Error> {
    let asked: Vec<([f64; 2], &str)> = ends.iter().map(|&(i, last)| (end_of(&pieces[i], last).0, pieces[i].gers_id.as_str())).collect();

    let mut best: HashMap<usize, (f64, Piece)> = HashMap::new();
    for (e, own_at, row) in source::connected_rows(url, &asked)? {
        let (i, last) = ends[e];
        let structure = &pieces[i];
        let (at, inward) = end_of(structure, last);
        let Some(g) = wkb_to_geometries(&row.geom_wkb).into_iter().next() else { continue };
        let Some(line) = Line::of(&g.points) else { continue };
        let len = line.length();
        let step = (0.5 / len).min(0.5);
        let off = flag_ranges(row.flags.as_deref(), &[UNSEEN_FLAGS, DECK_FLAGS].concat());
        let stretches = without(&value_ranges(row.levels.as_deref(), |v| v >= 1.0), &off);
        for outward in [1.0, -1.0] {
            let next = own_at + outward * step;
            let Some(&[from, to]) = stretches.iter().find(|r| (r[0]..=r[1]).contains(&next)) else { continue };
            let (a, b) = if outward > 0.0 { (own_at, to) } else { (from, own_at) };
            if (b - a) * len < 1.0 {
                continue;
            }
        }
        for (w, from, dir) in leaves {
            let way = &ways[w];
            let (len, flags) = (way.line.length(), way.row.flags.as_deref());
            let ground = without(&[[0.0, 1.0]], &flag_ranges(flags, &[DECK_FLAGS, UNSEEN_FLAGS].concat(), cell_of(len)));
            let Some(&[lo, hi]) = ground.iter().find(|r| (r[0]..=r[1]).contains(&(from + dir * SAME_AT))) else {
                continue;
            };
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
                out.push(Branch {
                    end: e,
                    way: w,
                    from,
                    to,
                    length_m,
                    across: ends.get(&far).copied(),
                    at,
                });
            }
        }
    }
    Ok(best.into_values().map(|(_, p)| p).collect())
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
    b.at.iter()
        .zip(ground)
        .take_while(|((d, _), _)| *d < reach)
        .map(|(&(d, p), &z)| [p[0], p[1], z + sep.signum() * (sep.abs() - slope * d)])
        .collect()
}

#[cfg(test)]
#[path = "approach.test.rs"]
mod test;
