// pxl-render/decor_compact.wgsl

// Compacts each record that its region drew a slot for into the decor's instances at that
// slot, reading the records as `decor_cnt_cs` does. An instance holds the middle of its box's
// base about the anchor, its heading and its scale, in the lanes the decor and impostor shaders
// read, and the index list holds its own slot.

@group(0) @binding(9) var<storage, read> recs: array<u32>;
@group(0) @binding(13) var<storage, read> drawing: array<u32>;
@group(0) @binding(14) var<storage, read_write> regions: array<atomic<u32>>;

/// The corner of the record at word `i` on `axis`, measured from the anchor in unit cells. On
/// X it is taken the short way round the world (`wrap_idx`).
fn from_anchor(i: u32, axis: u32) -> f32 {
  let d = i32(recs[i + 1u + axis]) - i32(drawing[DRAWING_ANCHOR + axis]);
  return f32(select(d, wrap_idx(LEAF_CELL_ZOOM, d), axis == 0u));
}

fn compact_one(i: u32) {
  let w = recs[i];
  let region = w >> DRAWING_REGION_SHIFT;
  if (region == DRAWING_NONE) {
    return;
  }
  let slot = atomicAdd(&regions[REGIONS + region], 1u);
  let cap = instance_cap();
  if (slot >= cap) {
    return;
  }
  let turn = rec_turn(w);
  let size = decor_size(rec_decor(w), rec_variant(w), turn);
  instances[slot] = bitcast<u32>(from_anchor(i, 0u) + f32(size.x) / 2.0);
  instances[cap + slot] = bitcast<u32>(from_anchor(i, 1u) + f32(size.y) / 2.0);
  // The base as drawn lies under the box's corner by the kept top of its column's surface.
  instances[2u * cap + slot] = bitcast<u32>(from_anchor(i, 2u) - f32(recs[i + 4u]) * (1.0 / f32(1u << FRAC_W)));
  instances[3u * cap + slot] = turn_spin(turn);
  instances[4u * cap + slot] = bitcast<u32>(1.0f);
  instances[DRAWING_COLUMNS * cap + slot] = slot;
}

@compute @workgroup_size(LANES)
fn decor_compact_cs(@builtin(workgroup_id) group: vec3<u32>, @builtin(local_invocation_id) lane: vec3<u32>) {
  if (!holds_recs(group.x)) {
    return;
  }
  for (var k = lane.x; k < RUNS; k += LANES) {
    let run = run_of(group.x, k);
    for (var n = 0u; n < run.y; n += 1u) {
      compact_one(rec_at(run.x + n));
    }
  }
}
