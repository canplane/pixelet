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
/// `Rays`), a thread each: the tile the point it met the world at lies on
/// marked stood on (`light::stands_at`), and the tiles between it and where
/// each pixel after it met it, across, down and either diagonal, each pair
/// of neighbors once, its footprint (`light::FOOTPRINT_TILES`), so a
/// jittered ray stands on a tile marked whichever way it moves, where the
/// head asks it (`light::HEAD_FOOTPRINT`), else its own tile alone; nothing
/// where it met nothing. A word written alike by every thread it is marked
/// by, so none needs an atomic, and only by the first to find it unmarked
/// (light.rs `stand`). Run after the eye's rays, before the propagation
/// (`light_propagate_cs`); the drawn receivers stand apart
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
    let s = light_of(eye);
    let (got, tile) = met_tile(eye, rays, &s, UVec3::new(id.x, id.y, 0), drawn);
    if !got {
        return;
    }
    let (n, cast) = (size(head), cast_number(head));
    stand(list, n, cast, tile);
    // The neighbors after it, so every pair of neighbors is taken once:
    // across, down, and the two diagonals.
    let mut j = 0;
    while j < 4 && footprint(head) {
        let (dx, dy) = match j {
            0 => (1, 0),
            1 => (0, 1),
            2 => (1, 1),
            _ => (1, -1),
        };
        let qy = id.y as i32 + dy;
        if qy >= 0 {
            let (met, at) = met_tile(eye, rays, &s, UVec3::new(id.x + dx, qy as u32, 0), drawn);
            if met {
                stand_between(list, n, cast, tile, at);
            }
        }
        j += 1;
    }
}

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
