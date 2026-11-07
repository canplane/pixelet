// pxl-render/light_box.wgsl

// The light's box past what light_aim.wgsl reads of it, as the receivers' passes and the
// slide take it: the cast's numbers off the head, where a receiver stands
// (`Light::tile_of`), how far from the eye one receives and the tiles a box of them stands
// on (`Light::reach`, `Light::box_tiles`), and what a slide keeps (`Light::keeps`).

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
/// corner from `was` and the eye's zooms moved from the cast before's (`Light::keeps`,
/// `light::HEAD_RANGE_WAS`).
fn keeps_tile(tile: vec2<u32>, was: vec2<i32>) -> bool {
  let n = i32(light_tiles());
  let here = vec2<i32>(tile);
  let held = here + (light_corner() - was) / i32(LIGHT_TILE);
  let range_was = vec2<u32>(head_word(LIGHT_HEAD_RANGE_WAS), head_word(LIGHT_HEAD_RANGE_WAS + 1u));
  return all(held >= vec2<i32>(0)) && all(held < vec2<i32>(n))
    && tile_band_in(held, range_was) == tile_band(here);
}

/// How far from the eye a surface receives the map, in units (`Light::reach`): the effects'
/// share of the box's half-width and a texel past it.
fn light_reach() -> f32 {
  let width = eye_f32(LIGHT + LIGHT_WIDTH);
  return eye_f32(LIGHT + LIGHT_REACH_NDC) * width * (f32(light_size()) * 0.5) + width;
}

/// Whether the box from `lo` to `hi`, units from the eye, comes within `reach` of it
/// (`Light::within`).
fn light_within(lo: vec3<f32>, hi: vec3<f32>, reach: f32) -> bool {
  let gap = max(max(lo, -hi), vec3<f32>(0.0));
  return gap.x * gap.x + gap.y * gap.y + gap.z * gap.z <= reach * reach;
}

/// The box's tiles a receiver may stand on: whether any, the lowest and the highest.
struct BoxTiles {
  got: bool,
  first: vec2<i32>,
  last: vec2<i32>,
}

/// The tiles a receiver on the box from `lo` to `hi`, units from the eye, stands on
/// (`Light::box_tiles`): those its eight corners lie on, and `LIGHT_FOOTPRINT_TILES` more
/// each way where the head asks the footprint (`footprint`); none past the reach or the box.
fn box_tiles(lo: vec3<f32>, hi: vec3<f32>) -> BoxTiles {
  if (!light_within(lo, hi, light_reach())) {
    return BoxTiles(false, vec2<i32>(0), vec2<i32>(0));
  }
  return corner_tiles(lo, hi);
}

/// The tiles the box from `lo` to `hi` stands on, whatever the reach (`Light::corner_tiles`).
fn corner_tiles(lo: vec3<f32>, hi: vec3<f32>) -> BoxTiles {
  var first = vec2<i32>(2147483647i);
  var last = vec2<i32>(-2147483647i);
  for (var k = 0u; k < 8u; k += 1u) {
    let v = select(lo, hi, vec3<bool>((k & 1u) != 0u, (k & 2u) != 0u, (k & 4u) != 0u));
    let t = tile_of(v);
    first = min(first, t);
    last = max(last, t);
  }
  let grow = vec2<i32>(select(0i, LIGHT_FOOTPRINT_TILES, footprint()));
  let n = i32(light_tiles()) - 1i;
  first = max(first - grow, vec2<i32>(0));
  last = min(last + grow, vec2<i32>(n));
  return BoxTiles(all(first <= last), first, last);
}
