// pxl-render/decor_compact.wgsl

// Each record its region drew a slot of, read as `decor_cnt_cs` reads them, compacted into the
// decor's instances at that slot: its box's base middle about the anchor, its heading and scale,
// in the lanes the decor and impostor shaders read, and the slot its own in the index list.

@group(0) @binding(9) var<storage, read> records: array<u32>;
@group(0) @binding(13) var<storage, read> drawing: array<u32>;
@group(0) @binding(14) var<storage, read_write> regions: array<atomic<u32>>;
@group(0) @binding(15) var<storage, read_write> instances: array<u32>;

/// The corner of the record at word `i` on `axis` about the anchor, in unit cells, X the short
/// way round the world (`wrap_idx`).
fn from_anchor(i: u32, axis: u32) -> f32 {
  let d = i32(records[i + 1u + axis]) - i32(drawing[DRAWN_ANCHOR + axis]);
  return f32(select(d, wrap_idx(LEAF_CELL, d), axis == 0u));
}

/// `decor_compact_cs` for the record at word `i`.
fn compact_one(i: u32) {
  let w = records[i];
  let region = w >> DRAWN_REGION_SHIFT;
  if (region == DRAWN_NONE) {
    return;
  }
  let slot = atomicAdd(&regions[REGIONS + region], 1u);
  let cap = arrayLength(&instances) / (DRAWN_COLUMNS + 1u);
  if (slot >= cap) {
    return;
  }
  let turn = record_turn(w);
  let size = cells(record_decor(w), record_variant(w), turn);
  instances[slot] = bitcast<u32>(from_anchor(i, 0u) + f32(size.x) / 2.0);
  instances[cap + slot] = bitcast<u32>(from_anchor(i, 1u) + f32(size.y) / 2.0);
  // Its base as drawn: under its box's corner by its column surface's kept top.
  instances[2u * cap + slot] = bitcast<u32>(from_anchor(i, 2u) - f32(records[i + 4u]) / f32(1u << FRAC_W));
  instances[3u * cap + slot] = turn_spin(turn);
  instances[4u * cap + slot] = bitcast<u32>(1.0f);
  instances[DRAWN_COLUMNS * cap + slot] = slot;
}

@compute @workgroup_size(LANES)
fn decor_compact_cs(@builtin(workgroup_id) group: vec3<u32>, @builtin(local_invocation_id) lane: vec3<u32>) {
  if (!is_held(group.x)) {
    return;
  }
  for (var k = lane.x; k < RUNS; k += LANES) {
    let run = run_of(group.x, k);
    for (var n = 0u; n < run.y; n += 1u) {
      compact_one(record_at(run.x + n));
    }
  }
}
