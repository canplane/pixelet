// pxl_shared/record/placement.test.rs
// (no pair: placement.ts's tests are record.test.ts, mirrored beside this)

use super::*;
use xpute_core::kit::encoding::bytes_to_hex;
use xpute_core::sys::vector::Vector;

const VECTOR: &str = include_str!("../../vector/record/placement.tsv");

fn bits(x: f64) -> u64 {
    x.to_bits()
}

#[test]
fn placements_are_what_the_typescript_writes_and_reads() {
    let v = Vector::load(VECTOR);
    let bare = vec![Placement {
        id: 1,
        x: 0.1,
        y: -2.5,
        z: 1e9,
        size: 0.25,
        heading: core::f64::consts::PI,
        value: 228,
        scale: None,
        rotation: None,
        tags: None,
    }];
    let mut full = bare.clone();
    full.push(Placement {
        id: 0xffffffffffffffff,
        x: 3.0,
        y: 4.0,
        z: 5.0,
        size: 2.0,
        heading: -1.0,
        value: 7,
        scale: Some(1.1),
        rotation: Some([0.1, 0.2, 0.3, 0.9]),
        tags: Some(vec![("a".into(), Some("b".into())), ("1".into(), Some("one".into()))]),
    });
    full.push(Placement {
        id: 9,
        x: 0.0,
        y: 0.0,
        z: 0.0,
        size: 1.0,
        heading: 0.0,
        value: 0,
        scale: None,
        rotation: None,
        tags: Some(vec![("gone".into(), None)]),
    });
    assert_eq!(bytes_to_hex(&encode_placements(&bare).unwrap()), v.s("bare"));
    assert_eq!(bytes_to_hex(&encode_placements(&full).unwrap()), v.s("full"));
    for (rows, base) in [(&bare, "read_bare"), (&full, "read_full")] {
        let t = PlacementTable::new(RecordSource::Packet(encode_placements(rows).unwrap())).unwrap();
        v.each(base, |k| {
            let i: u32 = k.rsplit('.').next().unwrap().parse().unwrap();
            let r = t.row(i).unwrap();
            assert_eq!(bits(r.x), v.f64(&format!("{k}.x")).to_bits(), "{k}.x");
            assert_eq!(bits(r.heading), v.f64(&format!("{k}.heading")).to_bits(), "{k}.heading");
            assert_eq!(bits(t.scale_of(i)), v.f64(&format!("{k}.scale")).to_bits(), "{k}.scale");
            match r.rotation {
                None => assert_eq!("none", v.s(&format!("{k}.rotation")), "{k}.rotation"),
                Some(q) => {
                    for (a, x) in q.iter().enumerate() {
                        assert_eq!(bits(*x), v.f64(&format!("{k}.rotation.{a}")).to_bits(), "{k}.rotation.{a}");
                    }
                }
            }
            let tags = r.tags.map_or("none".to_string(), |t| xpute_core::kit::json::stringify_str_map(&t));
            assert_eq!(tags, v.s(&format!("{k}.tags")), "{k}.tags");
        });
    }
}
