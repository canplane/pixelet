// pxl_shared/record/shipped.test.rs
// (no pair: shipped.ts's tests are record.test.ts, mirrored beside this)

use super::*;
use xpute_core::kit::hash::fnv1a_64_hex;
use xpute_core::vector::Vector;

const VECTOR: &str = include_str!("../../vector/record/shipped.tsv");

#[test]
fn the_shipped_palette_is_the_typescript_s_row_for_row() {
    let v = Vector::load(VECTOR);
    assert_eq!(fnv1a_64_hex(&encode_palette(&SHIPPED_PALETTE).unwrap()), v.s("packet_fnv"));
    let mut n = 0;
    v.each("rows", |k| {
        let r = &SHIPPED_PALETTE[n];
        n += 1;
        assert_eq!(r.id, v.u32(&format!("{k}.id")), "{k}.id");
        assert_eq!(r.kind.as_str(), v.s(&format!("{k}.kind")), "{k}.kind");
        assert_eq!(r.name, v.s(&format!("{k}.name")), "{k}.name");
        assert_eq!(r.model_hash, v.u64(&format!("{k}.model_hash")), "{k}.model_hash");
        assert_eq!(if r.model_key.is_empty() { "none" } else { &r.model_key }, v.s(&format!("{k}.model_key")), "{k}.model_key");
        assert_eq!(r.skin_key.as_deref().unwrap_or("none"), v.s(&format!("{k}.skin_key")), "{k}.skin_key");
        for (a, x) in r.extent.iter().enumerate() {
            assert_eq!(x.to_bits(), v.f64(&format!("{k}.extent.{a}")).to_bits(), "{k}.extent.{a}");
        }
        assert_eq!(r.size.to_bits(), v.f64(&format!("{k}.size")).to_bits(), "{k}.size");
        assert_eq!(if r.license.is_empty() { "none" } else { &r.license }, v.s(&format!("{k}.license")), "{k}.license");
        assert_eq!(if r.author.is_empty() { "none" } else { &r.author }, v.s(&format!("{k}.author")), "{k}.author");
        assert_eq!(r.color.as_deref().unwrap_or("none"), v.s(&format!("{k}.color")), "{k}.color");
        assert_eq!(r.group.as_deref().unwrap_or("none"), v.s(&format!("{k}.group")), "{k}.group");
    });
    assert_eq!(n, SHIPPED_PALETTE.len());
}
