// xpute-core/codec/json.test.rs
// (no pair: json.rs is the host's JSON, ported for the records' tags)

use super::*;
use crate::golden::Golden;

const GOLDEN: &str = include_str!("../../golden/codec/json.tsv");

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
    if v.has(key) {
        String::from_utf8_lossy(&hex::decode(v.s(key)).unwrap()).into_owned()
    } else {
        String::new()
    }
}

fn list(v: &Golden, base: &str) -> Vec<String> {
    if !v.has(&format!("{base}.0")) {
        return Vec::new();
    }
    (0..v.len(base)).map(|j| v.s(&format!("{base}.{j}")).to_string()).collect()
}

#[test]
fn json_writes_and_reads_what_the_host_does() {
    let v = Golden::load(GOLDEN);
    v.each("stringify", |k| {
        let keys = list(&v, &format!("{k}.keys"));
        let values = list(&v, &format!("{k}.values"));
        let map: StrMap = keys
            .iter()
            .zip(values.iter())
            .map(|(key, val)| {
                (
                    String::from_utf8_lossy(&hex::decode(key).unwrap()).into_owned(),
                    if val == "undefined" {
                        None
                    } else {
                        Some(String::from_utf8_lossy(&hex::decode(val).unwrap()).into_owned())
                    },
                )
            })
            .collect();
        assert_eq!(stringify_str_map(&map), text(&v, &format!("{k}.json")), "{k}");
    });
    v.each("parse", |k| {
        let got = match parse_str_map(&text(&v, &format!("{k}.text"))) {
            Err(_) => "refused".to_string(),
            Ok(m) if m.is_empty() => "empty".to_string(),
            Ok(m) => m
                .iter()
                .map(|(key, val)| format!("{}={}", hex::encode(key), hex::encode(val.as_deref().unwrap())))
                .collect::<Vec<_>>()
                .join(","),
        };
        assert_eq!(got, v.s(&format!("{k}.read")), "{k}");
    });
}
