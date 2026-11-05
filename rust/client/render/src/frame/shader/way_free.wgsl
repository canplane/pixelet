// pxl-render/way_free.wgsl

// Each re-filled brick's old ways' tree given back to the pool (pxl-engine `way::tree`), a
// workgroup of one a brick, ahead of its build (`way_tree_cs`): walked from its root word,
// depth first, every child group its own and its branches' pushed. No stack: the path down
// is a word, four bits a level the next of its eight to look under, and a level's child
// group found again from the root along it.

@group(0) @binding(0) var<storage, read> fresh: array<u32>;
@group(0) @binding(1) var<storage, read> ground: array<u32>;
@group(0) @binding(2) var<storage, read> pool: array<u32>;
@group(0) @binding(3) var<storage, read_write> stack: Stack;

/// Digit `d` of `path`: the next of its level's eight to look under.
fn path_at(path: u32, d: u32) -> u32 {
  return (path >> (4u * d)) & 15u;
}

@compute @workgroup_size(1)
fn way_free_cs(@builtin(workgroup_id) group: vec3<u32>) {
  let root = ground[fresh[handed_at(group.z) + FILL_BRICK] * BRICK_WORDS + ROOT_AT];
  if (root == 0u) {
    return;
  }
  // The root's child group, the brick's own nodes along z (`way::tree::root_slot`).
  let first = CHILDREN * (root & (POOL_CHILD_GROUPS - 1u));
  push(first / CHILDREN);
  var pushed = 1u;
  var path = 0u;
  var d = 0u;
  var b = first;
  // A tree way_tree_cs built holds branches down to the brick's cells alone, `BRICK_LEVELS`
  // under its root child group, and no more than `BRICK_CHILD_GROUPS` child groups. A word
  // past either is no such tree's: the walk stops there and leaves the child groups it has
  // not reached taken.
  loop {
    let k = path_at(path, d);
    if (k == CHILDREN) {
      if (d == 0u) {
        break;
      }
      d -= 1u;
      b = first;
      for (var i = 0u; i < d; i++) {
        b = pool[b + path_at(path, i) - 1u] & ~BRANCH;
      }
      continue;
    }
    path += 1u << (4u * d);
    let h = pool[b + k];
    if (h != 0u) {
      if (d == BRICK_LEVELS || pushed == BRICK_CHILD_GROUPS) {
        break;
      }
      b = h & ~BRANCH;
      push(b / CHILDREN);
      pushed += 1u;
      d += 1u;
      path &= ~(15u << (4u * d));
    }
  }
}
