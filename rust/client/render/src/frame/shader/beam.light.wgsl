// pxl-render/beam.light.wgsl

// The shadow map as its beam rays' walks take it, off its list (light_list.wgsl): a beam ray
// every `light::BEAM` texels, `BEAM_WINDOW` of them before the box's first, taken off what the
// cast casts, each along the light from where its texel lies on the near plane, at its tile's
// band (pxl-engine `light::corner_at`, `Light::band`), its line in the light's list.

/// The beam rays across the map, and down (`light::beam_across`).
fn beam_lattice() -> vec2<u32> {
  return vec2<u32>(beam_across());
}

/// The beam rays a run takes: those listed for the tiles the cast claimed, a beam ray a place
/// (light_list.wgsl `light_item_at`).
fn corner_cnt() -> u32 {
  return in_cap(beams[list_cnt_at(LIGHT_LIST_CORNERS)], beam_across() * beam_across());
}

fn corner_queued(k: u32) -> vec2<u32> {
  return light_item_at(LEVEL_CORNER, k);
}

/// The first word of beam ray `c`'s parked state (`light::corner_states_at`).
fn corner_state_at(c: vec2<u32>) -> u32 {
  return header_word(LIGHT_HEADER_CORNER_STATES) + (c.y * beam_across() + c.x) * STATE_WORDS;
}

/// The first word of beam ray `c`'s line (`light::line`).
fn beam_line(c: vec2<u32>) -> u32 {
  return light_line(c.y * beam_across() + c.x);
}

/// The word of how far the rays about beam ray `c` start (`light::hand_at`).
fn beam_hand_at(c: vec2<u32>) -> u32 {
  return light_hand_at(c.y * beam_across() + c.x);
}

/// Beam ray `c`'s ray, at the band of the tile its texel, which may lie past the box, lies on,
/// its line `beam_line`.
fn beam_aim(c: vec2<u32>) -> Aim {
  let band = tile_band(light_on(LEVEL_CORNER, c));
  return beam_ray(light_ray(corner_texel(c), band), beam_line(c), beam_spacing());
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

