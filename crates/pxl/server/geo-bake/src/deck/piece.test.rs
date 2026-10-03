// pxl-geo-bake/deck/piece.test.rs

use super::*;

fn piece(points: &[[f64; 2]]) -> Piece {
    let points: Vec<G3857> = points.iter().map(|p| [p[0], p[1], 0.0]).collect();
    let whole = [points[0], points[points.len() - 1]];
    Piece {
        gers_id: "a".into(),
        subtype: "road".into(),
        class: None,
        level: 1,
        width_m: 4.0,
        form: Form::Deck,
        approach: false,
        ends: [End::of("a", whole, 0.0), End::of("a", whole, 1.0)],
        points,
    }
}

const BOX: [[f64; 2]; 2] = [[0.0, 0.0], [10.0, 10.0]];

fn runs(p: &[[f64; 2]]) -> Vec<Vec<[f64; 2]>> {
    piece(p).clip(BOX).into_iter().map(|r| r.points.iter().map(|q| [q[0], q[1]]).collect()).collect()
}

#[test]
fn a_piece_inside_is_kept_whole_with_its_ends() {
    let p = piece(&[[1.0, 1.0], [9.0, 9.0]]);
    let ends = p.ends.clone();
    let kept = p.clip(BOX);
    assert_eq!(kept.len(), 1);
    assert!(kept[0].ends == ends);
}

#[test]
fn a_piece_across_the_edge_stops_on_it_at_a_fresh_end() {
    let p = piece(&[[5.0, 5.0], [15.0, 5.0]]);
    let start = p.ends[0].clone();
    let kept = p.clip(BOX);
    assert_eq!(kept.len(), 1);
    assert_eq!(kept[0].points.iter().map(|q| [q[0], q[1]]).collect::<Vec<_>>(), [[5.0, 5.0], [10.0, 5.0]]);
    assert!(kept[0].ends[0] == start && kept[0].ends[1] != start);
}

#[test]
fn out_in_out_is_one_run_between_the_edges() {
    assert_eq!(runs(&[[-5.0, 5.0], [15.0, 5.0]]), [[[0.0, 5.0], [10.0, 5.0]]]);
    assert_eq!(runs(&[[-5.0, 5.0], [5.0, 5.0], [5.0, 15.0], [15.0, 15.0], [15.0, 5.0], [5.0, 5.0]]).len(), 2);
}

#[test]
fn touching_a_corner_or_lying_outside_leaves_nothing() {
    assert!(runs(&[[-5.0, 5.0], [5.0, -5.0]]).is_empty(), "a corner touched");
    assert!(runs(&[[20.0, 20.0], [30.0, 30.0]]).is_empty());
}

#[test]
fn a_piece_on_the_edge_is_kept() {
    assert_eq!(runs(&[[0.0, 2.0], [0.0, 8.0]]), [[[0.0, 2.0], [0.0, 8.0]]]);
}
