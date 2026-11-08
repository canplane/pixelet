// pxl-render/light_cell.wgsl

// Works out where the rays of each cell of the shadow map start (pxl-engine
// `beam::CELL_WORDS`), one thread for each cell. The starts lie past the beam's lines in the
// light's list (light_list.wgsl). cell.wgsl does the same for the eye.

@group(0) @binding(8) var<storage, read_write> beams: array<u32>;

/// Writes the start of cell `c`: the least of the beam rays' starts within its window, which
/// spans `2 + 2 · BEAM_WINDOW` beam rays a side from the cell's own corner, and the beam ray
/// that holds it, whose line the cell's rays resume from. The rays resume from that line only
/// where every beam ray of the window asked the band the cell's rays ask
/// (`cast::Detail::alike`). The light's beam has one spacing, so the band alone decides. Where
/// a beam ray asked another band, the cell's rays walk from the root. A beam ray lies a tile
/// from the next (`light::BEAM`), so the window's beam rays lie on every tile from its first's
/// to its last's, and `tiles_ask` checks them all. A workgroup is a square of the light's
/// list's lanes, as `light_item` reads them.
@compute @workgroup_size(SAMPLE_SIDE, SAMPLE_SIDE, 1)
fn light_cell_cs(@builtin(global_invocation_id) id: vec3<u32>) {
  let cells = header_word(LIGHT_HEADER_CELLS_ACROSS);
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
