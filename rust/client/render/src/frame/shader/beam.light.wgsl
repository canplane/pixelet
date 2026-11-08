// pxl-render/beam.light.wgsl

// The shadow map's beam rays, as the beam rays' walks read them from the light's list
// (corner.wgsl, light_list.wgsl). A beam ray starts over every `light::BEAM`-th texel, from
// `BEAM_WINDOW` of them before the box's first (pxl-engine `light::corner_at`). A run takes only
// those listed for the tiles the cast claimed. Each goes along the light from where its texel
// lies on the near plane, at its tile's band (`Light::band`), and keeps its line in the light's
// list.

fn beam_lattice() -> vec2<u32> {
  return vec2<u32>(beam_across());
}

/// How many beam rays a run takes: those listed for the tiles the cast claimed, one a place in
/// the list (light_list.wgsl `light_item_at`).
fn corner_cnt() -> u32 {
  return in_cap(beams[list_cnt_at(LIGHT_LIST_CORNERS)], beam_across() * beam_across());
}

fn corner_queued(k: u32) -> vec2<u32> {
  return light_item_at(LEVEL_CORNER, k);
}

fn corner_state_at(c: vec2<u32>) -> u32 {
  return header_word(LIGHT_HEADER_CORNER_STATES) + (c.y * beam_across() + c.x) * STATE_WORDS;
}

fn beam_line(c: vec2<u32>) -> u32 {
  return light_line(c.y * beam_across() + c.x);
}

/// The word that holds how far the rays about beam ray `c` start (`light::hand_at`).
fn beam_hand_at(c: vec2<u32>) -> u32 {
  return light_hand_at(c.y * beam_across() + c.x);
}

/// Beam ray `c`'s ray, at the band of the tile its texel lies on, which may lie past the box.
/// It keeps its line at `beam_line`.
fn beam_aim(c: vec2<u32>) -> Aim {
  let band = tile_band(light_on(LEVEL_CORNER, c));
  return beam_ray(light_ray(corner_texel(c), band), beam_line(c), beam_spacing());
}

/// How far apart the light's beam rays lie: `light::BEAM` texels across, and none in angle, as
/// the light's rays are parallel (`Light::spacing`).
fn beam_spacing() -> vec2<f32> {
  return vec2<f32>(f32(LIGHT_BEAM) * eye_f32(LIGHT + LIGHT_WIDTH), 0.0);
}

/// Lays the origin a line's nodes are held from, as every light ray starts from its own
/// (`light::LINE_ORIGIN`).
fn beam_lay_origin(base: u32, origin: vec3<i32>) {
  beams[base + LIGHT_LINE_ORIGIN] = u32(origin.x);
  beams[base + LIGHT_LINE_ORIGIN + 1u] = u32(origin.y);
  beams[base + LIGHT_LINE_ORIGIN + 2u] = u32(origin.z);
}

/// Where the texels of the cells about a beam ray that stopped at `t` start: a margin short of
/// `t`, and never before the near plane (`light::margin`). The product is rounded, and the Q3
/// steps are added after, as the engine computes it. Metal may fuse the two into one rounding;
/// tests/margin.rs names each rounding a pass takes.
fn beam_hand(t: f32) -> f32 {
  return max(t - (LIGHT_MARGIN_PER_WIDTH * beam_spacing().x + LIGHT_MARGIN_STEPS), 0.0);
}

