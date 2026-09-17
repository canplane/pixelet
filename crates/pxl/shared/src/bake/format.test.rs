// pxl_shared/bake/format.test.rs
// (no pair: format.ts's tests are poi_page.test.ts, mirrored beside this)

use xpute_core::kit::array::TypedArray;
use xpute_core::kit::encoding::{bytes_to_hex, hex_to_bytes, td};
use xpute_core::kit::json::stringify_str_map;
use xpute_core::sys::vector::Vector;

use super::*;

const VECTOR: &str = include_str!("../../vector/bake/format.tsv");

fn text(v: &Vector, key: &str) -> String {
    if v.has(key) {
        td.decode(&hex_to_bytes(v.s(key)))
    } else {
        String::new()
    }
}

#[test]
fn the_formats_write_and_read_what_the_typescript_does() {
    let v = Vector::load(VECTOR);
    let ele: I16Array = TypedArray::from(&(0..v.len("ele")).map(|j| v.i32(&format!("ele.{j}")) as i16).collect::<Vec<_>>());
    let ch = |j: usize| hex_to_bytes(v.s(&format!("ground.{j}")));
    let ground = Ground {
        land: ch(0),
        water: ch(1),
        structure: ch(2),
        transport: ch(3),
    };
    let terrain = encode_terrain(&ele, &ground).unwrap();
    assert_eq!(bytes_to_hex(&terrain), v.s("terrain"));
    let decoded = decode_terrain(terrain).unwrap();
    assert_eq!(decoded.ele.as_slice(), ele.as_slice());
    for (j, key) in GROUND_CH_KEYS.into_iter().enumerate() {
        assert_eq!(bytes_to_hex(decoded.ground.get(key)), v.s(&format!("decoded.{j}")), "decoded.{j}");
    }

    let pois: Vec<POI> = (0..v.len("pois"))
        .map(|j| {
            let k = format!("pois.{j}");
            let n = if v.has(&format!("{k}.tag_keys.0")) { v.len(&format!("{k}.tag_keys")) } else { 0 };
            // The generator's tags, rebuilt by key; their values are fixed in it.
            let tags = (0..n)
                .map(|t| {
                    let key = text(&v, &format!("{k}.tag_keys.{t}"));
                    let val = match (j, key.as_str()) {
                        (0, "name") => Some("가\nnew".to_string()),
                        (0, "2") => Some("two".to_string()),
                        (0, "z") => None,
                        (0, "10") => Some("ten".to_string()),
                        (2, "") => Some("\\".to_string()),
                        _ => panic!("{k}: unexpected tag {key:?}"),
                    };
                    (key, val)
                })
                .collect();
            POI {
                id: text(&v, &format!("{k}.id")),
                r#type: "poi",
                category: v.s(&format!("{k}.category")).to_string(),
                name: text(&v, &format!("{k}.name")),
                center: [v.f64(&format!("{k}.center.0")), v.f64(&format!("{k}.center.1")), v.f64(&format!("{k}.center.2"))],
                tags,
                rank: v.u32(&format!("{k}.rank")),
            }
        })
        .collect();
    let page_pkt = encode_poi_page(&pois).unwrap();
    assert_eq!(bytes_to_hex(&page_pkt), v.s("page"));
    assert_eq!(bytes_to_hex(&encode_empty_poi_page().unwrap()), v.s("empty"));
    let page = PoiPage::new(page_pkt).unwrap();
    v.each("read", |k| {
        let i: u32 = k.rsplit('.').next().unwrap().parse().unwrap();
        assert_eq!(page.category_of(i), v.s(&format!("{k}.category")), "{k}.category");
        assert_eq!(stringify_str_map(&page.tags_of(i).unwrap()), text(&v, &format!("{k}.tags")), "{k}.tags");
        assert_eq!(page.record(i).unwrap().rank, v.u32(&format!("{k}.rank")), "{k}.rank");
    });
}
