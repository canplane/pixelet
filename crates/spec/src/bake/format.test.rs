// pxl-spec/bake/format.test.rs

use xpute_core::golden::Golden;

use super::*;
use xpute_core::math::rng::SplitMix64;

const GOLDEN: &str = include_str!("../../golden/bake/format.tsv");

/// So a tile's lanes are the same on every run.
struct Rng(SplitMix64);

impl Rng {
    fn next(&mut self) -> u64 {
        u64::from(self.0.next_u32()) << 32 | u64::from(self.0.next_u32())
    }
}

/// Random lanes, each index cell's lowest under its highest.
fn tile(seed: u64) -> Terrain {
    let mut rng = Rng(SplitMix64::new(seed));
    let ele: Vec<i16> = (0..ELE_LANE).map(|_| rng.next() as i16).collect();
    let cells: Vec<GroundCell> = (0..GROUND_LANE).map(|_| rng.next() as GroundCell).collect();
    let extent: Vec<[i16; 2]> = (0..EXTENT_LANE)
        .map(|_| {
            let (a, b) = (rng.next() as i16, rng.next() as i16);
            [a.min(b), a.max(b)]
        })
        .collect();
    let mut decks = deck::Builder::default();
    for _ in 0..3 {
        let points: Vec<([f64; 2], i16)> = (0..4).map(|_| ([(rng.next() % 600) as f64, (rng.next() % 600) as f64], rng.next() as i16)).collect();
        decks.push(points, 1 + (rng.next() % 200) as u8, rng.next() as u8, deck::Form::of((rng.next() % 3) as u8), rng.next() as u8);
    }
    Terrain {
        ele: ele.into(),
        ground: Ground { cells: cells.into() },
        decks: decks.finish(),
        index: Index::new(extent, Occupancy::of_bits(0b10101).unwrap()),
    }
}

fn room() -> TileRoom {
    TileRoom {
        cells: vec![0; GROUND_LANE].leak(),
        ele: vec![0; ELE_LANE].leak(),
        extent: vec![[0; 2]; EXTENT_LANE].leak(),
        coarse: vec![[0; 2]; COARSE_LANE].leak(),
        decks: deck::Room {
            starts: vec![0; deck::MAX_DECKS + 1].leak(),
            points: vec![[0; 2]; deck::MAX_POINTS].leak(),
            tops: vec![0; deck::MAX_POINTS].leak(),
            width: vec![0; deck::MAX_DECKS].leak(),
            kind: vec![0; deck::MAX_DECKS].leak(),
            form: vec![0; deck::MAX_DECKS].leak(),
            clear: vec![0; deck::MAX_DECKS].leak(),
            boxes: vec![[0; 4]; deck::MAX_BOXES].leak(),
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
    let terrain = encode_terrain(&tile).unwrap();
    assert_eq!(format!("{:016x}", fnv(&terrain)), v.s("terrain.fnv"), "the tile's bytes");
    assert_eq!(decode_terrain(&terrain).unwrap(), tile, "a tile reads back as it was written");
    let room = room();
    assert_eq!(decode_terrain_into(&terrain, room).unwrap(), tile, "a tile reads into its room as it was written");
}

#[test]
fn a_packet_that_is_not_a_tile_is_refused() {
    let v = Golden::load(GOLDEN);
    let small = hex::decode(v.s("not_a_page")).unwrap();
    assert!(decode_terrain(&small).is_err());
    let room = room();
    assert!(decode_terrain_into(&small, room).is_err());
}

/// Every index block is exactly the join of its cells, at every size.
#[test]
fn a_block_says_what_its_cells_say() {
    for seed in [1, 0x2545_f491, 0x9e37_79b9_7f4a_7c15] {
        let index = tile(seed).index;
        let n = EXTENT.tile_cells();
        for j in 0..=n.trailing_zeros() {
            let side = 1 << j;
            for y in (0..n).step_by(side as usize) {
                for x in (0..n).step_by(side as usize) {
                    let lane = index.extent();
                    let cells = (y..y + side).flat_map(|cy| (x..x + side).map(move |cx| lane[EXTENT.cell_index(cx, cy) as usize]));
                    let lo = cells.clone().map(|c| drawn(c)[0]).min().unwrap();
                    let hi = cells.map(|c| drawn(c)[1]).max().unwrap();
                    assert_eq!(index.over([x, y], side), [lo, hi], "{side} from ({x}, {y})");
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
