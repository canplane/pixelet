// The sky's own words: the ray through each point of the screen, and the Moon's.

struct Sky {
  ray: mat3x3f,
  pole: vec3f,
  mapped: f32,
};

// Field by field, never as a struct: a struct lost Safari's WebGPU its GPU process.
@group(0) @binding(19) var<uniform> sky_words: array<vec4f, 4>;

fn sky_ray() -> mat3x3f { return mat3x3f(sky_words[0].xyz, sky_words[1].xyz, sky_words[2].xyz); }
fn sky_pole() -> vec3f { return sky_words[3].xyz; }
fn sky_mapped() -> f32 { return sky_words[3].w; }
