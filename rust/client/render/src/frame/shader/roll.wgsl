// pxl-render/roll.wgsl

// The close of a frame the eye's queue runs in, its rolled lattice's words kept from cast to
// cast while its grid stands (pxl-engine `beam::ROLLS`, `node_cache::ROLL_K`): where the frame cast
// its samples, the offset they were cast at is counted where their run left none parked
// (`node_cache::LATTICE_LEFT`), and the next offset not counted taken; else the next cast's
// samples go on from where they were parked, at the same offset. Every cast is counted, so the
// next cast's beam rays go on from this one's (`node_cache::ROLL_CASTS`). In every such frame, the eye
// owes its lattice where an offset is not counted (`count::LATTICE_OWED`). One thread, after the
// eye's owed pixels were listed off its owed word (gpu/ray/cast.rs `dispatch`).

@group(0) @binding(9) var<storage, read_write> node_cache: array<atomic<u32>>;

/// The eye's lattice owed where an offset is not counted.
fn owe() {
  if (atomicLoad(&node_cache[ROLL_DONE]) != ROLLS_DONE) {
    atomicOr(&node_cache[EYE_OWED], LATTICE_OWED);
  }
}

@compute @workgroup_size(1)
fn roll_cs() {
  atomicAdd(&node_cache[ROLL_CASTS], 1u);
  if (atomicLoad(&node_cache[LATTICE_LEFT]) != 0u) {
    atomicStore(&node_cache[ROLL_RESUMES], 1u);
    owe();
    return;
  }
  let k = atomicLoad(&node_cache[ROLL_K]);
  let done = atomicLoad(&node_cache[ROLL_DONE]) | (1u << k);
  atomicStore(&node_cache[ROLL_DONE], done);
  // The first offset after it in the rolled lattice's order not counted, itself where every
  // one is.
  var next = k;
  for (var j = 1u; j < ROLLS; j += 1u) {
    let c = (k + j) % ROLLS;
    if ((done & (1u << c)) == 0u) {
      next = c;
      break;
    }
  }
  atomicStore(&node_cache[ROLL_K], next);
  let offset = roll_offset(next);
  atomicStore(&node_cache[ROLL_AT], bitcast<u32>(offset.x));
  atomicStore(&node_cache[ROLL_AT + 1u], bitcast<u32>(offset.y));
  atomicStore(&node_cache[ROLL_RESUMES], 0u);
  owe();
}

@compute @workgroup_size(1)
fn roll_owed_cs() {
  owe();
}
