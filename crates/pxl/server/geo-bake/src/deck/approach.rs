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
                form: Form::Ramp,
                approach: true,
                points,
            };
            best.insert(e, (straight, piece));
        }
    }
    Ok(best.into_values().map(|(_, p)| p).collect())
}
