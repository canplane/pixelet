// pxl-shader/light.rs

//! The head the shadow map's cast is handed (`Head`), which the eye's passes
//! bind beside the light's. The shadow map's passes are pxl-render's by hand
//! (light_beam.wgsl, light_cell.wgsl, sample.wgsl, light.wgsl,
//! light_miss.wgsl, light_mark.wgsl, light_mark_draws.wgsl,
//! light_propagate.wgsl, light_slide.wgsl, light_sample_box.wgsl), the map
//! read there as light_list.wgsl.

use spirv_std::glam::IVec4;

/// What the light's cast is handed beside the eye's words, a uniform
/// (pxl-engine `light::HEAD_VEC4S`): its number, whether it is counted,
/// its epoch and the map's side, where the rays start (`light::Starts`) and
/// the actors drawn. The eye's head, all zero, names no map (`size`).
#[repr(C)]
#[derive(Clone, Copy)]
pub struct Head {
    v: [IVec4; pxl_engine::light::HEAD_VEC4S],
}
