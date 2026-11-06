// pxl-render/light_sample_box.wgsl

// Each sample of the shadow map the cast takes, a thread each, its voxel written as a box for
// the raster's draw and the face it entered drawn, the map as sample.light.wgsl takes it, in
// two passes (boxes.wgsl `claim_box`, `observe_box`), from where box texel (0, 0)'s ray starts
// and none about an eye, as every ray starts on the near plane (pxl-engine light.rs,
// raster.rs).

@compute @workgroup_size(SQUARE_SIDE, SQUARE_SIDE, 1)
fn light_sample_box_cs(@builtin(global_invocation_id) id: vec3<u32>) {
  claim_box(id.xy, texel_start(light_corner()), false, vec2<f32>(0.0));
}

@compute @workgroup_size(SQUARE_SIDE, SQUARE_SIDE, 1)
fn light_sample_observe_cs(@builtin(global_invocation_id) id: vec3<u32>) {
  observe_box(id.xy, texel_start(light_corner()), false, vec2<f32>(0.0));
}
