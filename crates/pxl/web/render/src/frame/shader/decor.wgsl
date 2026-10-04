// Decor: instanced variant models, reading the decor block as the instance
// buffer; the n-th instance drawn is the one the index list's n-th word names.

struct Draw {
  translation: vec3f,
  offset: u32,
  stride: u32,
  opacity: f32,
  _p0: u32,
  index: u32,
};

// Read field by field from vec4 words, never as a struct: a struct here lost
// Safari's WebGPU its GPU process.
@group(1) @binding(0) var<uniform> draw_words: array<vec4f, 2>;

fn draw_translation() -> vec3f { return draw_words[0].xyz; }
fn draw_offset() -> u32 { return bitcast<u32>(draw_words[0].w); }
fn draw_stride() -> u32 { return bitcast<u32>(draw_words[1].x); }
fn draw_opacity() -> f32 { return draw_words[1].y; }
fn draw_index() -> u32 { return bitcast<u32>(draw_words[1].w); }

@group(2) @binding(0) var skin: texture_2d<f32>;
@group(2) @binding(1) var skin_sampler: sampler;
@group(3) @binding(0) var<storage, read> block: array<u32>;

struct VertexIn {
  @builtin(instance_index) i: u32,
  @location(0) position: vec3f,
  @location(1) normal: vec3f,
  @location(2) uv: vec2f,
};

struct VertexOut {
  @builtin(position) clip: vec4f,
  @location(0) uv: vec2f,
  @location(1) normal: vec3f,
  @location(2) world_pos: vec3f,
  @location(3) through: vec3f,
  @location(4) scattered: vec3f,
  @location(5) past: vec3f,
  @location(6) @interpolate(flat) fill: vec3f,
  @location(7) @interpolate(flat) sun: vec3f,
  @location(8) @interpolate(flat) other: vec3f,
};

fn lane(k: u32, i: u32) -> f32 {
  return bitcast<f32>(block[draw_offset() + k * draw_stride() + i]);
}

// One function, so the depth passes place a vertex where the lit pass does.
struct Placed {
  world: vec3f,
  spin: f32,
};

fn placed(v: VertexIn) -> Placed {
  let i = block[draw_index() + v.i];
  let at = vec3f(lane(0u, i), lane(1u, i), lane(2u, i));
  var p: Placed;
  p.spin = lane(3u, i);
  let scale = lane(4u, i);
  p.world = on_globe(place_up(v.position * scale, p.spin) + at + draw_translation());
  return p;
}

@vertex
fn vs(v: VertexIn) -> VertexOut {
  let p = placed(v);
  var o: VertexOut;
  o.clip = frame_view_proj() * vec4f(p.world, 1.0);
  o.uv = v.uv;
  o.normal = place_up(v.normal, p.spin);
  o.world_pos = p.world;
  let seen = seen_from_eye(p.world);
  o.through = seen.through;
  o.scattered = seen.scattered;
  o.past = seen.past;
  o.sun = seen.light.sun;
  o.other = seen.light.other;
  o.fill = seen.light.fill;
  return o;
}

@vertex
fn vs_depth(v: VertexIn) -> @builtin(position) vec4f {
  return frame_view_proj() * vec4f(placed(v).world, 1.0);
}

@fragment
fn fs(i: VertexOut) -> @location(0) vec4f {
  let albedo = textureSample(skin, skin_sampler, i.uv).rgb;
  let n = normalize(i.normal);
  let color = shade(albedo, n, i.world_pos, sun_lit(i.world_pos, n), vec3f(screen_open(i.clip.xy)), Seen(i.through, i.scattered, i.past, Light(i.sun, i.other, i.fill)));
  return vec4f(finish(color, i.clip.xy), draw_opacity());
}
