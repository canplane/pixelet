// pxl-spec/bake/format.test.rs

use xpute_core::golden::Golden;

use super::*;
use crate::bake::pack::pack;

const GOLDEN: &str = include_str!("../../golden/bake/format.tsv");

#[test]
fn the_formats_write_and_read_what_the_record_holds() {
    let v = Golden::load(GOLDEN);
    let ele: Vec<i16> = (0..v.len("ele")).map(|j| v.i32(&format!("ele.{j}")) as i16).collect();
    // The golden holds a channel's numbers by that channel's own identity;
    // the page is one array of cells and each names all four.
    let ch = |j: usize| -> Vec<u8> { hex::decode(v.s(&format!("ground.{j}"))).unwrap() };
    let numbers = GROUND_CH_KEYS.map(|k| ch(k as usize));
    let ground = Ground {
        cells: (0..numbers[0].len()).map(|i| pack(GROUND_CH_KEYS.map(|k| numbers[ch_index(k)][i]))).collect::<Vec<_>>().into(),
    };
    let extent: Vec<i16> = (0..v.len("extent")).map(|j| v.i32(&format!("extent.{j}")) as i16).collect();
    let page = Terrain {
        ele: ele.clone().into(),
        ground,
        index: Index {
            extent: extent.as_chunks::<2>().0.to_vec().into(),
            occupancy: Occupancy::of_bits(v.u32("occupancy") as u8).unwrap(),
        },
    };
    let terrain = encode_terrain(&page).unwrap();
    assert_eq!(hex::encode(&terrain), v.s("terrain"));
    let decoded = decode_terrain(&terrain).unwrap();
    assert_eq!(decoded, page, "a page reads back as it was written");
    for key in GROUND_CH_KEYS {
        let j = key as u8;
        let read: Vec<u8> = (0..decoded.ground.len()).map(|i| decoded.ground.channel(i, key)).collect();
        assert_eq!(hex::encode(&read), v.s(&format!("decoded.{j}")), "decoded.{j}");
    }
    // Into a record's room, as the client takes it: the same page.
    let (mut cells, mut ele_room, mut extent_room) = (vec![0; page.ground.len()], vec![0; ele.len()], vec![[0; 2]; page.index.extent.len()]);
    let room = PageRoom {
        cells: &mut cells,
        ele: &mut ele_room,
        extent: &mut extent_room,
    };
    assert_eq!(decode_terrain_into(&terrain, room).unwrap(), page.index.occupancy);
    assert_eq!((cells.as_slice(), ele_room.as_slice(), extent_room.as_slice()), (&*page.ground.cells, &*page.ele, &*page.index.extent));
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
