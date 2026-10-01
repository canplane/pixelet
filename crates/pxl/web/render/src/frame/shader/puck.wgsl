// The location puck's draw uniform, shared by its zone and its beacon
// (kernel layer/marker/location_puck.rs): the translation, the fourth float
// of the module's uniform (the beacon's spin), the color the puck pass paints
// the zone in (gpu/pass/puck.rs) — the UI's action color — and whether it is
// night.

struct Draw {
  translation: vec3f,
  fourth: f32,
  color: vec3f,
  night: f32,
};

// Held as vec4 words, each field read from its own by `draw_<field>()`:
// what the host and a shader share has no room inside it for a driver to
// lay out its own way (issue/20260927-a-matrix-the-driver-reads-its-own-way.md),
// and nothing builds the struct back, with which Safari's WebGPU lost its
// GPU process.
@group(1) @binding(0) var<uniform> draw_words: array<vec4f, 2>;

fn draw_translation() -> vec3f { return draw_words[0].xyz; }
fn draw_fourth() -> f32 { return draw_words[0].w; }
fn draw_color() -> vec3f { return draw_words[1].xyz; }
fn draw_night() -> f32 { return draw_words[1].w; }
