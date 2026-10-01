// pxl-geo-bake/deck/approach.rs

//! A structure's approaches: the way at the surface a bridge is carried down
//! to the ground on, and the way a tunnel drops through to its portal.
//!
//! OSM tags a bridge where the bridge is and a tunnel where its roof is, and
//! the approach to either — the embankment a bridge climbs on, the cutting a
//! tunnel's road drops through — as a way of its own:
//!
//! - **A ramp.** Arnis takes a way joined to a bridge's end as its ramp only
//!   where it is tagged an embankment or stands at `layer` 1 or over, and
//!   otherwise brings the deck down to the ground inside its own length.
//!   Overture carries no embankment, but it carries the layer
//!   (`level_rules`), so a ramp is the stretch of a joined way that stands at
//!   level 1 or over, from the bridge's end to where that stretch ends.
//! - **A cutting.** A tunnel's portal is where its roof begins, so the floor
//!   is already a bore and its roof under the ground there, and the drop to
//!   it is the way before the portal's, cut open — as Arnis descends the
//!   surface way into a portal so that the bore starts at depth. It is as
//!   long as that drop takes at the steepest a way climbs, along the way at
//!   the surface joined to the portal, and no longer: past it the way is on
//!   the ground. Deeper than that is the tunnel's own to go, inside.
//!
//! Either is a piece of the structure whose far end is on the ground, and the
//! solve grades the whole of it, as far as it leaves the ground (deck.rs
//! `on_structure`).

use std::collections::HashMap;

use pxl_spec::bake::deck::Form;

use crate::deck::clearance::{BORE_DEPTH, MAX_SLOPE};
use crate::deck::piece::Piece;
use crate::deck::source;
use crate::deck::structure::{end_of, key_of};
use crate::terrain::pipeline::overture::between::{flag_ranges, value_ranges, without, Line, Range};
use crate::terrain::pipeline::overture::policy::{DECK_FLAGS, UNSEEN_FLAGS};
use crate::terrain::pipeline::overture::width::{width_m, WayShape};
use crate::wkb::wkb_to_geometries;

/// How far an approach may turn from the structure it continues and still be
/// its approach: a slip road leaving at an angle is another way.
const APPROACH_TURN_COS: f64 = 0.5; // 60°

/// The ends of `pieces` only one of them reaches: where a structure meets
/// the ground or stops, as (piece, its last end).
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

/// Each end's approach of `form` — a ramp up to a deck, a cutting down to a
/// portal: the way joined to it at the surface that carries on most nearly
/// straight, over the stretch the approach takes of it, as a piece of the
/// structure.
pub fn approaches(url: &str, pieces: &[Piece], ends: &[(usize, bool)], form: Form) -> Result<Vec<Piece>, postgres::Error> {
    let asked: Vec<([f64; 2], &str)> = ends.iter().map(|&(i, last)| (end_of(&pieces[i], last).0, pieces[i].gers_id.as_str())).collect();

    // The best candidate each end has: how straight it carries on, and the
    // approach it makes.
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
        let (stretches, most): (Vec<Range>, f64) = match form {
            Form::Ramp => (without(&value_ranges(row.levels.as_deref(), |v| v >= 1.0), &off), f64::INFINITY),
            _ => {
                let mut below: Vec<Range> = [off, value_ranges(row.levels.as_deref(), |v| v < 0.0)].concat();
                below.sort_by(|x, y| x[0].total_cmp(&y[0]));
                // The drop to the portal, at the steepest a way climbs.
                (without(&[[0.0, 1.0]], &below), BORE_DEPTH / MAX_SLOPE * structure.scale() / len)
            }
        };
        for outward in [1.0, -1.0] {
            let next = own_at + outward * step;
            let Some(&[from, to]) = stretches.iter().find(|r| (r[0]..=r[1]).contains(&next)) else { continue };
            let (a, b) = if outward > 0.0 { (own_at, to.min(own_at + most)) } else { (from.max(own_at - most), own_at) };
            if (b - a) * len < 1.0 {
                continue;
            }
            let mut points = line.between(a, b);
            if outward < 0.0 {
                points.reverse();
            }
            points[0] = [at[0], at[1], 0.0];
            let (dx, dy) = (points[1][0] - points[0][0], points[1][1] - points[0][1]);
            let straight = -(dx * inward[0] + dy * inward[1]) / dx.hypot(dy).max(f64::EPSILON);
            if straight < APPROACH_TURN_COS || best.get(&e).is_some_and(|(s, _)| *s >= straight) {
                continue;
            }
            let shape = WayShape::at(row.columns(), (a + b) / 2.0);
            let piece = Piece {
                gers_id: row.gers_id.clone(),
                subtype: row.subtype.clone(),
                class: row.class.clone(),
                level: 0,
                width_m: width_m(&row.subtype, row.class.as_deref(), shape),
                form,
                approach: true,
                points,
            };
            best.insert(e, (straight, piece));
        }
    }
    Ok(best.into_values().map(|(_, p)| p).collect())
}
