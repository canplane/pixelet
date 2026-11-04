// pxl-render/light_aim.wgsl

// The light's head, a texel's ray and band and the word a slot's walk leaves, as the light's
// passes by hand take them (pxl-engine light.rs `ray`, `Light::band`, `depth`, `word`).

@group(0) @binding(11) var<uniform> head: array<vec4<i32>, LIGHT_HEAD_VEC4S>;

fn head_word(k: u32) -> u32 {
  return u32(head[k / 4u][k % 4u]);
}

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
fn gap(k: i32, half: f32) -> f32 {
  let a = f32(k) * f32(LIGHT_TILE);
  let b = f32(k + 1i) * f32(LIGHT_TILE);
  return max(max(a - half, half - b), 0.0);
}

/// The slot's texel of the box (`Light::texel`).
fn slot_texel(slot: vec2<u32>) -> vec2<u32> {
  return vec2<u32>(vec2<i32>(slot) - light_corner()) & vec2<u32>(head_word(LIGHT_HEAD_SIZE) - 1u);
}

fn light_corner() -> vec2<i32> {
  return vec2<i32>(i32(eye_word(LIGHT + LIGHT_CORNER_X)), i32(eye_word(LIGHT + LIGHT_CORNER_Y)));
}

/// The band of box tile `tile`, which may lie past the box (`Light::band`).
fn tile_band(tile: vec2<i32>) -> u32 {
  let half = f32(head_word(LIGHT_HEAD_SIZE)) / 2.0;
  let x = gap(tile.x, half) * length(light_v3(LIGHT_ACROSS));
  let y = gap(tile.y, half) * length(light_v3(LIGHT_DOWN));
  return zoom_at(x * x + y * y, vec2<u32>(eye_word(RANGE), eye_word(RANGE + 1u)));
}

/// The band of the tile a texel lies on (`Light::band`).
fn texel_band(texel: vec2<u32>) -> u32 {
  return tile_band(vec2<i32>(texel / LIGHT_TILE));
}

/// The word of a walk that stopped at `t` on `axis`: a share of the box's depth, pushed
/// back as its face's axis is (`light::depth`, `light::word`).
fn stop_word(t: f32, axis: u32) -> u32 {
  return depth_word(min(t / eye_f32(LIGHT + LIGHT_T_MAX) + light_v3(LIGHT_BIAS)[axis], 1.0));
}

fn depth_word(d: f32) -> u32 {
  return min(u32(d * f32(LIGHT_DEPTH_ONE) + 0.5), LIGHT_DEPTH_ONE);
}

/// The word a slot's ended walk leaves: the far plane where it stopped at nothing.
fn walked_word(w: Walked) -> u32 {
  return select(depth_word(1.0), stop_word(w.t, w.axis), w.ends == 1u);
}
