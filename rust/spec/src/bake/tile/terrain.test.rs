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
    // coarsest zooms and a grid sweep's boxes.
    let resident = decode_into::<Refused>(&terrain, vec![0; ELE_COLUMN].leak()).unwrap();
    let want = Separation {
        first_zooms: Default::default(),
        ..tile.separation.clone()
    };
    assert_eq!(resident.separation, want, "its lines");
    assert_eq!(resident.ele, tile.ele, "its samples");
    assert_eq!(resident.occupancy, tile.occupancy);
}

/// An Earth tile is as long as its texels say.
#[test]
fn earth_pkt_size() {
    let earth = earth::encode(&vec![0; earth::EARTH_BYTES]).unwrap();
    assert_eq!(earth.len(), earth::PKT_BYTES);
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
