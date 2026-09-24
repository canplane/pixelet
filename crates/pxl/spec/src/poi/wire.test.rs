// pxl-spec/poi/wire.test.rs

use super::*;

fn node() -> Node {
    let key = xy::parse("132110320000000").unwrap();
    let b = xy::bounds(key);
    Node {
        revision: 17,
        summary: Summary {
            key,
            count: 1_000_000,
            bounds: [b[0], b[1], -20.0, b[0] + 100.0, b[1] + 100.0, 800.0],
            refinable: false,
        },
        children: vec![],
    }
}

#[test]
fn summary_wire_is_independent_of_population_and_preserves_xyz() {
    let n = node();
    let a = encode(&n).unwrap();
    let mut larger = n.clone();
    larger.summary.count = u64::MAX;
    let b = encode(&larger).unwrap();
    assert_eq!(a.len(), b.len());
    assert!(b.len() <= MAX_PACKET_BYTES);
    assert_eq!(decode(&a).unwrap(), n);
    assert_eq!(decode(&b).unwrap(), larger);
}

#[test]
fn malformed_or_incomplete_refinement_is_not_an_empty_population() {
    let n = node();
    let a = encode(&n).unwrap();
    for len in 0..a.len() {
        assert!(decode(&a[..len]).is_err());
    }
    let mut extra = a.clone();
    extra.push(0);
    assert!(decode(&extra).is_err());
    let mut invalid = n.clone();
    invalid.summary.bounds[2] = f64::NAN;
    assert!(encode(&invalid).is_err());
    invalid = n;
    invalid.summary.refinable = true;
    assert!(encode(&invalid).is_err());
}

#[test]
fn xy_transport_partitions_boundaries_without_duplicate_ownership() {
    let key = xy::parse("132110320000000").unwrap();
    let b = xy::bounds(key);
    let center = [(b[0] + b[2]) * 0.5, (b[1] + b[3]) * 0.5, 100.0];
    let child = xy::key_at(center, 16).unwrap();
    assert_eq!(child, key * 4 + 1);
    assert_eq!(xy::parse(&xy::name(child)), Some(child));
    assert!(xy::contains(key, child));
    assert_eq!(xy::key_at([center[0], center[1], -1000.0], 16), Some(child));
}
