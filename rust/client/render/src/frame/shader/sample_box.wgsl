// pxl-render/sample_box.wgsl

// Each sample of the camera grid, a thread each, its voxel written as a box for the raster's
// draw and the face it entered drawn, the eye as sample.eye.wgsl takes it, in two passes
// (boxes.wgsl `claim_box`, `observe_box`; pxl-engine `raster`). The shadow map's is
// light_sample_box.wgsl.

fn eye_origin() -> vec3<i32> {
  return vec3<i32>(i32(eye_word(ORIGIN)), i32(eye_word(ORIGIN + 1u)), i32(eye_word(ORIGIN + 2u)));
}

@compute @workgroup_size(SQUARE_SIDE, SQUARE_SIDE, 1)
fn sample_box_cs(@builtin(global_invocation_id) id: vec3<u32>) {
  claim_box(id.xy, eye_origin(), true, vec2<f32>(eye_f32(TAN), eye_f32(TAN + 1u)));
}

@compute @workgroup_size(SQUARE_SIDE, SQUARE_SIDE, 1)
fn sample_observe_cs(@builtin(global_invocation_id) id: vec3<u32>) {
  observe_box(id.xy, eye_origin(), true, vec2<f32>(eye_f32(TAN), eye_f32(TAN + 1u)));
}
