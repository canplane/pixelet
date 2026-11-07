// pxl-kernel/domain/layer/tile_cache.test.rs
//
// What a tile cache says shows, against a count made afresh.

use super::*;
use pxl_spec::bake::tile::terrain::TerrainTile;

fn asks(tiles: &[Quadrant]) -> FnvHashMap<Quadrant, ()> {
    tiles.iter().map(|&p| (p, ())).collect()
}

/// `is_shown` and `shown_share` are of the asks and what is held as they
/// stand: after every change to either, what they say equals a count made
/// afresh.
#[test]
fn shown_as_counted_afresh() {
    let mut c = TileCache::<TerrainTile>::default();
    let top = Quadrant::new(LEAF_ZOOM - 2, [40, -20]);
    let [a, b, ..] = top.children();
    let fine = a.children()[0];
    let check = |c: &TileCache<TerrainTile>, what: &str| {
        let [asked, shown] = c.count_shown();
        assert_eq!(c.is_shown(), asked == shown, "{what}");
        let share = match asked {
            0 => 0.0,
            _ => f64::from(shown) / f64::from(asked),
        };
        assert_eq!(c.shown_share(), share, "{what}");
    };
    let admit = |c: &mut TileCache<TerrainTile>, p: Quadrant| {
        c.admit(p, |_| None, |_| Ok::<_, ()>(TerrainTile::void()))
            .unwrap();
    };
    check(&c, "nothing asked");
    c.ask(&asks(&[top, a, fine]));
    check(&c, "asked, nothing held");
    admit(&mut c, top);
    check(&c, "the top held");
    c.ask(&asks(&[a, b, fine]));
    check(&c, "the top let go");
    c.answer_above(b);
    check(&c, "one answered from above");
    admit(&mut c, a);
    check(&c, "a finer one held");
    c.ask(&asks(&[top, a, b, fine]));
    check(&c, "the top asked again");
    c.set(fine, TerrainTile::void());
    check(&c, "the finest set");
    c.ask(&asks(&[]));
    check(&c, "nothing asked again");
}
