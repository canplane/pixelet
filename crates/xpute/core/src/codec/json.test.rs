// xpute-core/codec/json.test.rs

use super::*;
use crate::golden::Golden;

const GOLDEN: &str = include_str!("../../../../../spec/xpute/golden/codec/json.tsv");

#[test]
fn json_read_in_place_walks_what_the_text_holds_and_refuses_what_is_not_json() {
    let doc = read_json(r#" {"a": [1, -2.5e1, true, null, "x\"y"], "b": {"k": "\uD83D\uDE00 \u00e9\n"}, "a": "last"} "#).unwrap();
    assert_eq!(doc.kind(), JsonKind::Object);
    assert!(doc.get("a").unwrap().as_str().unwrap().eq_str("last"), "a key given twice reads as its last");
    let first = doc.fields().next().unwrap().1;
    let items: Vec<JsonKind> = first.items().map(|v| v.kind()).collect();
    assert_eq!(items, [JsonKind::Number, JsonKind::Number, JsonKind::Bool, JsonKind::Null, JsonKind::String]);
    assert_eq!(first.at(1).unwrap().as_f64(), Some(-25.0));
    assert_eq!(first.at(2).unwrap().as_bool(), Some(true));
    assert!(first.at(3).unwrap().is_null());
    let quoted = first.at(4).unwrap().as_str().unwrap();
    assert_eq!((quoted.as_plain(), quoted.to_string()), (None, "x\"y".to_string()));
    let k = doc.get("b").unwrap().get("k").unwrap().as_str().unwrap();
    assert!(k.eq_str("😀 é\n"), "a surrogate pair is one character");
    assert_eq!(doc.get("missing").map(|v| v.kind()), None);
    assert_eq!(read_json("[]").unwrap().items().count(), 0);
    assert_eq!(read_json(r#""\ud800""#).unwrap().as_str().unwrap().to_string(), "\u{fffd}", "a lone surrogate");
    for bad in ["", "[1,]", "{\"a\" 1}", "\"open", "\"a\u{1}\"", "\"\\q\"", "1 2", "{\"a\":1,}", "tru"] {
        assert!(read_json(bad).is_none(), "{bad:?} is not JSON");
    }
    let deep = "[".repeat(200) + &"]".repeat(200);
    assert!(read_json(&deep).is_none(), "nesting past the stack's depth");
}

fn text(v: &Golden, key: &str) -> String {
    String::from_utf8_lossy(&hex::decode(v.s(key)).unwrap()).into_owned()
}

#[test]
fn json_is_what_the_host_s_json_parse_takes() {
    let v = Golden::load(GOLDEN);
    v.each("grammar", |k| {
        let got = if read_json(&text(&v, &format!("{k}.text"))).is_some() { "json" } else { "refused" };
        assert_eq!(got, v.s(&format!("{k}.read")), "{k}");
    });
}
