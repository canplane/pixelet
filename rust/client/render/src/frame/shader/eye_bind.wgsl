// pxl-render/eye_bind.wgsl

// The eye a cast is made from. Its words lie where pxl-engine eye.rs places them, and a shader
// reads them one word at a time.

@group(0) @binding(0) var<uniform> eye: array<vec4<u32>, EYE_VEC4S>;

fn eye_word(k: u32) -> u32 {
  return eye[k / 4u][k % 4u];
}

fn eye_f32(k: u32) -> f32 {
  return bitcast<f32>(eye_word(k));
}

fn eye_v3(k: u32) -> vec3<f32> {
  return vec3<f32>(eye_f32(k), eye_f32(k + 1u), eye_f32(k + 2u));
}
