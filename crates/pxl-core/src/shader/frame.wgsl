// The frame's uniform as every pass declares it, at group 0 binding 0: the
// view, the camera, the environment's run (environment.rs's output, sun_dir
// through fog_color), the effects' strengths, and the light's view for the
// shadow. The layout is shader.rs's FRAME_AT to name; the glue fills it.

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
  hemi_sky: vec3f,
  floor: f32,
  hemi_ground: vec3f,
  edge_width: f32,
  fog_color: vec3f,
  rim_intensity: f32,
  viewport: vec2f,
  shadow_strength: f32,
  shadow_texel: f32,
  vignette: f32,
  _p0: f32,
  _p1: f32,
  _p2: f32,
  light_view_proj: mat4x4f,
};

@group(0) @binding(0) var<uniform> frame: Frame;
