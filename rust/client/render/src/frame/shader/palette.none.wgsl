// pxl-render/palette.none.wgsl

// A world asked by a pass that reads no asset's second word binds no palette (palette.wgsl
// is the binding's form).

fn palette_word(i: u32) -> u32 {
  return 0u;
}
