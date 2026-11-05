// pxl-render/way_return.wgsl

// The child groups each handed brick's build gave back (`way_tree_cs`), on the stack
// again: after the build, so no push lands under another brick's ids while it reads them.
// A workgroup a brick, a lane a child group.

@group(0) @binding(0) var<storage, read> scratch: array<u32>;
@group(0) @binding(1) var<storage, read_write> stack: Stack;

@compute @workgroup_size(WAY_LANES)
fn way_return_cs(@builtin(workgroup_id) group: vec3<u32>, @builtin(local_invocation_id) lane: vec3<u32>) {
  let freed = scratch_at(arrayLength(&scratch), group.z) + FREED_AT;
  // A build gives back no more than it took, `BRICK_CHILD_GROUPS` at most (`tree::fits`).
  let n = min(scratch[freed], BRICK_CHILD_GROUPS);
  for (var r = lane.x; r < n; r += WAY_LANES) {
    push(scratch[freed + 1u + r]);
  }
}
