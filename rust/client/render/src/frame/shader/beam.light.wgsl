// pxl-render/beam.light.wgsl

// The shadow map as its beam rays' walks take it, off its list (light_list.wgsl): a beam ray
// every `light::BEAM` texels, `BEAM_WINDOW` of them before the box's first, taken off what the
// cast casts, each along the light from where its texel lies on the near plane, at its tile's
// band (pxl-engine `light::corner_at`, `Light::band`), its line in the light's list.

const WALK_PASS: u32 = LIGHT_BEAM_PASS;
const VISITS_COUNT: u32 = LIGHT_BEAM_COUNT;
const ASKED_COUNT: u32 = LIGHT_ASKED_COUNT;

/// The beam rays across the map, and down (`light::beam_across`).
fn beam_lattice() -> vec2<u32> {
  return vec2<u32>(beam_across());
}

/// The beam ray the thread at `id` walks: its own where the cast begins its epoch, else one of
/// the corners listed, `NONE` past them (light_list.wgsl `light_item`).
fn beam_item(id: vec2<u32>) -> vec2<u32> {
  return light_item(LEVEL_CORNER, id);
}

/// Whether a cell whose window holds beam ray `c` is one this cast casts (`light_needs`).
fn beam_needed(c: vec2<u32>) -> bool {
  return light_needs(LEVEL_CORNER, c);
}

/// The first word of beam ray `c`'s line (`light::line`).
fn beam_line(c: vec2<u32>) -> u32 {
  return light_line(c.y * beam_across() + c.x);
}

/// The word of how far the rays about beam ray `c` start (`light::hand_at`).
fn beam_hand_at(c: vec2<u32>) -> u32 {
  return light_hand_at(c.y * beam_across() + c.x);
}

/// The list of the beam rays the walk over the node cache left (`light::corners_left_at`).
fn corners_left_at() -> u32 {
  return head_word(LIGHT_HEAD_CORNERS_LEFT);
}

/// Beam ray `c`'s ray, at the band of the tile its texel, which may lie past the box, lies on.
fn beam_aim(c: vec2<u32>) -> Aim {
  let band = tile_band(light_on(LEVEL_CORNER, c));
  return light_ray(corner_texel(c), band, mix_hash(head_word(LIGHT_HEAD_CAST), c.x | (c.y << 16u)));
}

/// How far apart its beam rays lie, `light::BEAM` texels (`Light::spacing`): the light's rays
/// are parallel, their spacing across alone.
fn beam_spacing() -> vec2<f32> {
  return vec2<f32>(f32(LIGHT_BEAM) * eye_f32(LIGHT + LIGHT_WIDTH), 0.0);
}

/// The origin each line is held from, as every light ray starts apart (`light::LINE_ORIGIN`).
fn beam_lay_origin(base: u32, origin: vec3<i32>) {
  beams[base + LIGHT_LINE_ORIGIN] = u32(origin.x);
  beams[base + LIGHT_LINE_ORIGIN + 1u] = u32(origin.y);
  beams[base + LIGHT_LINE_ORIGIN + 2u] = u32(origin.z);
}

/// How far short of where a beam ray stopped at `t` its cells' texels start, none before the
/// near plane (`light::margin`): the product rounded, then the Q3 steps added, as the engine
/// lays it. Metal may fuse the two into one rounding. tests/margin.rs names each rounding a pass
/// takes.
fn beam_hand(t: f32) -> f32 {
  return max(t - (LIGHT_MARGIN_PER_WIDTH * beam_spacing().x + LIGHT_MARGIN_STEPS), 0.0);
}
