// pxl-geo-bake/deck/approach.rs

//! A structure's approaches: the way it joins at the surface, any `layer`.

use std::collections::HashMap;

use pxl_spec::bake::deck::Form;

use crate::deck::param::APPROACH_TURN_COS;
use crate::deck::piece::{End, Piece};
use crate::deck::source;
use crate::deck::structure::end_of;
use crate::terrain::pipeline::overture::between::{flag_ranges, without, Line};
use crate::terrain::pipeline::overture::policy::{DECK_FLAGS, UNSEEN_FLAGS};
use crate::terrain::pipeline::overture::width::{width_m, WayShape};
use crate::wkb::wkb_to_geometries;

/// Ends only one piece reaches.
pub fn free_ends(pieces: &[Piece]) -> Vec<(usize, bool)> {
    let mut count: HashMap<&End, usize> = HashMap::new();
    for end in pieces.iter().flat_map(|p| &p.ends) {
        *count.entry(end).or_default() += 1;
    }
    (0..pieces.len()).flat_map(|i| [(i, false), (i, true)]).filter(|&(i, last)| count[pieces[i].end(last)] == 1).collect()
}

/// The straightest joined way; ties by id, then outward.
pub fn approaches(url: &str, pieces: &[Piece], ends: &[(usize, bool)], form: Form) -> Result<Vec<Piece>, postgres::Error> {
    let asked: Vec<([f64; 2], &str)> = ends.iter().map(|&(i, last)| (end_of(&pieces[i], last).0, pieces[i].gers_id.as_str())).collect();

    let mut best: HashMap<usize, ((f64, String, bool), Piece)> = HashMap::new();
    for (e, own_at, row) in source::connected_rows(url, &asked)? {
        let (i, last) = ends[e];
        let structure = &pieces[i];
        let (at, inward) = end_of(structure, last);
        let Some(g) = wkb_to_geometries(&row.geom_wkb).into_iter().next() else { continue };
        let Some(line) = Line::of(&g.points) else { continue };
        let whole = [g.points[0], g.points[g.points.len() - 1]];
        let len = line.length();
        let step = (source::SIDE_STEP / len).min(0.5);
        let off = flag_ranges(row.flags.as_deref(), &[UNSEEN_FLAGS, DECK_FLAGS].concat());
        let stretches = without(&[[0.0, 1.0]], &off);
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
            let far = End::of(&row.gers_id, whole, if outward > 0.0 { b } else { a });
            let (dx, dy) = (points[1][0] - points[0][0], points[1][1] - points[0][1]);
            let straight = -(dx * inward[0] + dy * inward[1]) / libm::hypot(dx, dy).max(f64::EPSILON);
            let key = (straight, row.gers_id.clone(), outward > 0.0);
            let beats = |old: &(f64, String, bool)| key.0.total_cmp(&old.0).then_with(|| old.1.cmp(&key.1)).then(key.2.cmp(&old.2)).is_gt();
            if straight < APPROACH_TURN_COS || best.get(&e).is_some_and(|(old, _)| !beats(old)) {
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
                ends: [structure.end(last).clone(), far],
                points,
            };
            best.insert(e, (key, piece));
        }
    }
    let mut out: Vec<(usize, Piece)> = best.into_iter().map(|(e, (_, p))| (e, p)).collect();
    out.sort_unstable_by_key(|&(e, _)| e);
    Ok(out.into_iter().map(|(_, p)| p).collect())
}
