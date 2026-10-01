// Decor: instanced draws of each variant's model, the instances read
// straight out of the decor block the module built (decor.rs's layout) as
// it lies on the GPU — the block is the instance buffer, the chunk's
// translation is its uniform. The model's lanes are model.rs's; the skin is
// the glue's. An instance is its position, its spin about Z after the glTF
// Y-up to Z-up turn, and its scale; a hidden one is sent outside the clip
// volume.

struct Draw {
  translation: vec3f,
  offset: u32,
  count: u32,
  opacity: f32,
  _p: vec2u,
};

// Held as vec4 words, each field read from its own by `draw_<field>()`:
// what the host and a shader share has no room inside it for a driver to
// lay out its own way (issue/20260927-a-matrix-the-driver-reads-its-own-way.md),
// and nothing builds the struct back, with which Safari's WebGPU lost its
// GPU process.
@group(1) @binding(0) var<uniform> draw_words: array<vec4f, 2>;

fn draw_translation() -> vec3f { return draw_words[0].xyz; }
fn draw_offset() -> u32 { return bitcast<u32>(draw_words[0].w); }
fn draw_count() -> u32 { return bitcast<u32>(draw_words[1].x); }
fn draw_opacity() -> f32 { return draw_words[1].y; }

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
  // The air in front of the vertex (atmos_view.wgsl's Seen).
  @location(3) through: vec3f,
  @location(4) scattered: vec3f,
  @location(5) past: vec3f,
  @location(6) fill: vec3f,
};

fn lane(k: u32, i: u32) -> f32 {
  return bitcast<f32>(block[draw_offset() + k * draw_count() + i]);
}

@vertex
fn vs(v: VertexIn) -> VertexOut {
  let n = draw_count();
  let flags = (block[draw_offset() + 5u * n + v.i / 4u] >> ((v.i % 4u) * 8u)) & 255u;
  var o: VertexOut;
  if ((flags & 1u) != 0u) {
    o.clip = vec4f(0.0, 0.0, -2.0, 1.0);
    return o;
  }
  let at = vec3f(lane(0u, v.i), lane(1u, v.i), lane(2u, v.i));
  let spin = lane(3u, v.i);
  let scale = lane(4u, v.i);
  let world = on_globe(place_up(v.position * scale, spin) + at + draw_translation());
  o.clip = frame_view_proj() * vec4f(world, 1.0);
  o.uv = v.uv;
  o.normal = place_up(v.normal, spin);
  o.world_pos = world;
  let seen = seen_at_vertex(world);
  o.through = seen.through;
  o.scattered = seen.scattered;
  o.past = seen.past;
  o.fill = seen.fill;
  return o;
}

@fragment
fn fs(i: VertexOut) -> @location(0) vec4f {
  let albedo = textureSample(skin, skin_sampler, i.uv).rgb;
  let n = normalize(i.normal);
  let color = screen_ao(shade(albedo, n, i.world_pos, sun_lit(i.world_pos, n), vec3f(1.0), Seen(i.through, i.scattered, i.past, i.fill)), i.clip.xy);
  return vec4f(finish(color, i.clip.xy), draw_opacity());
}
