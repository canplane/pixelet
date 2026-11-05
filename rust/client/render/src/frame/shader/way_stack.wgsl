// pxl-render/way_stack.wgsl

// The ways' pool's free stack (pxl-engine `way::tree::TAKEN_CNT`), the free, the build and
// the return passes': a slot's word its id XOR its index, read and written here alone. Every
// pass that binds it names it `stack`. Its count, the first word, is atomic: a pop is an add,
// a push an add of -1, no compare-and-swap, which Safari's WGSL compiler writes Metal for that
// does not compile (doc/experiment/20261103-shaders-on-two-phones). Its slots are plain: a
// pass that pushes writes each slot by the one push whose add took it, and reads none; the
// build, which pops, reads the slots the passes before it wrote, and writes none.

struct Stack {
  taken: atomic<u32>,
  slots: array<u32>,
}

/// The words the stack holds, its count's and its slots'.
fn stack_len() -> u32 {
  return 1u + arrayLength(&stack.slots);
}

/// The id stack slot `i` holds, `i` its word in the stack.
fn stack_id(i: u32) -> u32 {
  return stack.slots[i - 1u] ^ i;
}

/// Child group `child_group` back on the stack, into the slot of the last taken: the count
/// one fewer, an add of -1 as its word is unsigned.
fn push(child_group: u32) {
  let was = atomicAdd(&stack.taken, 0xffffffffu);
  stack.slots[was - 1u] = child_group ^ was;
}
