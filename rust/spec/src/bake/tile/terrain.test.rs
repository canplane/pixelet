// pxl-spec/bake/tile/terrain.test.rs

use xpute_golden::Golden;

use super::*;
use crate::bake::tile::earth;
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
    let ele: Vec<i16> = (0..ELE_COLUMN).map(|_| rng.next() as i16).collect();
    let cells: Vec<GroundId> = (0..FOLD_COLUMN).map(|_| rng.next() as GroundId).collect();
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
    let resident = decode_into::<Refused>(&terrain, vec![0; ELE_COLUMN].leak()).unwrap();
    let want = Separation {
        first_zooms: Default::default(),
        ..tile.separation.clone()
    };
    assert_eq!(resident.separation, want, "its lines");
    assert_eq!(resident.ele, tile.ele, "its samples");
    assert_eq!(resident.occupancy, tile.occupancy);
}

/// A brick's entries are those of the walk of the tile's ways whose bin is the brick's, in
/// their points' order and a point's in its bins', and every brick's together are the walk's.
#[test]
fn brick_pairs_are_the_walk_by_brick() {
    let across = WAYS_BIN_ACROSS;
    for seed in 0..8 {
        let separation = tile(seed).separation;
        let by_brick = BrickPairs::of(&separation).unwrap();
        let mut cnt = 0;
        for place in (0..BRICKS_ACROSS).flat_map(|y| (0..BRICKS_ACROSS).map(move |x| [x, y])) {
            let mut want = Vec::new();
            separation.reaches(f64::from(TILE_CELLS), WAYS_ACROSS, |k, d, _, s| {
                let [x, y] = [k as u32 % WAYS_ACROSS, k as u32 / WAYS_ACROSS];
                if [x / across, y / across] == place {
                    want.push((
                        (y % across) * across + x % across,
                        separation.starts[d] + s as u32,
                    ));
                }
            });
            want.sort_unstable_by_key(|&(b, p)| (p, b));
            assert_eq!(by_brick.at(place), want, "seed {seed}, brick {place:?}");
            cnt += want.len();
        }
        assert!(cnt > 0, "seed {seed}: its lines reach a brick");
    }
}

/// An Earth tile is as long as its texels say.
#[test]
fn earth_pkt_size() {
    let earth = earth::encode(&vec![0; earth::EARTH_SZ]).unwrap();
    assert_eq!(earth.len(), earth::PKT_SZ as usize);
}

#[test]
fn non_tile_pkt_refused() {
    let v = Golden::load(GOLDEN);
    let small = hex::decode(v.s("old_format_tile")).unwrap();
    assert!(decode(&small).is_err());
    assert!(decode_into::<Refused>(&small, vec![0; ELE_COLUMN].leak()).is_err());
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
/// column holds any other count is refused.
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
    for n in [GROUND_COLUMN, FOLD_COLUMN] {
        assert_eq!(cells_of(&pkt(n)).ok().map(<[GroundId]>::len), Some(n));
    }
    for n in [
        0,
        1,
        GROUND_COLUMN - 1,
        GROUND_COLUMN + 1,
        2 * GROUND_COLUMN,
        FOLD_COLUMN + 1,
    ] {
        assert!(cells_of(&pkt(n)).is_err(), "{n} cells");
    }
}
