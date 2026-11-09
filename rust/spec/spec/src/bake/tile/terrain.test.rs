// pxl-spec/bake/tile/terrain.test.rs

use xpute_kit::golden::Golden;

use super::*;
use crate::bake::tile::earth;
use xpute_kit::math::rng::SplitMix64;
use xpute_kit::status::errno::Errno;

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
    let cells: Vec<GroundId> = (0..PALETTE_COLUMN)
        .map(|_| rng.next() as GroundId)
        .collect();
    TerrainTile::of(
        ele,
        cells.into(),
        lines(&mut rng),
        Occupancy::of_bits(0b10101).unwrap(),
    )
}

const ROOM_LINES: usize = 438;
const ROOM_POINTS: usize = 4193;

fn room() -> separation::Room {
    separation::Room {
        starts: vec![0; ROOM_LINES + 1].leak(),
        points: vec![[0; 2]; ROOM_POINTS].leak(),
        tops: vec![0; ROOM_POINTS].leak(),
        widths: vec![0; ROOM_LINES].leak(),
        kinds: vec![0; ROOM_LINES].leak(),
        forms: vec![0; ROOM_LINES].leak(),
        clears: vec![0; ROOM_LINES].leak(),
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
    let resident = decode_into(&terrain, room(), vec![0; ELE_COLUMN].leak()).unwrap();
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

/// A tile whose lines are past a client's room is refused, not cut.
#[test]
fn lines_past_the_room_refused() {
    let tile = tile(7);
    let long = Separation {
        starts: (0..=ROOM_LINES as u32 + 1).collect::<Vec<_>>().into(),
        points: vec![[0; 2]; ROOM_LINES + 1].into(),
        tops: vec![0; ROOM_LINES + 1].into(),
        widths: vec![1; ROOM_LINES + 1].into(),
        kinds: vec![0; ROOM_LINES + 1].into(),
        forms: vec![0; ROOM_LINES + 1].into(),
        clears: vec![0; ROOM_LINES + 1].into(),
        first_zooms: vec![0; ROOM_LINES + 1].into(),
    };
    let long = encode(&TerrainTile {
        separation: long,
        ..tile
    })
    .unwrap();
    let err = decode_into(&long, room(), vec![0; ELE_COLUMN].leak()).unwrap_err();
    assert_eq!(err.errnum, Errno::EFBIG);
}

#[test]
fn non_tile_pkt_refused() {
    let v = Golden::load(GOLDEN);
    let small = hex::decode(v.s("old_format_tile")).unwrap();
    assert!(decode(&small).is_err());
    let room = room();
    assert!(decode_into(&small, room, vec![0; ELE_COLUMN].leak()).is_err());
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
