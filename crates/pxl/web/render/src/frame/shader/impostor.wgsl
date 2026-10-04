// Decor's impostor tier: each instance of a decor block drawn as a quad
// turned to the eye about the world's up, its variant's sprite on it
// (impostor_bake.wgsl), where the variant is too small on the screen for its
// model to be worth its triangles (kernel layer/decor/build.rs `decor_tier`).
// The instances are decor.wgsl's, read the same way; a hidden one is sent
// outside the clip volume.

// Held as vec4 words, as decor.wgsl's, with the sprite's layer where decor
// has padding.
struct Draw {
  translation: vec3f,
  offset: u32,
  stride: u32,
  opacity: f32,
  layer: u32,
  index: u32,
};

@group(1) @binding(0) var<uniform> draw_words: array<vec4f, 2>;

fn draw_translation() -> vec3f { return draw_words[0].xyz; }
fn draw_offset() -> u32 { return bitcast<u32>(draw_words[0].w); }
fn draw_count() -> u32 { return bitcast<u32>(draw_words[1].x); }
fn draw_opacity() -> f32 { return draw_words[1].y; }
fn draw_layer() -> u32 { return bitcast<u32>(draw_words[1].z); }

@group(2) @binding(0) var sprites: texture_2d_array<f32>;
@group(2) @binding(1) var sprite_sampler: sampler;
// Each layer's fit, as the bake drew it: the model's center across and up
// and its half side, in the model's units.
@group(2) @binding(2) var<uniform> fits: array<vec4f, 64>;
@group(3) @binding(0) var<storage, read> block: array<u32>;

struct VertexIn {
  @builtin(instance_index) i: u32,
  @builtin(vertex_index) k: u32,
};

// The normal is the quad's, turned to the eye once for all four corners,
// and the sky's fill the frame's: both flat.
struct VertexOut {
  @builtin(position) clip: vec4f,
  @location(0) uv: vec2f,
  @location(1) @interpolate(flat) normal: vec3f,
  @location(2) world_pos: vec3f,
  @location(3) through: vec3f,
  @location(4) scattered: vec3f,
  @location(5) past: vec3f,
  @location(6) @interpolate(flat) fill: vec3f,
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
  let base = vec3f(lane(0u, v.i), lane(1u, v.i), lane(2u, v.i)) + draw_translation();
  let scale = lane(4u, v.i);
  let fit = fits[draw_layer()];
  let corner = vec2f(select(-1.0, 1.0, (v.k & 1u) != 0u), select(-1.0, 1.0, (v.k & 2u) != 0u));
  let up = vec3f(0.0, 0.0, 1.0);
  let to_eye = frame_camera_pos() - base;
  let flat = vec3f(to_eye.xy, 0.0);
  let facing = select(vec3f(0.0, -1.0, 0.0), normalize(flat), dot(flat, flat) > 1e-8);
  let side = cross(up, facing);
  let local = side * (fit.x + corner.x * fit.z) * scale + up * (fit.y + corner.y * fit.z) * scale;
  let world = on_globe(base + local);
  o.clip = frame_view_proj() * vec4f(world, 1.0);
  o.uv = vec2f(0.5 + 0.5 * corner.x, 0.5 - 0.5 * corner.y);
  o.normal = facing;
  o.world_pos = world;
  let seen = seen_from_eye(world);
  o.through = seen.through;
  o.scattered = seen.scattered;
  o.past = seen.past;
  o.fill = seen.fill;
  return o;
}

@fragment
fn fs(i: VertexOut) -> @location(0) vec4f {
  let sprite = textureSample(sprites, sprite_sampler, i.uv, draw_layer());
  if (sprite.a < 0.5) {
    discard;
  }
  let n = normalize(i.normal);
  let color = shade(sprite.rgb, n, i.world_pos, sun_lit(i.world_pos, n), vec3f(1.0), Seen(i.through, i.scattered, i.past, i.fill));
  return vec4f(finish(color, i.clip.xy), draw_opacity());
}
