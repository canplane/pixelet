// The frame's uniform. Its fields are listed in frame/shader/word.rs `frame`,
// and build.rs appends an accessor for each of them to this part.
@group(0) @binding(0) var<uniform> frame_words: array<vec4f, FRAME_VEC4S>;

// A record's position in units from the eye. The position is given as Q3
// steps `q` and a remainder `r` (gpu/draw.rs `Position`). The steps are
// subtracted as i32 first, and that difference wraps, so X is taken the short
// way round the world.
fn from_eye(q: vec3i, r: vec3f) -> vec3f {
  return vec3f(q - frame_eye()) * Q3_STEP + r;
}

// The air's center lies straight under the eye, so up at the eye is the
// world's z.
fn frame_up() -> vec3f { return vec3f(0.0, 0.0, 1.0); }

// A world draw's clip position, moved against this frame's jitter. Each
// pixel's middle then draws what that pixel's own ray goes through, as the
// cast's rays do.
fn jittered(clip: vec4f) -> vec4f {
  return vec4f(clip.xy - frame_jitter() * clip.w, clip.z, clip.w);
}
