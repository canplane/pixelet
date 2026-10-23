// The frame uniform, group 0 binding 0, read word by word, never as a
// struct: a struct here lost Safari's WebGPU its GPU process. Its fields
// are its accessors', in record.rs's FRAME_AT order; the map holds the
// eye's place on it, Mercator's longitude and ordinate, and the map's
// radius in units. It holds no light: every light is the air's
// (atmos_medium.wgsl).
@group(0) @binding(0) var<uniform> frame_words: array<vec4f, FRAME_VEC4S>;

fn frame_view_proj() -> mat4x4f { return mat4x4f(frame_words[0], frame_words[1], frame_words[2], frame_words[3]); }
fn frame_camera_pos() -> vec3f { return frame_words[4].xyz; }
fn frame_edge_strength() -> f32 { return frame_words[4].w; }
fn frame_viewport() -> vec2f { return frame_words[5].xy; }
fn frame_screen_ao() -> f32 { return frame_words[5].z; }
fn frame_vignette() -> f32 { return frame_words[5].w; }
fn frame_map() -> vec3f { return frame_words[6].xyz; }
fn frame_cell() -> f32 { return frame_words[6].w; }
fn frame_low_spec() -> f32 { return frame_words[7].x; }
fn frame_edge_width() -> f32 { return frame_words[7].y; }
fn frame_rim_intensity() -> f32 { return frame_words[7].z; }
// Up at the eye, the world's z: the air's centre lies straight under it.
fn frame_up() -> vec3f { return vec3f(0.0, 0.0, 1.0); }
