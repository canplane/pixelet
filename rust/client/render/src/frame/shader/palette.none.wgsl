// pxl-render/palette.none.wgsl

// A pass that asks the world but reads no asset's second word binds no palette, and every
// cell's palette word reads as 0. palette.wgsl binds the palette.

fn palette_word(i: u32) -> u32 {
  return 0u;
}
