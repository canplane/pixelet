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
  _p1: u32,
  _p2: u32,
};

@group(1) @binding(0) var<uniform> draw: Draw;
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
};

fn lane(k: u32, i: u32) -> f32 {
  return bitcast<f32>(block[draw.offset + k * draw.count + i]);
}

// glTF is Y-up: a quarter turn about X, (x, y, z) to (x, -z, y), then the
// instance's spin about Z.
fn place(v: vec3f, spin: f32) -> vec3f {
  let up = vec3f(v.x, -v.z, v.y);
  let c = cos(spin);
  let s = sin(spin);
  return vec3f(up.x * c - up.y * s, up.x * s + up.y * c, up.z);
}

@vertex
fn vs(v: VertexIn) -> VertexOut {
  let n = draw.count;
  let flags = (block[draw.offset + 5u * n + v.i / 4u] >> ((v.i % 4u) * 8u)) & 255u;
  var o: VertexOut;
  if ((flags & 1u) != 0u) {
    o.clip = vec4f(0.0, 0.0, -2.0, 1.0);
    return o;
  }
  let at = vec3f(lane(0u, v.i), lane(1u, v.i), lane(2u, v.i));
  let spin = lane(3u, v.i);
  let scale = lane(4u, v.i);
  let world = place(v.position * scale, spin) + at + draw.translation;
  o.clip = frame.view_proj * vec4f(world, 1.0);
  o.uv = v.uv;
  o.normal = place(v.normal, spin);
  o.world_pos = world;
  return o;
}

@fragment
fn fs(i: VertexOut) -> @location(0) vec4f {
  let albedo = textureSample(skin, skin_sampler, i.uv).rgb;
  let n = normalize(i.normal);
  let color = shade(albedo, n, i.world_pos, 1.0 / i.clip.w, sun_lit(i.world_pos, n));
  return vec4f(finish(color, i.clip.xy), draw.opacity);
}
