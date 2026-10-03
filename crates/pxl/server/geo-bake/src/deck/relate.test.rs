// pxl-geo-bake/deck/relate.test.rs

use super::*;
use crate::deck::piece::End;
use pxl_spec::bake::deck::Form;

fn row(id: &str, points: &[[f64; 2]]) -> BridgeRow {
    let mut wkb = vec![1u8, 2, 0, 0, 0];
    wkb.extend((points.len() as u32).to_le_bytes());
    for p in points {
        wkb.extend(p[0].to_le_bytes());
        wkb.extend(p[1].to_le_bytes());
    }
    BridgeRow {
        gers_id: id.into(),
        subtype: "road".into(),
        class: None,
        flags: None,
        levels: None,
        widths: None,
        access: None,
        subclass: None,
        subclasses: None,
        geom_wkb: wkb.into(),
    }
}

fn line(points: &[[f64; 2]]) -> Polyline {
    Polyline::of(points.iter().copied())
}

fn piece(id: &str, level: i64, points: &[[f64; 2]]) -> Piece {
    let points: Vec<[f64; 3]> = points.iter().map(|p| [p[0], p[1], 0.0]).collect();
    let whole = [points[0], points[points.len() - 1]];
    Piece {
        gers_id: id.into(),
        subtype: "road".into(),
        class: None,
        level,
        width_m: 4.0,
        form: Form::Deck,
        ends: [End::of(id, whole, 0.0), End::of(id, whole, 1.0)],
        start: 0.0,
        points,
    }
}

#[test]
fn a_way_under_a_deck_is_found_where_it_crosses() {
    let ways = [Way::of(row("w", &[[5.0, -5.0], [5.0, 5.0]])).unwrap()];
    let c = ways_crossing(&[line(&[[0.0, 0.0], [10.0, 0.0]])], &ways);
    assert_eq!(c.len(), 1);
    assert_eq!((c[0].at, c[0].own_at), (5.0, 0.5));
}

#[test]
fn water_under_a_deck_is_the_stretch_over_it_holes_left_out() {
    // A 4 wide square with a 2 wide hole in its middle.
    let mut wkb = vec![1u8, 3, 0, 0, 0, 2, 0, 0, 0];
    for ring in [[[2.0, -2.0], [6.0, -2.0], [6.0, 2.0], [2.0, 2.0], [2.0, -2.0]], [[3.0, -1.0], [5.0, -1.0], [5.0, 1.0], [3.0, 1.0], [3.0, -1.0]]] {
        wkb.extend(5u32.to_le_bytes());
        for p in ring {
            wkb.extend(f64::to_le_bytes(p[0]));
            wkb.extend(f64::to_le_bytes(p[1]));
        }
    }
    let water = [Water::of("river".into(), None, &wkb)];
    let mut under: Vec<[f64; 2]> = water_under(&[line(&[[0.0, 0.0], [10.0, 0.0]])], &water).iter().map(|w| [w.from, w.to]).collect();
    under.sort_by(|a, b| a[0].total_cmp(&b[0]));
    assert_eq!(under, [[2.0, 3.0], [5.0, 6.0]]);
}

#[test]
fn decks_cross_through_each_other_not_where_one_ends_on_the_other() {
    let pieces = [piece("a", 1, &[[0.0, 0.0], [10.0, 0.0]]), piece("b", 2, &[[5.0, -5.0], [5.0, 5.0]]), piece("c", 2, &[[8.0, 0.0], [8.0, 5.0]])];
    let lines: Vec<Polyline> = pieces.iter().map(|p| line(&p.points.iter().map(|q| [q[0], q[1]]).collect::<Vec<_>>())).collect();
    let refs: Vec<&Piece> = pieces.iter().collect();
    let c = decks_crossing(&refs, &lines);
    assert_eq!(c.len(), 1, "c only touches a");
    assert_eq!((c[0].upper, c[0].lower, c[0].upper_at, c[0].lower_at), (1, 0, 5.0, 5.0));
}

#[test]
fn a_way_ending_at_a_point_is_connected_and_one_passing_is_not_unless_it_is_its_own() {
    let ways = [
        Way::of(row("ends", &[[0.0, 0.0], [10.0, 0.0]])).unwrap(),
        Way::of(row("passes", &[[0.0, -5.0], [0.0, 5.0]])).unwrap(),
        Way::of(row("own", &[[-5.0, 0.0], [5.0, 0.0]])).unwrap(),
    ];
    let mut found: Vec<&str> = connected(&[([0.0, 0.0], "own")], &ways).into_iter().map(|(_, _, w)| w.row.gers_id.as_str()).collect();
    found.sort_unstable();
    assert_eq!(found, ["ends", "own"]);
}
