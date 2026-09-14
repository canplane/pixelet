// pxl_shared/record/texture.test.rs
// (no pair: texture.ts's tests are record.test.ts, mirrored beside this)

use super::*;
use crate::spec::cell::cube::cube_types;
use xpute_core::vector::Vector;

const VECTOR: &str = include_str!("../../vector/record/texture.tsv");

#[test]
fn the_texture_keys_and_tiles_are_the_typescript_s() {
    let v = Vector::load(VECTOR);
    assert_eq!(
        (SHIPPED_SHEET.key, SHIPPED_SHEET.cell, SHIPPED_SHEET.columns),
        (v.s("sheet.key"), v.u32("sheet.cell"), v.u32("sheet.columns"))
    );
    let mut faces = Vec::new();
    for (channel, types) in cube_types::CHANNELS {
        for (name, _) in types.iter() {
            for face in [CubeFace::Top, CubeFace::Side] {
                faces.push((*channel, *name, face));
            }
        }
    }
    let mut n = 0;
    v.each("faces", |k| {
        let (channel, name, face) = faces[n];
        n += 1;
        assert_eq!(
            (channel, name, if face == CubeFace::Top { "top" } else { "side" }),
            (v.s(&format!("{k}.channel")), v.s(&format!("{k}.name")), v.s(&format!("{k}.face"))),
            "{k}"
        );
        assert_eq!(shipped_texture_key(channel, name, face).unwrap_or("none"), v.s(&format!("{k}.key")), "{k}.key");
        let tile = shipped_texture_tile(channel, name, face).map_or("none".to_string(), |t| format!("{},{}", t.x, t.y));
        assert_eq!(tile, v.s(&format!("{k}.tile")), "{k}.tile");
    });
    assert_eq!(n, faces.len());
}
