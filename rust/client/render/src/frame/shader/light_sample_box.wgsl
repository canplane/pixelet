// pxl-render/light_sample_box.wgsl

// Each sample of the shadow map the cast takes, a thread each, written as a box for the
// raster's draw (boxes.wgsl, the map as sample_light.wgsl takes it), from where box texel
// (0, 0)'s ray starts and none about an eye, as every ray starts on the near plane
// (pxl-shader light.rs `Map`, raster.rs `light_sample_box_cs`); none asks its faces.

@compute @workgroup_size(SQUARE_SIDE, SQUARE_SIDE, 1)
fn light_sample_box_cs(@builtin(global_invocation_id) id: vec3<u32>) {
  sample_box(id.xy, texel_start(light_corner()), false, vec2<f32>(0.0));
}
