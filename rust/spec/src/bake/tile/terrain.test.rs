// pxl-spec/bake/tile/terrain.test.rs

use xpute_kit::golden::Golden;

use super::*;
use crate::bake::tile::earth;
use xpute_kit::math::rng::SplitMix64;

const GOLDEN: &str = include_str!("../../../golden/bake/terrain.tsv");

/// So a tile's lanes are the same on every run.
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
    let cells: Vec<GroundId> = (0..LOOK_LANE).map(|_| rng.next() as GroundId).collect();
    TerrainTile::of(
        ele,
        Ground {
            cells: cells.into(),
        },
        lines(&mut rng),
        Occupancy::of_bits(0b10101).unwrap(),
    )
}

fn room() -> separation::Room {
    separation::Room {
        starts: vec![0; separation::MAX_LINES + 1].leak(),
        points: vec![[0; 2]; separation::MAX_POINTS].leak(),
        tops: vec![0; separation::MAX_POINTS].leak(),
        width: vec![0; separation::MAX_LINES].leak(),
        kind: vec![0; separation::MAX_LINES].leak(),
        form: vec![0; separation::MAX_LINES].leak(),
        clear: vec![0; separation::MAX_LINES].leak(),
    }
}

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
    let resident = decode_into(&terrain, room()).unwrap();
    let held = Separation {
        coarsest: Default::default(),
        ..tile.separation.clone()
    };
    assert_eq!(resident.separation, held, "its lines");
    assert_eq!(resident.occupancy, tile.occupancy);
}

/// A tile at every lane's longest is as long as `MAX_PKT_BYTES` says.
#[test]
fn full_tile_pkt_size() {
    use separation::{MAX_LINES, MAX_POINTS};
    let starts: Vec<u32> = (0..=MAX_LINES)
        .map(|d| (d * MAX_POINTS / MAX_LINES) as u32)
        .collect();
    let full = Separation {
        starts: starts.into(),
        points: vec![[0; 2]; MAX_POINTS].into(),
        tops: vec![0; MAX_POINTS].into(),
        width: vec![1; MAX_LINES].into(),
        kind: vec![0; MAX_LINES].into(),
        form: vec![0; MAX_LINES].into(),
        clear: vec![0; MAX_LINES].into(),
        coarsest: vec![0; MAX_POINTS].into(),
    };
    let tile = TerrainTile {
        separation: full,
        ..tile(7)
    };
    assert_eq!(encode(&tile).unwrap().len(), MAX_PKT_BYTES);
    let earth = earth::encode(&vec![0; earth::EARTH_BYTES]).unwrap();
    assert_eq!(
        earth.len(),
        earth::PKT_BYTES,
        "and an Earth tile is as long as its texels say"
    );
}

#[test]
fn non_tile_pkt_refused() {
    let v = Golden::load(GOLDEN);
    let small = hex::decode(v.s("old_format_tile")).unwrap();
    assert!(decode(&small).is_err());
    let room = room();
    assert!(decode_into(&small, room).is_err());
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
