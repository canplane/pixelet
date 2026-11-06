struct Draw {
  // The position's Q3 and its remainder (frame/draw.rs `Position`).
  q: vec3i,
  fourth: f32,
  color: vec3f,
  night: f32,
  r: vec3f,
  _p: f32,
};

// Read field by field from vec4 words, never as a struct: a struct here lost
// Safari's WebGPU its GPU process.
@group(1) @binding(0) var<uniform> draw_words: array<vec4f, DRAW_VEC4S>;

fn draw_translation() -> vec3f { return from_eye(bitcast<vec3i>(draw_words[0].xyz), draw_words[2].xyz); }
fn draw_fourth() -> f32 { return draw_words[0].w; }
fn draw_color() -> vec3f { return draw_words[1].xyz; }
fn draw_night() -> f32 { return draw_words[1].w; }
