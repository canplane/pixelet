// xpute_core/kit/json.test.rs
// (no pair: json.rs is the host's JSON, ported for the records' tags)

use super::*;
use crate::kit::encoding::{bytes_to_hex, hex_to_bytes, td, te};
use crate::vector::Vector;

const VECTOR: &str = include_str!("../../vector/kit/json.tsv");

fn text(v: &Vector, key: &str) -> String {
    if v.has(key) {
        td.decode(&hex_to_bytes(v.s(key)))
    } else {
        String::new()
    }
}

fn list(v: &Vector, base: &str) -> Vec<String> {
    if !v.has(&format!("{base}.0")) {
        return Vec::new();
    }
    (0..v.len(base)).map(|j| v.s(&format!("{base}.{j}")).to_string()).collect()
}

#[test]
fn json_writes_and_reads_what_the_host_does() {
    let v = Vector::load(VECTOR);
    v.each("stringify", |k| {
        let keys = list(&v, &format!("{k}.keys"));
        let values = list(&v, &format!("{k}.values"));
        let map: StrMap = keys
            .iter()
            .zip(values.iter())
            .map(|(key, val)| (td.decode(&hex_to_bytes(key)), if val == "undefined" { None } else { Some(td.decode(&hex_to_bytes(val))) }))
            .collect();
        assert_eq!(stringify_str_map(&map), text(&v, &format!("{k}.json")), "{k}");
    });
    v.each("parse", |k| {
        let got = match parse_str_map(&text(&v, &format!("{k}.text"))) {
            Err(_) => "refused".to_string(),
            Ok(m) if m.is_empty() => "empty".to_string(),
            Ok(m) => m
                .iter()
                .map(|(key, val)| format!("{}={}", bytes_to_hex(&te.encode(key)), bytes_to_hex(&te.encode(val.as_deref().unwrap()))))
                .collect::<Vec<_>>()
                .join(","),
        };
        assert_eq!(got, v.s(&format!("{k}.read")), "{k}");
    });
}
