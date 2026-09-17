// pxl_shared/record/palette.test.rs
// (no pair: palette.ts's tests are record.test.ts, mirrored beside this)

use xpute_core::kit::encoding::bytes_to_hex;
use xpute_core::sys::vector::Vector;

use super::*;

const VECTOR: &str = include_str!("../../vector/record/palette.tsv");

#[test]
fn a_palette_is_what_the_typescript_writes_and_reads() {
    let v = Vector::load(VECTOR);
    let e = |id, kind, name: &str, model_hash, model_key: &str, extent, size, license: &str, author: &str| PaletteEntry {
        id,
        kind,
        name: name.into(),
        model_hash,
        model_key: model_key.into(),
        skin_key: None,
        extent,
        size,
        license: license.into(),
        author: author.into(),
        color: None,
        group: None,
    };
    let rows = [
        PaletteEntry {
            color: Some("#ffffff".into()),
            ..e(1, PaletteKind::Cube, "white", 0, "", [1.0, 1.0, 1.0], 1.0, "", "")
        },
        PaletteEntry {
            skin_key: Some("asset/y".into()),
            group: Some("tree".into()),
            ..e(0x1000, PaletteKind::Decor, "tree:pine", 0xfeed, "asset/x", [1.18, 1.33, 2.3], 0.25, "CC0-1.0", "Kenney")
        },
        e(0x2000, PaletteKind::Actor, "cat", 1, "asset/z", [f64::NAN, 2.0, 1.0], 1.0, "", ""),
    ];
    let pkt = encode_palette(&rows).unwrap();
    assert_eq!(bytes_to_hex(&pkt), v.s("packet"));
    let p = PaletteTable::new(RecordSource::Packet(pkt)).unwrap();
    for i in 0..rows.len() as u32 {
        // NaN with NaN: the payload is the engine's.
        let (got, want) = (p.extent_max(i) as f64, v.f64(&format!("extent_max.{i}")));
        assert!(got.to_bits() == want.to_bits() || (got.is_nan() && want.is_nan()), "extent_max.{i}: {got} vs {want}");
        let e = p.entry(i).unwrap();
        let k = format!("entries.{i}");
        assert_eq!(e.kind.as_str(), v.s(&format!("{k}.kind")), "{k}.kind");
        assert_eq!(e.skin_key.as_deref().unwrap_or("none"), v.s(&format!("{k}.skin_key")), "{k}.skin_key");
        assert_eq!(e.color.as_deref().unwrap_or("none"), v.s(&format!("{k}.color")), "{k}.color");
        assert_eq!(e.group.as_deref().unwrap_or("none"), v.s(&format!("{k}.group")), "{k}.group");
        for (a, x) in e.extent.iter().enumerate() {
            let want = v.f64(&format!("{k}.extent.{a}"));
            assert!(x.to_bits() == want.to_bits() || (x.is_nan() && want.is_nan()), "{k}.extent.{a}: {x} vs {want}");
        }
    }
}
