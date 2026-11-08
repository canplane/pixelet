// pxl-render/palette.wgsl

// The four palette indices of each cell of the bricks the clip levels hold, one word for each
// cell (pxl-engine `clipmap::BrickSource::palette_word`, `ground::palette_indices`). A pass that
// draws by them binds them.

@group(0) @binding(4) var<storage, read> palette_indices: array<u32>;

fn palette_word(i: u32) -> u32 {
  return palette_indices[i];
}
