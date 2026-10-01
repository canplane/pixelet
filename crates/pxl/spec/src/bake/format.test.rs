// pxl-spec/bake/format.test.rs

use xpute_core::golden::Golden;

use super::*;

const GOLDEN: &str = include_str!("../../golden/bake/format.tsv");

/// xorshift64: a page's lanes the same on every run.
struct Rng(u64);

impl Rng {
    fn next(&mut self) -> u64 {
        self.0 ^= self.0 << 13;
        self.0 ^= self.0 >> 7;
        self.0 ^= self.0 << 17;
        self.0
    }
}

/// A page of lanes drawn from `seed`: the ground's words and the heights
/// anything, and each index cell a lowest under its highest.
fn page(seed: u64) -> Terrain {
    let mut rng = Rng(seed);
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
        decks.push(points, 1 + (rng.next() % 200) as u8, rng.next() as u8, deck::Form::of((rng.next() % 2) as u8));
    }
    Terrain {
        ele: ele.into(),
        ground: Ground { cells: cells.into() },
        decks: decks.finish(),
        index: Index::new(extent, Occupancy::of_bits(0b10101).unwrap()),
    }
}

/// A record's room, as the kernel's fixed range would give it.
fn room() -> PageRoom {
    PageRoom {
        cells: vec![0; GROUND_LANE].leak(),
        ele: vec![0; ELE_LANE].leak(),
        extent: vec![[0; 2]; EXTENT_LANE].leak(),
        blocks: vec![Block::default(); BLOCK_LANE].leak(),
        decks: deck::Room {
            starts: vec![0; deck::MAX_DECKS + 1].leak(),
            points: vec![[0; 2]; deck::MAX_POINTS].leak(),
            tops: vec![0; deck::MAX_POINTS].leak(),
            width: vec![0; deck::MAX_DECKS].leak(),
            kind: vec![0; deck::MAX_DECKS].leak(),
            form: vec![0; deck::MAX_DECKS].leak(),
        },
    }
}

/// FNV-1a over 64 bits: what the golden holds of a page's bytes, which are
/// a megabyte.
fn fnv(bytes: &[u8]) -> u64 {
    bytes.iter().fold(0xcbf2_9ce4_8422_2325, |h, &b| (h ^ u64::from(b)).wrapping_mul(0x0100_0000_01b3))
}

/// A page is written as the golden holds it and reads back as it was
/// written, in hand and into a record's room alike.
#[test]
fn the_formats_write_and_read_what_the_record_holds() {
    let v = Golden::load(GOLDEN);
    let page = page(0x9e37_79b9_7f4a_7c15);
    let terrain = encode_terrain(&page).unwrap();
    assert_eq!(format!("{:016x}", fnv(&terrain)), v.s("terrain.fnv"), "the page's bytes");
    assert_eq!(decode_terrain(&terrain).unwrap(), page, "a page reads back as it was written");
    let room = room();
    assert_eq!(decode_terrain_into(&terrain, room).unwrap(), page, "a page reads into its room as it was written");
}

/// A packet whose lanes are not a page's is no page, in hand or into a room.
#[test]
fn a_packet_that_is_not_a_page_is_refused() {
    let v = Golden::load(GOLDEN);
    let small = hex::decode(v.s("not_a_page")).unwrap();
    assert!(decode_terrain(&small).is_err());
    let room = room();
    assert!(decode_terrain_into(&small, room).is_err());
}

/// Every block of an index says exactly what its cells say together: the
/// lowest of their lowest, the highest of their highest, and the widest any
/// one spans — at every size from one cell to the page.
#[test]
fn a_block_says_what_its_cells_say() {
    for seed in [1, 0x2545_f491, 0x9e37_79b9_7f4a_7c15] {
        let index = page(seed).index;
        let n = EXTENT.page_cells();
        for j in 0..=n.trailing_zeros() {
            let side = 1 << j;
            for y in (0..n).step_by(side as usize) {
                for x in (0..n).step_by(side as usize) {
                    let lane = index.extent();
                    let cells = (y..y + side).flat_map(|cy| (x..x + side).map(move |cx| drawn(lane[EXTENT.cell_index(cx, cy) as usize])));
                    let lo = cells.clone().map(|[l, _]| l).min().unwrap();
                    let hi = cells.clone().map(|[_, h]| h).max().unwrap();
                    let span = cells.map(|[l, h]| (i32::from(h) - i32::from(l)) as u16).max().unwrap();
                    assert_eq!(index.over([x, y], side), Block { lo, hi, span }, "{side} from ({x}, {y})");
                }
            }
        }
    }
}

/// Every byte a page can carry names an occupancy and reads back as it, and
/// no other byte is taken: a child without its page, or a bit past the five.
#[test]
fn an_occupancy_is_a_page_and_its_children_or_nothing() {
    for bits in 0..=u8::MAX {
        match Occupancy::of_bits(bits) {
            Some(o) => {
                assert_eq!(o.bits(), bits);
                let children: [bool; 4] = std::array::from_fn(|q| o.has_child(q));
                assert!(o.is_page() || !children.contains(&true), "{bits:#b}: a child under the zero page");
            }
            None => assert!(bits >= 32 || (bits != 0 && bits & 16 == 0), "{bits:#b} refused"),
        }
    }
    let of = |pages: [bool; 4]| Occupancy::of_children(pages.map(|p| if p { Occupancy::LEAF } else { Occupancy::ZERO }));
    assert_eq!(of([false; 4]), Occupancy::ZERO, "a parent of the zero page alone is the zero page");
    let some = of([true, false, true, false]);
    assert!(some.is_page() && some.has_child(0) && !some.has_child(1) && some.has_child(2) && !some.has_child(3));
}
