// pxl-shader/light_mark.rs

//! The world's receivers of the shadow map off the eye's rays drawn
//! (`light_mark_cs`), the one pass of the light's that reads them (rays.rs
//! `Rays`), apart from the rest of the light's (light.rs).

use pxl_spec_core::light::Light;
use spirv_std::glam::UVec3;
use spirv_std::spirv;

use super::Rays;
use crate::cast::px_tan;
use crate::eye::{eye_ray, Eye};
use crate::light::{cast_number, footprint, light_of, size, stand, stand_between, Head};

/// The world's receivers of the shadow map: the drawn pixels' rays (rays.rs
/// `Rays`), a thread a square of them `light::MARK` a side, each read, the
/// tile the point it met the world at lies on marked stood on
/// (`light::stands_at`), a run on one tile once, and the tiles between it
/// and where each pixel beside it met it, across, down and either diagonal,
/// each pair of neighbors once, its footprint (`light::FOOTPRINT_TILES`), so
/// a jittered ray stands on a tile marked whichever way it moves, where the
/// head asks it (`light::HEAD_FOOTPRINT`), else its own tile alone; nothing
/// where it met nothing. A word written alike by every thread it is marked
/// by, so none needs an atomic. Run after the eye's rays, before the
/// propagation (`light_propagate_cs`); the drawn receivers stand apart
/// (`light_mark_drawn_cs`).
#[spirv(compute(threads(8, 8, 1), entry_point_name = "light_mark_cs"))]
pub fn light_mark_cs(
    #[spirv(global_invocation_id)] id: UVec3,
    #[spirv(uniform, descriptor_set = 0, binding = 0)] eye: &Eye,
    #[spirv(descriptor_set = 0, binding = 3)] rays: &Rays,
    #[spirv(uniform, descriptor_set = 0, binding = 11)] head: &Head,
    #[spirv(storage_buffer, descriptor_set = 0, binding = 12)] list: &mut [u32],
) {
    let drawn: UVec3 = rays.query_size();
    let m = pxl_spec_core::light::MARK;
    if id.x * m >= drawn.x || id.y * m >= drawn.y {
        return;
    }
    let (s, n) = (light_of(eye), size(head));
    let (cast, footprint) = (cast_number(head), footprint(head));
    // Each pixel's tile read once: the square's, and where the footprint is
    // taken, a column past it across and a row past it down and one before
    // it, which its neighbors take (`MARK_READ`); none where a pixel met
    // nothing or lies past the drawn.
    let (cols, rows) = (m + 1, m + 2);
    let mut met = [0u32; MARK_READ];
    let mut tiles = [[0i32; 2]; MARK_READ];
    let mut k = 0;
    while k < cols * rows {
        let (x, y) = ((id.x * m + k % cols) as i32, (id.y * m + k / cols) as i32 - 1);
        let own = k % cols < m && k / cols >= 1 && k / cols <= m;
        if y >= 0 && (footprint || own) {
            let (got, tile) = met_tile(eye, rays, &s, UVec3::new(x as u32, y as u32, 0), drawn);
            met[k as usize] = u32::from(got);
            tiles[k as usize] = tile;
        }
        k += 1;
    }
    // The tile last marked, so a run of pixels on one marks it once.
    let mut last = [i32::MIN; 2];
    let mut y = 0;
    while y < m {
        let mut x = 0;
        while x < m {
            let k = ((y + 1) * cols + x) as usize;
            if met[k] != 0 {
                let tile = tiles[k];
                // [rust-gpu] By its words: an array's `!=` is an intrinsic
                // it does not lower.
                if tile[0] != last[0] || tile[1] != last[1] {
                    stand(list, n, cast, tile);
                    last = tile;
                }
                // The neighbors after it, so every pair of neighbors is taken
                // once: across, down, and the two diagonals.
                let mut j = 0;
                while j < 4 && footprint {
                    let (dx, dy) = ([1, 0, 1, 1][j], [0, 1, 1, -1][j]);
                    let q = ((y as i32 + 1 + dy) as u32 * cols + x + dx) as usize;
                    if met[q] != 0 {
                        stand_between(list, n, cast, tile, tiles[q]);
                    }
                    j += 1;
                }
            }
            x += 1;
        }
        y += 1;
    }
}

/// The pixels a thread of the mark reads (`light_mark_cs`): its square,
/// `light::MARK` a side, a column past it and a row either side.
const MARK_READ: usize =
    ((pxl_spec_core::light::MARK + 1) * (pxl_spec_core::light::MARK + 2)) as usize;

/// Whether pixel `p`'s ray met the world, and the tile it met it on. A ray
/// that met nothing leaves its first word zero (cast.rs `word`).
fn met_tile(eye: &Eye, rays: &Rays, s: &Light, p: UVec3, drawn: UVec3) -> (bool, [i32; 2]) {
    if p.x >= drawn.x || p.y >= drawn.y {
        return (false, [0; 2]);
    }
    let first: u32 = rays.read(UVec3::new(p.x, p.y, 0));
    if first == 0 || first == pxl_spec_core::ray::MISS {
        return (false, [0; 2]);
    }
    let t = f32::from_bits(rays.read(UVec3::new(p.x, p.y, 1)));
    let d = eye_ray(eye, px_tan(eye, p, drawn)).dir;
    (true, s.tile_of([d[0] * t, d[1] * t, d[2] * t]))
}
