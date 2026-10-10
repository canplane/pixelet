// pxl_shared/bake/poi_page.test.rs

use xpute_core::kit::hash::fnv1a64_str;

use super::*;

fn poi(i: u32, category: &str) -> POI {
    POI {
        id: format!("gers-{i}"),
        r#type: "poi",
        category: category.to_string(),
        name: format!("장소 {i} Place"),
        center: [i as f64 * 10.5, -(i as f64) * 2.0, 0.0],
        tags: vec![
            ("addr".to_string(), Some(format!("street {i}"))),
            ("phone".to_string(), if i % 2 == 1 { Some("010".to_string()) } else { None }),
        ],
        rank: i * 100,
    }
}

#[test]
fn poi_page_lanes_read_in_place_strings_read_on_demand_every_field_round_trips() {
    let pois = [poi(0, "food"), poi(1, "shop"), poi(2, "food"), poi(3, "park")];
    let page = PoiPage::new(encode_poi_page(&pois).unwrap()).unwrap();
    assert_eq!(page.len, 4);
    assert_eq!(page.x.as_slice(), &[0.0, 10.5, 21.0, 31.5]);
    assert_eq!(page.y.as_slice(), &[0.0, -2.0, -4.0, -6.0]);
    assert_eq!(page.categories, ["food", "shop", "park"]);
    assert_eq!(page.category.as_slice(), &[0, 1, 0, 2]);
    assert_eq!(page.rank.as_slice(), &[0, 100, 200, 300]);
    assert_eq!(page.id_hash[2], fnv1a64_str("gers-2"));
    assert_eq!(page.name_of(1).unwrap(), "장소 1 Place");
    assert_eq!(page.id_of(3).unwrap(), "gers-3");
    assert_eq!(
        page.tags_of(1).unwrap(),
        vec![("addr".to_string(), Some("street 1".to_string())), ("phone".to_string(), Some("010".to_string()))]
    );
    let mut want = pois[2].clone();
    want.tags = vec![("addr".to_string(), Some("street 2".to_string()))];
    assert_eq!(page.record(2).unwrap(), want);
}

#[test]
fn poi_page_the_lanes_are_views_into_the_packet_and_an_empty_page_has_length_zero() {
    let pkt = encode_poi_page(&[poi(7, "food")]).unwrap();
    let page = PoiPage::new(pkt.clone()).unwrap();
    assert!(page.x.buffer().is(pkt.buffer()));
    assert!(page.id_hash.buffer().is(pkt.buffer()));
    let empty = PoiPage::new(encode_empty_poi_page().unwrap()).unwrap();
    assert_eq!(empty.len, 0);
    assert!(empty.categories.is_empty());
}

#[test]
fn poi_page_a_rank_past_u16_saturates_rather_than_wrapping() {
    let page = PoiPage::new(encode_poi_page(&[POI { rank: 1_000_000, ..poi(1, "x") }]).unwrap()).unwrap();
    assert_eq!(page.rank[0], 0xffff);
}
