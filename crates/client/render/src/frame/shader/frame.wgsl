// The frame uniform, group 0 binding 0, read word by word, never as a
// struct: a struct here lost Safari's WebGPU its GPU process. Its fields
// are its accessors', in record.rs's FRAME_AT order; the map's word holds
// the eye's place on it, Mercator's longitude and ordinate, and the map's
// radius in units.
@group(0) @binding(0) var<uniform> frame_words: array<vec4f, FRAME_VEC4S>;

fn frame_view_proj() -> mat4x4f { return mat4x4f(frame_words[0], frame_words[1], frame_words[2], frame_words[3]); }
fn frame_camera_pos() -> vec3f { return frame_words[4].xyz; }
fn frame_edge_strength() -> f32 { return frame_words[4].w; }
fn frame_sun_dir() -> vec3f { return frame_words[5].xyz; }
fn frame_low_spec() -> f32 { return frame_words[5].w; }
fn frame_sun_color() -> vec3f { return frame_words[6].xyz; }
fn frame_edge_width() -> f32 { return frame_words[6].w; }
fn frame_other_dir() -> vec3f { return frame_words[7].xyz; }
fn frame_rim_intensity() -> f32 { return frame_words[7].w; }
fn frame_other_color() -> vec3f { return frame_words[8].xyz; }
fn frame_vignette() -> f32 { return frame_words[8].w; }
fn frame_viewport() -> vec2f { return frame_words[9].xy; }
fn frame_map() -> vec4f { return frame_words[10]; }
fn frame_cell() -> f32 { return frame_words[11].x; }
// Up at the eye, the world's z: the air's centre lies straight under it.
fn frame_up() -> vec3f { return vec3f(0.0, 0.0, 1.0); }
