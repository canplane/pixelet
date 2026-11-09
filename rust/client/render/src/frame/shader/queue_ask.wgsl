// pxl-render/queue_ask.wgsl

// Asks the world once for each node a run's walk was parked at (queue_walk.wgsl, `ask_once`).
// A lane takes one listed node at a time. The lanes are a fixed dispatch (pxl-engine
// `ray::QUEUE_GROUPS`) that takes the list in strides. What the pass keeps is in the node
// cache when the next walk goes on from the parked states, after this dispatch ends.

/// The run's queue, bound from its head (pxl-engine `ray::QUEUE_WORDS`, `ray::queue_at`). Past
/// the heads lie the asks' records, which every queue shares (`queue_records_at`).
@group(0) @binding(10) var<storage, read_write> queue: array<atomic<u32>>;

/// Asks the world once, for all the rays the queue parked at it, for the node `c` at `zoom`
/// asked at `zooms`. What the node inherits is found as `world_answer` finds it (ask.wgsl). The
/// slot is claimed first (`claim_slot`), so every other asker of the node at those zooms finds
/// it claimed and asks nothing. Then, where one more ask stays within the frame's (`may_ask`),
/// the world is asked and the answer written to the slot (`fill`). Nothing is asked where a
/// slot keeps the node at those zooms, where the node cache does not keep such a node
/// (`keeps`), or where no slot can be claimed. Past the frame's asks the claim is let go, and
/// the slot's key is put back as it was. The rays parked at the node are then walked on by the
/// walk that asks (queue_last.wgsl). The ask is counted in the run's lane (`asked_at`), unless
/// that lane is `NONE`.
fn ask_once(parent: vec3<u32>, top: bool, zoom: u32, c: vec3<i32>, zooms: u32) {
  if (!keeps(zoom, c)) {
    return;
  }
  let first = bucket_of(zoom, c);
  if (holds_node(first, zoom, c, zooms)) {
    return;
  }
  let tag = walk_tag();
  let claimed = claim_slot(first, zoom, c, zooms, tag);
  if (claimed.x == NONE) {
    return;
  }
  if (!may_ask()) {
    atomicStore(&node_cache[key_at(claimed.x)], claimed.y);
    return;
  }
  // The node is kept at the zooms its answer is kept at (ask.wgsl `key_zooms`). Where it is a
  // cell that another slot already keeps by its node alone, the claim is let go.
  let a = world_answer(parent, top, zoom, c, zooms, asked_at());
  let kz = key_zooms(zoom, zooms, a);
  if (kz != zooms && holds_cell(first, zoom, c, kz != CELL_FAR)) {
    atomicStore(&node_cache[key_at(claimed.x)], claimed.y);
    return;
  }
  fill(claimed.x, zoom, c, kz, tag, a);
}

/// Asks once for each node that the first walk listed, or the `parked`.
fn queue_ask(parked: bool, id: u32) {
  let n = in_cap(atomicLoad(&queue[select(QUEUE_ASKS, QUEUE_ASKS_PARKED, parked)]), QUEUE_RECORDS_CAP);
  for (var k = id; k < n; k += QUEUE_GROUPS * QUEUE_LANES) {
    let at = queue_records_at() + k * QUEUE_RECORD_WORDS;
    let c = vec3<i32>(
      i32(atomicLoad(&queue[at + 1u])),
      i32(atomicLoad(&queue[at + 2u])),
      i32(atomicLoad(&queue[at + 3u])),
    );
    let parent = vec3<u32>(atomicLoad(&queue[at + 6u]), atomicLoad(&queue[at + 7u]), atomicLoad(&queue[at + 8u]));
    ask_once(parent, atomicLoad(&queue[at + 5u]) != 0u, atomicLoad(&queue[at]), c, atomicLoad(&queue[at + 4u]));
  }
}

@compute @workgroup_size(QUEUE_LANES)
fn queue_ask_cs(@builtin(global_invocation_id) id: vec3<u32>) {
  queue_ask(false, id.x);
}

/// Asks for the second walk's nodes. It also sets the counter and the count of the last walk's
/// rays, as that walk runs next and binds no queue (queue_last.wgsl).
@compute @workgroup_size(QUEUE_LANES)
fn queue_ask_parked_cs(@builtin(global_invocation_id) id: vec3<u32>) {
  if (id.x == 0u) {
    atomicStore(&node_cache[LAST_NEXT], 0u);
    atomicStore(&node_cache[LAST_CNT], atomicLoad(&queue[QUEUE_PARKED]));
  }
  queue_ask(true, id.x);
}
