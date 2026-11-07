// pxl-render/gpu/surface.test.rs

use super::*;
use crate::frame::shader::floor;
use crate::gpu::api::word_bytes;
use crate::gpu::ray::{drawn_fits, DRAWN_AREA, DRAWN_MOST};
use pxl_engine::{beam, ray};
use pxl_spec::space::camera::grid::HALF_TAN;

fn world(px: [u32; 2], dpr: f64, ratio: f64) -> Surface {
    Surface {
        handle: 1,
        role: surface_role::WORLD,
        px_w: px[0],
        px_h: px[1],
        dpr,
        ratio,
        ..Surface::default()
    }
}

/// An 8K canvas at any ratio, and a square one at and past the floor's side, is drawn at what
/// every device draws (`drawn_fits`): no side past the floor's, its screen tiles within a
/// dispatch's side, its shape kept, and at the floor's side or within a tile a side of the
/// dispatch's; a canvas within it at the ratio asked.
#[test]
fn drawn_within_the_floor() {
    for (px, dpr) in [
        ([7680, 4320], 1.0),
        ([15360, 8640], 2.0),
        ([8640, 15360], 2.0),
        ([8192, 8192], 1.0),
        ([16384, 16384], 2.0),
    ] {
        for share in [1.0, 0.5] {
            let (w, h) = world(px, dpr, dpr * share).render_px();
            assert!(drawn_fits([w, h]), "{px:?} at {share}: {w}x{h}");
            let shape = f64::from(w) / f64::from(h) - f64::from(px[0]) / f64::from(px[1]);
            assert!(shape.abs() < 1e-3, "{px:?}: {w}x{h} keeps no shape");
            let whole = [px[0], px[1]].map(|v| round(f64::from(v) * share) as u32);
            if drawn_fits(whole) {
                assert_eq!((w, h), (whole[0], whole[1]), "{px:?} at {share}");
            } else {
                let side = w.max(h) == DRAWN_MOST[0];
                let tiles = [w, h].map(|v| v.div_ceil(TILE) + 1);
                let area = f64::from(w) * f64::from(h) > 0.99 * f64::from(DRAWN_AREA);
                assert!(
                    side || area || tiles[0] * tiles[1] > WORKGROUPS_MOST,
                    "{px:?} at {share}: {w}x{h} short of the most"
                );
            }
        }
    }
}

/// At the most drawn, of every shape from a square to 32:9, the beam rays' buffer, the camera
/// grid's words at that shape's field and the eye's holes past them, fits the floor.
#[test]
fn most_drawn_fits_every_buffer() {
    for wide in [1.0f32, 16.0 / 9.0, 21.0 / 9.0, 32.0 / 9.0] {
        let short = (DRAWN_AREA as f32 / wide).sqrt();
        let long = short * wide;
        for size in [[long as u32, short as u32], [short as u32, long as u32]] {
            let aspect = size[0] as f32 / size[1] as f32;
            let tan = [HALF_TAN as f32 * aspect, HALF_TAN as f32];
            let words = beam::Grid::of(tan).walked_at() + ray::holes_words(size);
            let bytes = u64::from(word_bytes(words as usize));
            assert!(floor::fits(bytes, true), "{size:?}: {bytes} bytes");
        }
    }
}
