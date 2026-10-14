// pxl-spec/bake/tile/terrain.test.rs

use xpute_kit::golden::Golden;

use super::*;
use pxl_portable::ground::bound_of;
use xpute_kit::math::rng::SplitMix64;

const GOLDEN: &str = include_str!("../../../golden/bake/terrain.tsv");

/// So a tile's lanes are the same on every run.
struct Rng(SplitMix64);

impl Rng {
    fn next(&mut self) -> u64 {
        u64::from(self.0.next_u32()) << 32 | u64::from(self.0.next_u32())
    }
}

/// Random lanes.
fn tile(seed: u64) -> TerrainTile {
    let mut rng = Rng(SplitMix64::new(seed));
    let heights: Vec<i16> = (0..HEIGHT_LANE).map(|_| rng.next() as i16).collect();
    let cells: Vec<GroundId> = (0..GROUND_LANE).map(|_| rng.next() as GroundId).collect();
    let mut separation = separation::Builder::default();
    for _ in 0..3 {
        let points: Vec<([f64; 2], i16)> = (0..4).map(|_| ([(rng.next() % 600) as f64, (rng.next() % 600) as f64], rng.next() as i16)).collect();
        separation.push(points, 1 + (rng.next() % 200) as u8, rng.next() as u8, separation::Form::of((rng.next() % 3) as u8), rng.next() as u8);
    }
    TerrainTile::of(heights, Ground { cells: cells.into() }, separation.finish(), Occupancy::of_bits(0b10101).unwrap())
}

fn room() -> TileRoom {
    TileRoom {
        ground: vec![0; GROUND_LANE].leak(),
        heights: vec![0; HEIGHT_LANE].leak(),
        ways: vec![[0; 2]; WAYS_LANE].leak(),
        levels: vec![0; LEVEL_LANE].leak(),
        separation: separation::Room {
            starts: vec![0; separation::MAX_LINES + 1].leak(),
            points: vec![[0; 2]; separation::MAX_POINTS].leak(),
            tops: vec![0; separation::MAX_POINTS].leak(),
            width: vec![0; separation::MAX_LINES].leak(),
            kind: vec![0; separation::MAX_LINES].leak(),
            form: vec![0; separation::MAX_LINES].leak(),
            clear: vec![0; separation::MAX_LINES].leak(),
            boxes: vec![[0; 4]; separation::MAX_BOXES].leak(),
        },
    }
}

/// FNV-1a 64 of a tile's bytes, for the golden.
fn fnv(bytes: &[u8]) -> u64 {
    let mut h = fnv::FnvHasher::default();
    std::hash::Hasher::write(&mut h, bytes);
    std::hash::Hasher::finish(&h)
}

/// A tile round-trips as the golden holds it, in hand and into a record's room.
#[test]
fn the_formats_write_and_read_what_the_record_holds() {
    let v = Golden::load(GOLDEN);
    let tile = tile(0x9e37_79b9_7f4a_7c15);
    let terrain = encode(&tile).unwrap();
    assert_eq!(format!("{:016x}", fnv(&terrain)), v.s("terrain.fnv"), "the tile's bytes");
    assert_eq!(decode(&terrain).unwrap(), tile, "a tile reads back as it was written");
    let room = room();
    assert_eq!(decode_into(&terrain, room).unwrap(), tile, "a tile reads into its room as it was written");
}

#[test]
fn a_packet_that_is_not_a_tile_is_refused() {
    let v = Golden::load(GOLDEN);
    let small = hex::decode(v.s("not_a_page")).unwrap();
    assert!(decode(&small).is_err());
    let room = room();
    assert!(decode_into(&small, room).is_err());
}

/// Every entry of every level is exactly the lowest and highest its cells reach.
#[test]
fn a_level_says_what_its_cells_say() {
    for seed in [1, 0x2545_f491, 0x9e37_79b9_7f4a_7c15] {
        let tile = tile(seed);
        let (heights, ways) = (&tile.heights, &tile.ways);
        for j in 1..=BOUND_LEVELS {
            let (side, n) = (1u32 << j, TILE_CELLS >> j);
            for y in 0..n {
                for x in 0..n {
                    let cells = (y * side..(y + 1) * side).flat_map(|cy| (x * side..(x + 1) * side).map(move |cx| cell_bound(heights, ways, cx, cy)));
                    let (lo, hi) = (cells.clone().map(|c| c[0]).min().unwrap(), cells.map(|c| c[1]).max().unwrap());
                    assert_eq!(bound_of(tile.levels[(level_at(j) + y * n + x) as usize]), [lo, hi], "level {j} at ({x}, {y})");
                }
            }
        }
    }
}

/// Every valid occupancy byte round-trips and no other is accepted.
#[test]
fn an_occupancy_is_a_tile_and_its_children_or_nothing() {
    for bits in 0..=u8::MAX {
        match Occupancy::of_bits(bits) {
            Some(o) => {
                assert_eq!(o.bits(), bits);
                let children: [bool; 4] = std::array::from_fn(|q| o.has_child(q));
                assert!(o.is_tile() || !children.contains(&true), "{bits:#b}: a child under the zero tile");
            }
            None => assert!(bits >= 32 || (bits != 0 && bits & 16 == 0), "{bits:#b} refused"),
        }
    }
    let of = |tiles: [bool; 4]| Occupancy::of_children(tiles.map(|p| if p { Occupancy::LEAF } else { Occupancy::ZERO }));
    assert_eq!(of([false; 4]), Occupancy::ZERO, "a parent of the zero tile alone is the zero tile");
    let some = of([true, false, true, false]);
    assert!(some.is_tile() && some.has_child(0) && !some.has_child(1) && some.has_child(2) && !some.has_child(3));
}
