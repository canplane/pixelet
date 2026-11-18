// pxl-spec/bake/tile/terrain.test.rs

use xpute_golden::Golden;

use super::*;
use crate::bake::tile::imagery;
use xpute_kit::math::rng::SplitMix64;

const GOLDEN: &str = include_str!(concat!(env!("PXL_REPO"), "/spec/golden/bake/terrain.tsv"));

/// A seeded generator, so that a tile's lanes are the same on every run.
struct Rng(SplitMix64);

impl Rng {
    fn next(&mut self) -> u64 {
        u64::from(self.0.next_u32()) << 32 | u64::from(self.0.next_u32())
    }
}

/// Random lines.
fn lines(rng: &mut Rng) -> Separation {
    let mut separation = separation::Builder::default();
    for _ in 0..3 {
        let points: Vec<([f64; 2], i16, u8)> = (0..4)
            .map(|_| {
                (
                    [(rng.next() % 600) as f64, (rng.next() % 600) as f64],
                    rng.next() as i16,
                    rng.next() as u8,
                )
            })
            .collect();
        separation.push(
            points,
            1 + (rng.next() % 200) as u8,
            rng.next() as u8,
            separation::Form::of((rng.next() % 3) as u8),
            rng.next() as u8,
        );
    }
    separation.finish()
}

/// Random lanes, as the bake holds them.
fn tile(seed: u64) -> TerrainTile {
    let mut rng = Rng(SplitMix64::new(seed));
    let ele: Vec<i16> = (0..ELE_LANE).map(|_| rng.next() as i16).collect();
    let cells: Vec<GroundId> = (0..FOLD_LANE).map(|_| rng.next() as GroundId).collect();
    TerrainTile::of(
        ele,
        cells.into(),
        lines(&mut rng),
        Occupancy::of_bits(0b10101).unwrap(),
    )
}

/// How a client takes a decode's refusals, the object's or the heap's.
type Refused = Box<dyn std::error::Error>;

/// A tile round-trips, in hand and into a record's room.
#[test]
fn formats_round_trip() {
    let tile = tile(0x9e37_79b9_7f4a_7c15);
    let terrain = encode(&tile).unwrap();
    assert_eq!(
        decode(&terrain).unwrap(),
        tile,
        "a tile reads back as it was written"
    );
    // On the CPU its lines, but for what only a bake reads: a fold's
    // coarsest zooms.
    let resident = decode_into::<Refused>(&terrain, vec![0; ELE_LANE].leak()).unwrap();
    let want = Separation {
        first_zooms: Default::default(),
        ..tile.separation.clone()
    };
    assert_eq!(resident.separation, want, "its lines");
    assert_eq!(resident.ele, tile.ele, "its samples");
    assert_eq!(resident.occupancy, tile.occupancy);
}

/// A brick's entries are the walk of the tile's ways (`Separation::reaches`) where it falls
/// in the brick, in the walk's order: each bin of the brick a segment's footprint reaches,
/// with the point the segment starts at. A place outside the tile has none, and the bricks'
/// entries together are the walk, each of its steps in one brick.
///
/// A bin is 16 cells and a brick four bins a side. The first line, a cell wide, reaches
/// 1.71 cells from itself: from (40, 40) to (72, 40) it reaches the bins 2 to 4 of row 2,
/// the first two brick (0, 0)'s and the third brick (1, 0)'s; then down to (72, 24), the
/// bins of column 4 in rows 1 and 2, brick (1, 0)'s. The second line runs off the tile's
/// far corner and is held to its last bin, the last of brick (7, 7).
#[test]
fn brick_entries_are_the_walk_by_brick() {
    let mut lines = separation::Builder::default();
    lines.push(
        [
            ([40.0, 40.0], 0, 0),
            ([72.0, 40.0], 0, 0),
            ([72.0, 24.0], 0, 0),
        ],
        8,
        0,
        separation::Form::Bridge,
        0,
    );
    lines.push(
        [([500.0, 500.0], 0, 0), ([520.0, 500.0], 0, 0)],
        8,
        0,
        separation::Form::Tunnel,
        0,
    );
    let by_brick = BrickEntries::of(&lines.finish()).unwrap();
    let want = |place: [u32; 2]| -> &[(u32, u32)] {
        match place {
            [0, 0] => &[(10, 0), (11, 0)],
            [1, 0] => &[(8, 0), (4, 1), (8, 1)],
            [7, 7] => &[(15, 3)],
            _ => &[],
        }
    };
    for place in (0..8).flat_map(|y| (0..8).map(move |x| [x, y])) {
        assert_eq!(by_brick.at(place), want(place), "brick {place:?}");
    }
    for place in [[8, 0], [0, 8], [u32::MAX, 7]] {
        assert_eq!(by_brick.at(place), [], "{place:?} is outside the tile");
    }
}

/// An imagery tile is as long as its texels say.
#[test]
fn imagery_pkt_size() {
    let tile = imagery::encode(&vec![0; imagery::TILE_SZ]).unwrap();
    assert_eq!(tile.len(), imagery::PKT_SZ as usize);
}

#[test]
fn non_tile_pkt_refused() {
    let v = Golden::load(GOLDEN);
    let small = hex::decode(v.s("old_format_tile")).unwrap();
    assert!(decode(&small).is_err());
    assert!(decode_into::<Refused>(&small, vec![0; ELE_LANE].leak()).is_err());
}

/// Every valid occupancy byte round-trips.
#[test]
fn occupancy_vals() {
    for bits in 0..=u8::MAX {
        if let Some(o) = Occupancy::of_bits(bits) {
            assert_eq!(o.bits(), bits);
            let children: [bool; 4] = std::array::from_fn(|q| o.has_child(q));
            assert!(
                o.is_tile() || !children.contains(&true),
                "{bits:#b}: a child under the zero tile"
            );
        }
    }
    let of = |tiles: [bool; 4]| {
        Occupancy::of_children(tiles.map(|p| if p { Occupancy::LEAF } else { Occupancy::ZERO }))
    };
    assert_eq!(
        of([false; 4]),
        Occupancy::ZERO,
        "a parent of the zero tile alone is the zero tile"
    );
    let some = of([true, false, true, false]);
    assert!(
        some.is_tile()
            && some.has_child(0)
            && !some.has_child(1)
            && some.has_child(2)
            && !some.has_child(3)
    );
}

/// A packet's cells are one asset id a cell or four, and one whose cells
/// lane holds any other count is refused.
#[test]
fn cells_of_any_other_count_refused() {
    let t = tile(1);
    let pkt = |n: usize| {
        let cells: Vec<GroundId> = vec![0; n];
        shape::encode::<Packet>(
            (&t.ele, &cells[..], t.occupancy.bits(), t.separation.pkt()),
            PKT_CAP,
        )
        .unwrap()
    };
    for n in [GROUND_LANE, FOLD_LANE] {
        assert_eq!(cells_of(&pkt(n)).ok().map(<[GroundId]>::len), Some(n));
    }
    for n in [
        0,
        1,
        GROUND_LANE - 1,
        GROUND_LANE + 1,
        2 * GROUND_LANE,
        FOLD_LANE + 1,
    ] {
        assert!(cells_of(&pkt(n)).is_err(), "{n} cells");
    }
}
