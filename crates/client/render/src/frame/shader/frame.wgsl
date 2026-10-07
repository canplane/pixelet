// The frame uniform, group 0 binding 0; layout named by record.rs's FRAME_AT.

struct Frame {
  view_proj: mat4x4f,
  camera_pos: vec3f,
  edge_strength: f32,
  sun_dir: vec3f,
  low_spec: f32,
  sun_color: vec3f,
  _p8: f32,
  _p10: vec3f,
  _p12: f32,
  _p6: vec3f,
  _p11: f32,
  _p7: vec3f,
  edge_width: f32,
  _p5: vec3f,
  rim_intensity: f32,
  viewport: vec2f,
  shadow_strength: f32,
  shadow_texel: f32,
  vignette: f32,
  ao_strength: f32,
  screen_ao: f32,
  light_view_proj: mat4x4f,
  other_dir: vec3f,
  _p3: f32,
  other_color: vec3f,
  _p4: f32,
  // The sea under the eye in the root frame, planet radius (0 for none), center ground
  // height; then the center's Mercator exponential, sin and cos of latitude, Mercator radius.
  planet: vec4f,
  globe: vec4f,
  // Away from the planet's center, which lies under the eye, at the eye.
  up: vec3f,
  // The camera grid's cell angle, radians.
  cell: f32,
  // 1 while the camera grid is listed, derived by the renderer: chunks tint by their cells' share of a grid cell.
  gizmo: f32,
  _p13: f32,
  _p14: vec2f,
};

// Read field by field from vec4 words, never as a struct: a struct here lost
// Safari's WebGPU its GPU process.
@group(0) @binding(0) var<uniform> frame_words: array<vec4f, 23>;

fn frame_view_proj() -> mat4x4f { return mat4x4f(frame_words[0], frame_words[1], frame_words[2], frame_words[3]); }
fn frame_camera_pos() -> vec3f { return frame_words[4].xyz; }
fn frame_edge_strength() -> f32 { return frame_words[4].w; }
fn frame_sun_dir() -> vec3f { return frame_words[5].xyz; }
fn frame_low_spec() -> f32 { return frame_words[5].w; }
fn frame_sun_color() -> vec3f { return frame_words[6].xyz; }
fn frame_edge_width() -> f32 { return frame_words[9].w; }
fn frame_rim_intensity() -> f32 { return frame_words[10].w; }
fn frame_viewport() -> vec2f { return frame_words[11].xy; }
fn frame_shadow_strength() -> f32 { return frame_words[11].z; }
fn frame_shadow_texel() -> f32 { return frame_words[11].w; }
fn frame_vignette() -> f32 { return frame_words[12].x; }
fn frame_ao_strength() -> f32 { return frame_words[12].y; }
fn frame_screen_ao() -> f32 { return frame_words[12].z; }
fn frame_light_view_proj() -> mat4x4f { return mat4x4f(frame_words[13], frame_words[14], frame_words[15], frame_words[16]); }
fn frame_other_dir() -> vec3f { return frame_words[17].xyz; }
fn frame_other_color() -> vec3f { return frame_words[18].xyz; }
fn frame_planet() -> vec4f { return frame_words[19]; }
fn frame_globe() -> vec4f { return frame_words[20]; }
fn frame_up() -> vec3f { return frame_words[21].xyz; }
fn frame_cell() -> f32 { return frame_words[21].w; }
fn frame_gizmo() -> f32 { return frame_words[22].x; }

fn on_globe(p: vec3f) -> vec3f {
  return on_globe_of(p, frame_planet(), frame_globe());
}
