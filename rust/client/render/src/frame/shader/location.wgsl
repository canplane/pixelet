// The uniform is read field by field from vec4 words and is never declared as a
// struct. Declared as a struct here, it makes Safari's WebGPU lose its GPU
// process.
@group(1) @binding(0) var<uniform> location_words: array<vec4f, DRAW_VEC4S>;

fn draw_translation() -> vec3f { return from_eye(location_q(), location_r()); }
