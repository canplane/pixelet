// pxl-render/way_stack.wgsl

// The ways' pool's free stack (pxl-engine `way::tree::TAKEN_CNT`), the free, the build and
// the return passes': a slot's word its id XOR its index, read and written here alone. Every
// pass that binds it names it `stack`, all atomic: a pop is an add, a push an add of -1,
// no compare-and-swap, which Safari's WGSL compiler writes Metal for that does not compile
// (doc/experiment/20261103-shaders-on-two-phones).

/// The id stack slot `i` holds.
fn stack_id(i: u32) -> u32 {
  return atomicLoad(&stack[i]) ^ i;
}

/// Child group `child_group` back on the stack, into the slot of the last taken: the count
/// one fewer, an add of -1 as its word is unsigned.
fn push(child_group: u32) {
  let was = atomicAdd(&stack[TAKEN_CNT], 0xffffffffu);
  atomicStore(&stack[was], child_group ^ was);
}
