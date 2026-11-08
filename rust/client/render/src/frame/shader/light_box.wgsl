// pxl-render/light_box.wgsl

// What the receivers' passes and the slide read of the light's box, beyond what
// light_aim.wgsl reads. A receiver is a surface that reads the shadow map. This part gives the
// cast's numbers from the head, the tile a receiver stands on (`Light::tile_of`), how far from
// the eye a surface receives the map and which tiles a box of receivers stands on
// (`Light::reach`, `Light::box_tiles`), and what a slide keeps (`Light::keeps`).

/// The map's side, in texels.
fn light_size() -> u32 {
  return header_word(LIGHT_HEADER_SIZE);
}

/// The cast's number, counted up from one each cast.
fn cast_number() -> u32 {
  return header_word(LIGHT_HEADER_CAST);
}

/// The number of the cast that began the epoch, since which the light, the eye's zooms and the
/// world stayed as they were cast for. What a cast of the epoch left in a tile stands while the
/// box keeps the tile (`light::HEADER_EPOCH`).
fn light_epoch() -> u32 {
  return header_word(LIGHT_HEADER_EPOCH);
}

/// Whether the cast's marks take each pixel's footprint (`light::HEADER_FOOTPRINT`).
fn footprint() -> bool {
  return header_word(LIGHT_HEADER_FOOTPRINT) != 0u;
}

/// Where a point `v` units from the eye lies on the box, in texels across and down from the
/// middle of texel (0, 0) (`Light::texel_at`).
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

/// Whether box tile `tile` keeps what a cast left in it, after the box slid to its corner from
/// `was` and the eye's zooms moved from those of the cast before (`Light::keeps`,
/// `light::HEADER_LAST_RANGE`). It keeps it where the tile lay in the box before as well and
/// its band did not change.
fn keeps_tile(tile: vec2<u32>, was: vec2<i32>) -> bool {
  let n = i32(light_tiles());
  let here = vec2<i32>(tile);
  let before = here + (light_corner() - was) / i32(LIGHT_TILE);
  let last_range = vec2<u32>(header_word(LIGHT_HEADER_LAST_RANGE), header_word(LIGHT_HEADER_LAST_RANGE + 1u));
  return all(before >= vec2<i32>(0)) && all(before < vec2<i32>(n))
    && tile_band_in(before, last_range) == tile_band(here);
}

/// How far from the eye a surface receives the map, in units (`Light::reach`). It is the
/// effects' share of the box's half-width, and one texel more for the normal offset at which a
/// receiver reads the map.
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

/// The box tiles a receiver may stand on: whether there are any, and the lowest and the
/// highest of them.
struct TileRange {
  got: bool,
  first: vec2<i32>,
  last: vec2<i32>,
}

/// The tiles that a receiver on the box from `lo` to `hi`, in units from the eye, stands on
/// (`Light::box_tiles`). They span the tiles its eight corners lie on, which hold every point
/// of it as the light's rays are parallel, and `LIGHT_FOOTPRINT_TILES` more each way where the
/// head asks for the footprint (`footprint`). There are none where the box lies past the reach
/// or past the light's box.
fn box_tiles(lo: vec3<f32>, hi: vec3<f32>) -> TileRange {
  if (!light_within(lo, hi, light_reach())) {
    return TileRange(false, vec2<i32>(0), vec2<i32>(0));
  }
  return corner_tiles(lo, hi);
}

/// The tiles the box from `lo` to `hi` stands on, whatever the reach (`Light::corner_tiles`).
fn corner_tiles(lo: vec3<f32>, hi: vec3<f32>) -> TileRange {
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
  return TileRange(all(first <= last), first, last);
}
