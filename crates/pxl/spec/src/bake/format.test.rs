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
    let terrain = encode_terrain(&ele, &ground).unwrap();
    assert_eq!(hex::encode(&terrain), v.s("terrain"));
    let decoded = decode_terrain(&terrain).unwrap();
    assert_eq!(decoded.ele.as_ref(), ele.as_slice());
    for key in GROUND_CH_KEYS {
        let j = key as u8;
        let read: Vec<u8> = (0..decoded.ground.len()).map(|i| decoded.ground.channel(i, key)).collect();
        assert_eq!(hex::encode(&read), v.s(&format!("decoded.{j}")), "decoded.{j}");
    }
}
