// pxl-render/no_palette.wgsl

// A world asked by a pass that reads no asset's second word binds no palette (pxl-shader
// world.rs `NoPalette`).

fn palette_word(i: u32) -> u32 {
  return 0u;
}
