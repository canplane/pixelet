// pxl-render/light_cell.wgsl

// Where each cell of the shadow map's rays start (pxl-engine `beam::CELL_WORDS`), a thread a
// cell, past the beam's lines in the light's list (light_list.wgsl). The eye's are cell.wgsl's.

@group(0) @binding(8) var<storage, read_write> beams: array<u32>;

/// Cell `c`'s start: the least of the beam rays' starts within its window, `2 + 2 ·
/// BEAM_WINDOW` beam rays a side from its own corner, and that beam ray, whose line its rays
/// resume from; and whether they do, only where every beam ray of the window asked the band
/// its rays ask (`cast::Detail::alike`): the beam's spacing is one, so only its band tells. A
/// workgroup is a square of the light's list's lanes, as `light_item` reads them.
@compute @workgroup_size(SAMPLE_SIDE, SAMPLE_SIDE, 1)
fn light_cell_cs(@builtin(global_invocation_id) id: vec3<u32>) {
  let cells = head_word(LIGHT_HEAD_CELLS_ACROSS);
  let c = light_item(LEVEL_CELL, id.xy);
  if (c.x >= cells || c.y >= cells || !light_needs(LEVEL_CELL, c)) {
    return;
  }
  let across = beam_across();
  let side = 2u + 2u * LIGHT_BEAM_WINDOW;
  let asks = tile_band(light_on(LEVEL_CELL, c));
  var least_at = c.y * across + c.x;
  var least = bitcast<f32>(beams[light_hand_at(least_at)]);
  var alike = true;
  for (var j = 0u; j < side; j += 1u) {
    for (var i = 0u; i < side; i += 1u) {
      let k = (c.y + j) * across + c.x + i;
      let t = bitcast<f32>(beams[light_hand_at(k)]);
      if (t < least) {
        least = t;
        least_at = k;
      }
      alike = alike && tile_band(light_on(LEVEL_CORNER, c + vec2<u32>(i, j))) == asks;
    }
  }
  let at = light_cell_at(c);
  beams[at + CELL_T] = bitcast<u32>(least);
  beams[at + CELL_CORNER] = least_at;
  beams[at + CELL_RESUMES] = u32(alike);
}
