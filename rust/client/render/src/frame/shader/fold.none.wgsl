// pxl-render/fold.none.wgsl

// A pass that asks the world but reads no asset's second word binds no folds, and every
// cell's fold reads as 0. fold.wgsl binds them.

fn fold_at(i: u32) -> u32 {
  return 0u;
}
