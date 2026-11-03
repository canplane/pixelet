// pxl-render/light_aim.wgsl

// A listed slot's ray and the word its walk leaves there, as `light_miss_hit_cs` and
// `light_miss_cs` take them (pxl-engine light.rs `ray`, `Light::band`, `depth`, `word`).

@group(0) @binding(10) var light: texture_storage_2d_array<r32uint, read_write>;
@group(0) @binding(11) var<uniform> head: array<vec4<i32>, LIGHT_HEAD_VEC4S>;

fn light_v3(k: u32) -> vec3<f32> {
  return eye_v3(LIGHT + k);
}

fn head_v3(k: u32) -> vec3<f32> {
  let v = head[LIGHT_STARTS_AT + k];
  return vec3<f32>(bitcast<f32>(v.x), bitcast<f32>(v.y), bitcast<f32>(v.z));
}

/// Where the ray of world texel `w` starts (`Starts::start`).
fn texel_start(w: vec2<i32>) -> vec3<i32> {
  let base = head[LIGHT_STARTS_AT].xy;
  let d = w - base;
  let k = d >= vec2<i32>(i32(LIGHT_ANCHOR));
  let ij = vec2<f32>(d - select(vec2<i32>(0), vec2<i32>(i32(LIGHT_ANCHOR)), k));
  let anchor = u32(k.x) + 2u * u32(k.y);
  let at = head[LIGHT_STARTS_AT + STARTS_ANCHORS + anchor].xyz;
  let step = round(head_v3(STARTS_FRACS + anchor) + ij.x * head_v3(STARTS_ACROSS) + ij.y * head_v3(STARTS_DOWN));
  return at + vec3<i32>(step);
}

/// The texels from the box's middle to tile `k`'s span on one axis (`Light::band`).
fn gap(k: u32, half: f32) -> f32 {
  let a = f32(k * LIGHT_TILE);
  let b = f32((k + 1u) * LIGHT_TILE);
  return max(max(a - half, half - b), 0.0);
}

/// Where the listed slots start in the list, their count first (`light::misses_cnt`).
fn misses_cnt() -> u32 {
  return u32(head[LIGHT_HEAD_MISSES / 4u][LIGHT_HEAD_MISSES % 4u]);
}

/// The slot's texel of the box (`Light::texel`).
fn slot_texel(slot: vec2<u32>) -> vec2<u32> {
  let size = u32(head[LIGHT_HEAD_SIZE / 4u][LIGHT_HEAD_SIZE % 4u]);
  return vec2<u32>(vec2<i32>(slot) - light_corner()) & vec2<u32>(size - 1u);
}

fn light_corner() -> vec2<i32> {
  return vec2<i32>(i32(eye_word(LIGHT + LIGHT_CORNER_X)), i32(eye_word(LIGHT + LIGHT_CORNER_Y)));
}

/// The band of the tile a texel lies on (`Light::band`).
fn texel_band(texel: vec2<u32>) -> u32 {
  let tile = texel / LIGHT_TILE;
  let half = f32(u32(head[LIGHT_HEAD_SIZE / 4u][LIGHT_HEAD_SIZE % 4u])) / 2.0;
  let x = gap(tile.x, half) * length(light_v3(LIGHT_ACROSS));
  let y = gap(tile.y, half) * length(light_v3(LIGHT_DOWN));
  return zoom_at(x * x + y * y, vec2<u32>(eye_word(RANGE), eye_word(RANGE + 1u)));
}

/// The word of a walk that stopped at `t` on `axis`: a share of the box's depth, pushed
/// back as its face's axis is (`light::depth`, `light::word`).
fn stop_word(t: f32, axis: u32) -> u32 {
  return depth_word(min(t / eye_f32(LIGHT + LIGHT_T_MAX) + light_v3(LIGHT_BIAS)[axis], 1.0));
}

fn depth_word(d: f32) -> u32 {
  return min(u32(d * f32(LIGHT_DEPTH_ONE) + 0.5), LIGHT_DEPTH_ONE);
}
