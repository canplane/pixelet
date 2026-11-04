// pxl-render/palette.wgsl

// A world's cells' four palette indices, read as the Rust's `PaletteWords` reads them: a
// pass that draws by them binds them.

@group(0) @binding(4) var<storage, read> palette_indices: array<u32>;

fn palette_word(i: u32) -> u32 {
  return palette_indices[i];
}
