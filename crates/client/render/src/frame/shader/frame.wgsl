// The frame uniform, group 0 binding 0: its fields and their accessors are
// frame/shader/word.rs `frame`'s, which build.rs writes here.
@group(0) @binding(0) var<uniform> frame_words: array<vec4f, FRAME_VEC4S>;

// Up at the eye, the world's z: the air's centre lies straight under it.
fn frame_up() -> vec3f { return vec3f(0.0, 0.0, 1.0); }
