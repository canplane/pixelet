// pxl-render/light_box.wgsl

// The light's box past what light_aim.wgsl reads of it, as the receivers' passes and the
// slide take it: the cast's numbers off the head, where a receiver stands
// (`Light::tile_of`) and what a slide keeps (`Light::keeps`).

/// The map's side (`light::HEAD_SIZE`).
fn light_size() -> u32 {
  return head_word(LIGHT_HEAD_SIZE);
}

/// The cast's number (`light::HEAD_CAST`).
fn cast_number() -> u32 {
  return head_word(LIGHT_HEAD_CAST);
}

/// The number of the cast that began its epoch (`light::HEAD_EPOCH`).
fn light_epoch() -> u32 {
  return head_word(LIGHT_HEAD_EPOCH);
}

/// Whether the cast's marks take each pixel's footprint (`light::HEAD_FOOTPRINT`).
fn footprint() -> bool {
  return head_word(LIGHT_HEAD_FOOTPRINT) != 0u;
}

/// Where a point `v` units from the eye lies on the box, in texels across and down
/// from texel (0, 0)'s middle (`Light::texel_at`).
fn texel_at(v: vec3<f32>) -> vec2<f32> {
  let d = v - light_v3(0u);
  let across = dot(d, light_v3(LIGHT_ACROSS)) * eye_f32(LIGHT + LIGHT_ACROSS2_INVERSE);
  let down = dot(d, light_v3(LIGHT_DOWN)) * eye_f32(LIGHT + LIGHT_DOWN2_INVERSE);
  return vec2<f32>(across, down);
}

/// The tile of the box a receiver standing `v` units from the eye stands on
/// (`Light::tile_of`).
fn tile_of(v: vec3<f32>) -> vec2<i32> {
  return vec2<i32>(floor((texel_at(v) + vec2<f32>(0.5)) * (1.0 / f32(LIGHT_TILE))));
}

/// Whether box tile `tile` keeps what a cast left in it where the box slid to its
/// corner from `was` (`Light::keeps`).
fn keeps_tile(tile: vec2<u32>, was: vec2<i32>) -> bool {
  let n = i32(light_tiles());
  let here = vec2<i32>(tile);
  let held = here + (light_corner() - was) / i32(LIGHT_TILE);
  return all(held >= vec2<i32>(0)) && all(held < vec2<i32>(n)) && tile_band(held) == tile_band(here);
}
