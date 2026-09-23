// The frame's uniform as every pass declares it, at group 0 binding 0: the
// view, the camera, the environment's run (look/environment.rs's output, sun_dir
// through its padding), the effects' strengths, and the light's view for the
// shadow, and the second light — the dimmer of the sun and the moon, which
// casts no shadow; and the planet's curvature. The layout is shader.rs's
// FRAME_AT to name; the glue fills it.

struct Frame {
  view_proj: mat4x4f,
  camera_pos: vec3f,
  edge_strength: f32,
  sun_dir: vec3f,
  low_spec: f32,
  sun_color: vec3f,
  fog_near: f32,
  ambient: vec3f,
  fog_far: f32,
  _p6: vec3f,
  floor: f32,
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
  // 1 where the lit passes' vertices take the air in front of them
  // (atmos_view.wgsl's seen_at_vertex), 0 in the shadow map's frame, whose
  // passes write depth and nothing the air would change.
  air_at_vertices: f32,
  light_view_proj: mat4x4f,
  other_dir: vec3f,
  _p3: f32,
  other_color: vec3f,
  _p4: f32,
  // The planet the flat map is laid back on (look/globe.rs): the map
  // centre in the root frame, the planet's radius in its units (0 for none),
  // and the centre's ground height; then the centre's Mercator exponential,
  // its latitude's sine and cosine, and Mercator's radius in units.
  planet: vec4f,
  globe: vec4f,
};

@group(0) @binding(0) var<uniform> frame: Frame;

// A point of the flat map where the planet has it, by the frame's values
// (globe.wgsl).
fn on_globe(p: vec3f) -> vec3f {
  return on_globe_of(p, frame.planet, frame.globe);
}
