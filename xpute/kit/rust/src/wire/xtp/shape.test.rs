// xpute-kit/wire/xtp/shape.test.rs

use super::*;
use crate::wire::xtp::{MAX_PKT_SZ, NodeView, TreeEncoderConfig, TreeView, encoder};

type Packet = (Vec<u16>, u8, Vec<String>, (Vec<i16>, bool));

const AT: [u16; 3] = [1, 2, 3];
const NAMES: [&str; 2] = ["ada", ""];
const TOPS: [i16; 1] = [-1];

fn val() -> <Packet as Shape>::Write<'static> {
    (&AT, 7, &NAMES, (&TOPS, true))
}

#[test]
fn a_shape_writes_the_bytes_a_tree_does() {
    let mut t = TreeView::new();
    t.branch(Some(&mut |b| {
        b.u16_array(Some(&AT))
            .u8(Some(7))
            .strs(Some(&NAMES))
            .branch(Some(&mut |d| {
                d.i16_array(Some(&TOPS)).bool(Some(true));
            }));
    }));
    let tree = encoder()
        .encode(
            &t,
            TreeEncoderConfig {
                init_cap: 1 << 10,
                max_cap: None,
            },
        )
        .unwrap();
    assert_eq!(encode::<Packet>(val(), MAX_PKT_SZ).unwrap(), tree);
}

#[test]
fn a_packet_reads_back_in_its_shape() {
    let pkt = encode::<Packet>(val(), MAX_PKT_SZ).unwrap();
    let (at, n, names, (tops, on)) = get_deep::<Packet>(&pkt).unwrap();
    assert_eq!((at, n, tops, on), (&AT[..], 7, &TOPS[..], true));
    assert_eq!(names.iter().collect::<Vec<_>>(), NAMES);
}

#[test]
fn another_shape_is_refused() {
    let pkt = encode::<Packet>(val(), MAX_PKT_SZ).unwrap();
    assert!(get_deep::<(Vec<u32>, u8, Vec<String>, (Vec<i16>, bool))>(&pkt).is_err());
    assert!(get_deep::<(Vec<u16>, u16, Vec<String>, (Vec<i16>, bool))>(&pkt).is_err());
    assert!(get_deep::<(Vec<u16>, u8, Vec<String>)>(&pkt).is_err());
}

#[test]
fn a_packet_past_its_cap_overflows() {
    let e = encode::<Packet>(val(), 32).unwrap_err();
    assert_eq!(e.errnum, Errno::EOVERFLOW);
}
