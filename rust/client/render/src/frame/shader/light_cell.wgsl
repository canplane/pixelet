// pxl-render/light_cell.wgsl

// Where each cell of the shadow map's rays start (pxl-engine `beam::CELL_WORDS`), a thread a
// cell, past the beam's lines in the light's list (light_list.wgsl). The eye's are cell.wgsl's.

@group(0) @binding(8) var<storage, read_write> beams: array<u32>;

/// Cell `c`'s start: the least of the beam rays' starts within its window, `2 + 2 ·
/// BEAM_WINDOW` beam rays a side from its own corner, and that beam ray, whose line its rays
/// resume from; and whether they do, where every beam ray of the window ended and asked the
/// band its rays ask (`cast::Detail::alike`), the beam's spacing one, so only its band tells;
/// its rays walk from the root where one asked another band.
/// A beam ray lies a tile from the next (`light::BEAM`), so the window's beam rays lie on
/// every tile from its first's to its last's (`tiles_ask`). A workgroup is a square of the
/// light's list's lanes, as `light_item` reads them.
@compute @workgroup_size(SAMPLE_SIDE, SAMPLE_SIDE, 1)
fn light_cell_cs(@builtin(global_invocation_id) id: vec3<u32>) {
  let cells = head_word(LIGHT_HEAD_CELLS_ACROSS);
  let c = light_item(LEVEL_CELL, id.xy);
  if (c.x >= cells || c.y >= cells) {
    return;
  }
  let side = 2u + 2u * LIGHT_BEAM_WINDOW;
  let least = window_least(light_hand_at(0u), beam_across(), c, side);
  let lo = light_on(LEVEL_CORNER, c);
  let hi = light_on(LEVEL_CORNER, c + vec2<u32>(side - 1u));
  let at = light_cell_at(c);
  beams[at + CELL_T] = least.x;
  beams[at + CELL_CORNER] = least.y;
  beams[at + CELL_RESUMES] = u32(tiles_ask(lo, hi, tile_band(light_on(LEVEL_CELL, c))));
}
