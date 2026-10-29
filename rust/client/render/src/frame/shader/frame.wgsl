// The frame uniform, group 0 binding 0: its fields and their accessors are
// frame/shader/word.rs `frame`'s, which build.rs writes here.
@group(0) @binding(0) var<uniform> frame_words: array<vec4f, FRAME_VEC4S>;

// Up at the eye, the world's z: the air's center lies straight under it.
fn frame_up() -> vec3f { return vec3f(0.0, 0.0, 1.0); }

// A world draw's clip pos moved against this frame's offset
// (`frame_jitter`), so each pixel's middle draws what its own ray goes
// through, as the cast's rays do.
fn jittered(clip: vec4f) -> vec4f {
  return vec4f(clip.xy - frame_jitter() * clip.w, clip.z, clip.w);
}
