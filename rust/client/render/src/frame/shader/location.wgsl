// Read field by field from vec4 words, never as a struct: a struct here lost
// Safari's WebGPU its GPU process.
@group(1) @binding(0) var<uniform> location_words: array<vec4f, DRAW_VEC4S>;

fn draw_translation() -> vec3f { return from_eye(location_q(), location_r()); }
