// pxl-geo-bake/deck/approach.rs

//! Where a structure's way carries on at the surface: each stretch of it
//! once, whichever structure ends reach it, so one profile takes the road
//! from one structure's form to the ground or to the next one's.

use std::collections::{BTreeMap, HashMap};

use crate::deck::param::APPROACH_TURN_COS;
use crate::deck::piece::{End, Piece};
use crate::deck::relate::{self, SIDE_STEP};
use crate::deck::source::Input;
use crate::deck::structure::end_of;
use crate::terrain::pipeline::overture::between::{cell_of, flag_ranges, without, Line};
use crate::terrain::pipeline::overture::policy::{DECK_FLAGS, UNSEEN_FLAGS};
use crate::terrain::pipeline::overture::width::{width_m, WayShape};
use crate::wkb::wkb_to_geometries;

/// A stretch an end reaches: its way, its range, and whether the end is at its far side.
#[derive(Clone, Copy)]
struct Reached {
    way: usize,
    range: [f64; 2],
    at_to: bool,
}

/// Each end's straightest joined stretch, ties by id then outward; a stretch
/// two ends reach is one piece, its ends theirs.
pub fn continuations(input: &Input, pieces: &[Piece], ends: &[(usize, bool)]) -> Vec<Piece> {
    let asked: Vec<([f64; 2], &str)> = ends.iter().map(|&(i, last)| (end_of(&pieces[i], last).0, pieces[i].gers_id.as_str())).collect();

    let mut best: HashMap<usize, ((f64, String, bool), Reached)> = HashMap::new();
    for (e, own_at, way) in relate::connected(&asked, &input.ways) {
        let row = &way.row;
        let (i, last) = ends[e];
        let (_, inward) = end_of(&pieces[i], last);
        let points: Vec<_> = way.line.points.iter().map(|p| [p[0], p[1], 0.0]).collect();
        let Some(line) = Line::of(&points) else { continue };
        let len = line.length();
        let step = (SIDE_STEP / len).min(0.5);
        let off = flag_ranges(row.flags.as_deref(), &[UNSEEN_FLAGS, DECK_FLAGS].concat(), cell_of(len));
        for outward in [1.0, -1.0] {
            let next = own_at + outward * step;
            let Some(&[from, to]) = without(&[[0.0, 1.0]], &off).iter().find(|r| (r[0]..=r[1]).contains(&next)) else { continue };
            if (to - from) * len < 1.0 {
                continue;
            }
            let ahead = line.at(if outward > 0.0 { (from * len + SIDE_STEP).min(to * len) } else { (to * len - SIDE_STEP).max(from * len) });
            let start = line.at(if outward > 0.0 { from * len } else { to * len });
            let (dx, dy) = (ahead[0] - start[0], ahead[1] - start[1]);
            let straight = -(dx * inward[0] + dy * inward[1]) / libm::hypot(dx, dy).max(f64::EPSILON);
            let key = (straight, row.gers_id.clone(), outward > 0.0);
            let beats = |old: &(f64, String, bool)| key.0.total_cmp(&old.0).then_with(|| old.1.cmp(&key.1)).then(key.2.cmp(&old.2)).is_gt();
            if straight < APPROACH_TURN_COS || best.get(&e).is_some_and(|(old, _)| !beats(old)) {
                continue;
            }
            best.insert(e, (key, Reached { way: way_index(input, &row.gers_id), range: [from, to], at_to: outward < 0.0 }));
        }
    }

    let mut stretches: BTreeMap<(usize, [u64; 2]), Piece> = BTreeMap::new();
    let mut found: Vec<(usize, Reached)> = best.into_iter().map(|(e, (_, r))| (e, r)).collect();
    found.sort_unstable_by_key(|&(e, _)| e);
    for (e, Reached { way: w, range: [from, to], at_to }) in found {
        let (i, last) = ends[e];
        let row = &input.ways[w].row;
        let Some(g) = wkb_to_geometries(&row.geom_wkb).into_iter().next() else { continue };
        let Some(line) = Line::of(&g.points) else { continue };
        let whole = [g.points[0], g.points[g.points.len() - 1]];
        let piece = stretches.entry((w, [from.to_bits(), to.to_bits()])).or_insert_with(|| Piece {
            gers_id: row.gers_id.clone(),
            subtype: row.subtype.clone(),
            class: row.class.clone(),
            level: 0,
            width_m: width_m(&row.subtype, row.class.as_deref(), WayShape::at(row.columns(), (from + to) / 2.0)),
            form: pieces[i].form,
            approach: true,
            ends: [End::of(&row.gers_id, whole, from), End::of(&row.gers_id, whole, to)],
            start: from * line.length(),
            points: line.between(from, to),
        });
        let (at, _) = end_of(&pieces[i], last);
        let k = if at_to { piece.points.len() - 1 } else { 0 };
        piece.points[k] = [at[0], at[1], 0.0];
        piece.ends[usize::from(at_to)] = pieces[i].end(last).clone();
    }
    stretches.into_values().collect()
}

/// The ways are by id (source.rs `read_ring`).
fn way_index(input: &Input, gers_id: &str) -> usize {
    input.ways.partition_point(|w| w.row.gers_id.as_str() < gers_id)
}
