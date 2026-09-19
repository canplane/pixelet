// pxl-spec/bake/format.test.rs
// (no pair: format.ts's tests are poi_page.test.ts, mirrored beside this)

use xpute_core::codec::json::stringify_str_map;
use xpute_core::golden::Golden;

use super::*;

const GOLDEN: &str = include_str!("../../golden/bake/format.tsv");

fn text(v: &Golden, key: &str) -> String {
    if v.has(key) {
        String::from_utf8(hex::decode(v.s(key)).unwrap()).unwrap()
    } else {
        String::new()
    }
}

#[test]
fn the_formats_write_and_read_what_the_record_holds() {
    let v = Golden::load(GOLDEN);
    let ele: Vec<i16> = (0..v.len("ele")).map(|j| v.i32(&format!("ele.{j}")) as i16).collect();
    let ch = |j: usize| -> std::borrow::Cow<'static, [u8]> { hex::decode(v.s(&format!("ground.{j}"))).unwrap().into() };
    let ground = Ground {
        land: ch(0),
        water: ch(1),
        structure: ch(2),
        transport: ch(3),
    };
    let terrain = encode_terrain(&ele, &ground).unwrap();
    assert_eq!(hex::encode(&terrain), v.s("terrain"));
    let decoded = decode_terrain(&terrain).unwrap();
    assert_eq!(decoded.ele.as_ref(), ele.as_slice());
    for (j, key) in GROUND_CH_KEYS.into_iter().enumerate() {
        assert_eq!(hex::encode(decoded.ground.get(key)), v.s(&format!("decoded.{j}")), "decoded.{j}");
    }

    let pois: Vec<Poi> = (0..v.len("pois"))
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
            Poi {
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
    assert_eq!(hex::encode(&page_pkt), v.s("page"));
    assert_eq!(hex::encode(encode_empty_poi_page().unwrap()), v.s("empty"));
    let page = PoiPage::new(&page_pkt).unwrap();
    v.each("read", |k| {
        let i: u32 = k.rsplit('.').next().unwrap().parse().unwrap();
        assert_eq!(page.category_of(i), v.s(&format!("{k}.category")), "{k}.category");
        assert_eq!(stringify_str_map(&page.tags_of(i).unwrap()), text(&v, &format!("{k}.tags")), "{k}.tags");
        assert_eq!(page.record(i).unwrap().rank, v.u32(&format!("{k}.rank")), "{k}.rank");
    });
}
