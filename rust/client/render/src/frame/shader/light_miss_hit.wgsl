// pxl-render/light_miss_hit.wgsl

// The slots `light_cs` listed, their walk from the root over the observed pyramid alone
// (walk.wgsl). One that ends writes its word and is struck from the list; `light_miss_cs`
// walks what is left (pxl-shader light.rs `light_miss_cs`, pxl-engine light.rs `depth`).

@group(0) @binding(10) var light: texture_storage_2d_array<r32uint, read_write>;
@group(0) @binding(11) var<uniform> head: array<vec4<i32>, LIGHT_HEAD_VEC4S>;
/// `light_miss_cs`'s dispatch: its rows, up to the last it has to walk.
@group(0) @binding(13) var<storage, read_write> left: array<atomic<u32>, 4>;

fn light_v3(k: u32) -> vec3<f32> {
  return eye_v3(LIGHT + k);
}

fn head_v3(k: u32) -> vec3<f32> {
  let v = head[LIGHT_STARTS_AT + k];
  return vec3<f32>(bitcast<f32>(v.x), bitcast<f32>(v.y), bitcast<f32>(v.z));
}

/// Where the ray of world texel `w` starts (`Starts::start`).
fn start(w: vec2<i32>) -> vec3<i32> {
  let base = head[LIGHT_STARTS_AT].xy;
  let d = w - base;
  let k = d >= vec2<i32>(i32(LIGHT_ANCHOR));
  let ij = vec2<f32>(d - select(vec2<i32>(0), vec2<i32>(i32(LIGHT_ANCHOR)), k));
  let anchor = u32(k.x) + 2u * u32(k.y);
  let at = head[LIGHT_STARTS_AT + 3u + anchor].xyz;
  let step = round(head_v3(7u + anchor) + ij.x * head_v3(1u) + ij.y * head_v3(2u));
  return at + vec3<i32>(step);
}

/// The texels from the box's middle to tile `k`'s span on one axis (`Light::band`).
fn gap(k: u32, half: f32) -> f32 {
  let a = f32(k * LIGHT_TILE);
  let b = f32((k + 1u) * LIGHT_TILE);
  return max(max(a - half, half - b), 0.0);
}

@compute @workgroup_size(64, 1, 1)
fn light_miss_hit_cs(
  @builtin(workgroup_id) group: vec3<u32>,
  @builtin(local_invocation_id) lane: vec3<u32>,
) {
  let misses = u32(head[LIGHT_HEAD_MISSES / 4u][LIGHT_HEAD_MISSES % 4u]);
  let k = (group.y * MISS_ACROSS + group.x) * MISS_LANES + lane.x;
  if (k >= beams[misses]) {
    return;
  }
  let listed = beams[misses + 1u + k];
  let slot = vec2<u32>(listed & 0xffffu, listed >> 16u);
  let size = u32(head[0].w);
  let corner = vec2<i32>(i32(eye_word(LIGHT + 7u)), i32(eye_word(LIGHT + 11u)));
  let texel = vec2<u32>(vec2<i32>(slot) - corner) & vec2<u32>(size - 1u);
  let tile = texel / LIGHT_TILE;
  let half = f32(size) / 2.0;
  let x = gap(tile.x, half) * length(light_v3(4u));
  let y = gap(tile.y, half) * length(light_v3(8u));
  let band = zoom_at(x * x + y * y, vec2<u32>(eye_word(RANGE), eye_word(RANGE + 1u)));
  let t_max = eye_f32(LIGHT + 3u);
  let w = walk(
    Aim(start(corner + vec2<i32>(texel)), light_v3(12u), t_max, vec2<u32>(band), mix_hash(u32(head[0].x), listed)),
    vec2<u32>(NONE),
    NONE,
  );
  if (w.ends == 0u || w.ends == 4u) {
    atomicMax(&left[1], group.y + 1u);
    return;
  }
  // Where it stopped, as a share of the box's depth, pushed back as its face's axis is;
  // the far plane where it stopped at nothing (`light::depth`, `light::word`).
  var d = 1.0;
  if (w.ends == 1u) {
    d = min(w.t / t_max + light_v3(16u)[w.axis], 1.0);
  }
  let q = min(u32(d * f32(LIGHT_DEPTH_ONE) + 0.5), LIGHT_DEPTH_ONE);
  textureStore(light, slot, 0, vec4<u32>(q, 0u, 0u, 0u));
  beams[misses + 1u + k] = LIGHT_MISS_DONE;
}
