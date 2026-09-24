// pxl-spec/poi/detail.test.rs

use super::*;

#[test]
fn original_unicode_and_xyz_round_trip_and_malformed_packets_are_rejected() {
    let page = Page {
        revision: 7,
        key: xy::parse("132110320123012").unwrap(),
        offset: 16,
        total: 18,
        records: vec![Entity {
            id: "provider:1".into(),
            position: [1.0, 2.0, -12.5],
            text: "강남역 café 👩🏽‍💻".into(),
        }],
    };
    let bytes = encode(&page).unwrap();
    assert_eq!(decode(&bytes).unwrap(), page);
    assert_eq!(page.next(), Some(17));
    assert_eq!(parse(&name(page.key, page.offset)), Some((page.key, 16)));
    for end in 0..bytes.len() {
        assert!(decode(&bytes[..end]).is_err());
    }
    let mut extra = bytes.clone();
    extra.push(0);
    assert!(decode(&extra).is_err());
    let mut bad = page;
    bad.records = vec![bad.records[0].clone(); PAGE_RECORDS + 1];
    assert!(encode(&bad).is_err());
    assert!(parse("d132110320123012-../source.jsonl").is_none());
}
