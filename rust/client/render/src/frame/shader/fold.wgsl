// pxl-render/frame/shader/fold.wgsl

// The four asset ids of each cell of the bricks the clip levels hold, one word for each cell
// (pxl-engine `clipmap::BrickSource::fold_at`, pxl-spec `ground::asset_ids`). A pass that
// draws by them binds them.

@group(0) @binding(4) var<storage, read> folds: array<u32>;

fn fold_at(i: u32) -> u32 {
  return folds[i];
}
