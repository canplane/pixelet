// A decor draw's instances, as the model tier (decor.wgsl) and the
// impostors (impostor.wgsl) both read them: the draw's uniform
// (gpu/pass/model.rs `put_instances`) and the decor's instances, `instances`,
// a lane a field; the n-th instance drawn is the one the index list's
// n-th word names.

struct Draw {
  translation: vec3f,
  offset: u32,
  stride: u32,
  opacity: f32,
  // The impostor's sprite layer; the model tier's padding.
  layer: u32,
  index: u32,
};

// Read field by field from vec4 words, never as a struct: a struct here lost
// Safari's WebGPU its GPU process.
@group(1) @binding(0) var<uniform> draw_words: array<vec4f, DRAW_VEC4S>;

fn draw_translation() -> vec3f { return draw_words[0].xyz; }
fn draw_offset() -> u32 { return bitcast<u32>(draw_words[0].w); }
fn draw_stride() -> u32 { return bitcast<u32>(draw_words[1].x); }
fn draw_opacity() -> f32 { return draw_words[1].y; }
fn draw_layer() -> u32 { return bitcast<u32>(draw_words[1].z); }
fn draw_index() -> u32 { return bitcast<u32>(draw_words[1].w); }

@group(3) @binding(0) var<storage, read> instances: array<u32>;

// Instance `i`'s word in lane `k`.
fn lane(k: u32, i: u32) -> f32 {
  return bitcast<f32>(instances[draw_offset() + k * draw_stride() + i]);
}
