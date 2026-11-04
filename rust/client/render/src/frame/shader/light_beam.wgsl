// pxl-render/light_beam.wgsl

// Each beam ray of the shadow map, a thread a corner (pxl-engine `light::BEAM`), walked along
// the light's rays (beam_walk.wgsl), its line left in the light's list.

const WALK_PASS: u32 = LIGHT_BEAM_PASS;
const VISITS_TALLY: u32 = LIGHT_BEAM_TALLY;
const ASKED_TALLY: u32 = LIGHT_ASKED_TALLY;

/// How far apart its beam rays lie, `light::BEAM` texels (`Light::spacing`).
fn light_spacing() -> f32 {
  return f32(LIGHT_BEAM) * length(light_v3(LIGHT_ACROSS));
}

/// Each beam ray of the shadow map the cast needs, a thread a corner (pxl-shader observer.rs
/// `beam`, light.rs `Map::corner`): from where its texel, `BEAM_WINDOW` beam rays before the
/// box's first, lies on the near plane, at its tile's band; its line, its origin, its visits
/// and how far the rays about it start (`Map::handed`, `light::margin`) left in the list.
@compute @workgroup_size(CORNER_SIDE, CORNER_SIDE, 1)
fn light_beam_cs(@builtin(global_invocation_id) id: vec3<u32>) {
  let across = beam_across();
  let c = light_take(LEVEL_CORNER, id.xy);
  if (c.x >= across || c.y >= across || !light_needs(LEVEL_CORNER, c)) {
    return;
  }
  let origin = texel_start(light_corner() + corner_texel(c));
  let k = c.y * across + c.x;
  let base = light_line(k);
  beams[base + LIGHT_LINE_ORIGIN] = u32(origin.x);
  beams[base + LIGHT_LINE_ORIGIN + 1u] = u32(origin.y);
  beams[base + LIGHT_LINE_ORIGIN + 2u] = u32(origin.z);
  let band = tile_band(light_on(LEVEL_CORNER, c));
  // The light's rays are parallel: their spacing its `across` alone (`Light::spacing`).
  let met = beam_walk(origin, light_v3(LIGHT_DIR), eye_f32(LIGHT + LIGHT_T_MAX), vec2<u32>(band), vec2<f32>(light_spacing(), 0.0), base);
  if (pyramid_word(TALLY_ON) != 0u) {
    atomicAdd(&pyramid[VISITS_TALLY], met.visits);
  }
  let handed = max(met.t - (LIGHT_MARGIN_PER_WIDTH * light_spacing() + LIGHT_MARGIN_STEPS), 0.0);
  beams[light_hand_at(k)] = bitcast<u32>(handed);
  beams[base + LINE_VISITS] = met.visits;
}
