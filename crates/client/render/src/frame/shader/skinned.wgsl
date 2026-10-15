// Actors: four-joint skinning from the palette, then glTF Y-up to Z-up, the
// heading, and placement; the fragment is decor's.

struct Draw {
  translation: vec3f,
  rotation: f32,
  opacity: f32,
  scale: f32,
  _p0: f32,
  _p1: f32,
};

// Read field by field from vec4 words, never as a struct: a struct here lost
// Safari's WebGPU its GPU process.
@group(1) @binding(0) var<uniform> draw_words: array<vec4f, 2>;

fn draw_translation() -> vec3f { return draw_words[0].xyz; }
fn draw_rotation() -> f32 { return draw_words[0].w; }
fn draw_opacity() -> f32 { return draw_words[1].x; }
fn draw_scale() -> f32 { return draw_words[1].y; }

@group(2) @binding(0) var skin: texture_2d<f32>;
@group(2) @binding(1) var skin_sampler: sampler;
// The palette is read as vec4 columns: a Galaxy S23's Vulkan misread
// `array<mat4x4f>` in storage.
@group(3) @binding(0) var<storage, read> palette: array<vec4f>;

fn joint(j: u32) -> mat4x4f {
  return mat4x4f(palette[j * 4u], palette[j * 4u + 1u], palette[j * 4u + 2u], palette[j * 4u + 3u]);
}

struct VertexIn {
  @location(0) position: vec3f,
  @location(1) normal: vec3f,
  @location(2) uv: vec2f,
  @location(3) joints: vec4<u32>,
  @location(4) weights: vec4f,
};

struct VertexOut {
  @builtin(position) clip: vec4f,
  @location(0) uv: vec2f,
  @location(1) normal: vec3f,
  @location(2) world_pos: vec3f,
  @location(3) through: f32,
  @location(4) scattered: vec3f,
  @location(5) @interpolate(flat) fill: vec3f,
  @location(6) @interpolate(flat) sun: vec3f,
  @location(7) @interpolate(flat) other: vec3f,
};

fn skin_of(v: VertexIn) -> mat4x4f {
  return joint(v.joints.x) * v.weights.x + joint(v.joints.y) * v.weights.y + joint(v.joints.z) * v.weights.z + joint(v.joints.w) * v.weights.w;
}

// One function, so the depth passes place a vertex where the lit pass does.
fn skinned_world(v: VertexIn, m: mat4x4f) -> vec3f {
  let p = (m * vec4f(v.position, 1.0)).xyz * draw_scale();
  return on_globe(place_up(p, draw_rotation()) + draw_translation());
}

@vertex
fn vs(v: VertexIn) -> VertexOut {
  let m = skin_of(v);
  let world = skinned_world(v, m);
  let n = (m * vec4f(v.normal, 0.0)).xyz;
  var o: VertexOut;
  o.clip = frame_view_proj() * vec4f(world, 1.0);
  o.uv = v.uv;
  o.normal = place_up(n, draw_rotation());
  o.world_pos = world;
  let seen = seen_from_eye(world);
  o.through = seen.through;
  o.scattered = seen.scattered;
  o.sun = seen.light.sun;
  o.other = seen.light.other;
  o.fill = seen.light.fill;
  return o;
}

@vertex
fn vs_depth(v: VertexIn) -> @builtin(position) vec4f {
  return frame_view_proj() * vec4f(skinned_world(v, skin_of(v)), 1.0);
}

@fragment
fn fs(i: VertexOut) -> @location(0) vec4f {
  let albedo = textureSample(skin, skin_sampler, i.uv).rgb;
  let n = normalize(i.normal);
  let color = shade(albedo, n, i.world_pos, vec3f(screen_open(i.clip.xy)), Seen(i.through, i.scattered, Light(i.sun, i.other, i.fill)));
  return vec4f(finish(color, i.clip.xy), draw_opacity());
}
