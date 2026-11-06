// pxl-render/queue_ask.wgsl

// The nodes a walk of the holes' queue was parked at, asked of the world once a node
// (queue_walk.wgsl, ask.wgsl `ask_once`): a lane a node listed, the lanes a fixed dispatch
// (pxl-engine `ray::QUEUE_GROUPS`) that takes the list in strides. What it keeps is there when
// the next walk goes on from the parked states, past this dispatch's end.

/// The queue's words (pxl-engine `ray::QUEUE_WORDS`).
@group(0) @binding(10) var<storage, read_write> queue: array<atomic<u32>>;

const WALK_PASS: u32 = MISSED_PASS;

/// Whether a node may be asked of the world: the eye's always; the light's one more within
/// the frame's budget of asks (`light::ASKS_A_FRAME`), counted as it is taken.
fn may_ask() -> bool {
  return !light_run() || atomicAdd(&queue[QUEUE_ASKED], 1u) < ASKS_A_FRAME;
}

/// The nodes the walk `again` or the first listed, each asked once.
fn queue_ask(again: bool, id: u32) {
  let n = in_room(atomicLoad(&queue[select(QUEUE_ASKS, QUEUE_ASKS_AGAIN, again)]), QUEUE_RECORDS_ROOM);
  let asked = select(EYE_ASKED_COUNT, LIGHT_ASKED_COUNT, light_run());
  for (var k = id; k < n; k += QUEUE_GROUPS * QUEUE_LANES) {
    let at = QUEUE_RECORDS_AT + (select(0u, QUEUE_RECORDS_ROOM, again) + k) * QUEUE_RECORD_WORDS;
    let c = vec3<i32>(
      i32(atomicLoad(&queue[at + 1u])),
      i32(atomicLoad(&queue[at + 2u])),
      i32(atomicLoad(&queue[at + 3u])),
    );
    let parent = vec4<u32>(
      atomicLoad(&queue[at + 6u]),
      atomicLoad(&queue[at + 7u]),
      atomicLoad(&queue[at + 8u]),
      atomicLoad(&queue[at + 9u]),
    );
    ask_once(parent, atomicLoad(&queue[at + 5u]) != 0u, atomicLoad(&queue[at]), c, atomicLoad(&queue[at + 4u]), asked);
  }
}

@compute @workgroup_size(QUEUE_LANES)
fn queue_ask_cs(@builtin(global_invocation_id) id: vec3<u32>) {
  queue_ask(false, id.x);
}

/// The second walk's nodes; and the last walk's first ray to take and the slots it carries
/// set back, as that walk runs next (queue_last.wgsl).
@compute @workgroup_size(QUEUE_LANES)
fn queue_ask_parked_cs(@builtin(global_invocation_id) id: vec3<u32>) {
  if (id.x == 0u) {
    atomicStore(&node_cache[LAST_TAKEN], 0u);
    atomicStore(&node_cache[CARRIED], 0u);
  }
  queue_ask(true, id.x);
}
