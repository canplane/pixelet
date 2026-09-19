// pxl-spec/bake/poi_page.test.rs

use std::hash::Hasher;

use fnv::FnvHasher;

use super::*;

fn poi(i: u32, category: &str) -> Poi {
    Poi {
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
    let pkt = encode_poi_page(&pois).unwrap();
    let page = PoiPage::new(&pkt).unwrap();
    assert_eq!(page.len, 4);
    assert_eq!(page.x, [0.0, 10.5, 21.0, 31.5]);
    assert_eq!(page.y, [0.0, -2.0, -4.0, -6.0]);
    assert_eq!(page.categories, ["food", "shop", "park"]);
    assert_eq!(page.category, [0, 1, 0, 2]);
    assert_eq!(page.rank, [0, 100, 200, 300]);
    let mut h = FnvHasher::default();
    h.write(b"gers-2");
    assert_eq!(page.id_hash[2], h.finish());
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
    let page = PoiPage::new(&pkt).unwrap();
    let within = |lane: *const u8| {
        let base = pkt.as_ptr() as usize;
        (lane as usize) >= base && (lane as usize) < base + pkt.len()
    };
    assert!(within(page.x.as_ptr().cast()));
    assert!(within(page.id_hash.as_ptr().cast()));

    let empty_pkt = encode_empty_poi_page().unwrap();
    let empty = PoiPage::new(&empty_pkt).unwrap();
    assert_eq!(empty.len, 0);
    assert!(empty.categories.is_empty());
}

#[test]
fn poi_page_a_rank_past_u16_saturates_rather_than_wrapping() {
    let pkt = encode_poi_page(&[Poi { rank: 1_000_000, ..poi(1, "x") }]).unwrap();
    let page = PoiPage::new(&pkt).unwrap();
    assert_eq!(page.rank[0], 0xffff);
}
