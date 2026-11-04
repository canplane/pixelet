// pxl-render/way_tree.wgsl

// Each handed brick's ways as a tree of the placed tree's shape (pxl-engine `way::tree`), a
// workgroup a brick, `group.z` its place in `fresh`: from the brick's own nodes along z its
// ways' bound meets (`tree::span`) down a level at a time, each candidate flagged where the
// brick's levels say a way may lie under it and the flagged given their children's child
// groups off the pool's stack together, in the order the workgroup's scan ranks them; at the
// brick's cells each a leaf off the spans kept (`way::way_cell`); then up, each branch
// folded, one of eight equal leaves written as that leaf and its child group given back
// (`way_return_cs` pushes them), every child group it holds kept as a record for the
// read-back. Its root word last; none where it has no ways, nothing taken nor kept, or where
// the pool or its scratch ran out, the child groups it took given back, and its records'
// count saying which (`TREE_STARVED`, `TREE_CRAMPED`). A level's numbers are what every
// lane reads alike (`workgroupUniformLoad`), the loops' bounds, so every barrier is in
// uniform control flow.

@group(0) @binding(0) var<storage, read> fresh: array<u32>;
@group(0) @binding(1) var<storage, read_write> ground: array<u32>;
@group(0) @binding(2) var<storage, read_write> pool: array<u32>;
@group(0) @binding(3) var<storage, read_write> stack: array<atomic<u32>>;
@group(0) @binding(4) var<storage, read_write> scratch: array<atomic<u32>>;

/// A scan's sums, a lane's own.
var<workgroup> sums: array<u32, WAY_LANES>;
/// `levels[2j]`, `levels[2j + 1]`: level j's first id on the stack and its candidates,
/// level 0 the brick's own nodes along z; the last four words (`Z0_AT`, `TAKEN_AT`,
/// `CELL_AT`, `STATE_AT`), the first one's Z, the child groups the brick has taken, its
/// cells' zoom and whether every level so far fit the pool (`FITS`), or the brick has no
/// ways (`NO_WAYS`), or the level the brick took past its own most (`PAST`), or past what
/// its scratch holds (`CRAMPED`); 0 where the pool ran out.
var<workgroup> levels: array<u32, WAY_LANES>;

const Z0_AT = WAY_LANES - 4u;
const TAKEN_AT = WAY_LANES - 3u;
const CELL_AT = WAY_LANES - 2u;
const STATE_AT = WAY_LANES - 1u;
const FITS = 1u;
const PAST = 2u;
const CRAMPED = 3u;
const NO_WAYS = 4u;

// ---- the workgroup's words ----

/// Lane `t`'s inclusive sum of the workgroup's `own`s, a step a doubling (Hillis and Steele
/// 1986; GPU Gems 3, ch. 39): each sum adds the one `d` before it, both read before any is
/// written.
fn scan(t: u32, own: u32) -> u32 {
  sums[t] = own;
  for (var d = 1u; d < WAY_LANES; d *= 2u) {
    workgroupBarrier();
    let before = sums[(t - d) & (WAY_LANES - 1u)];
    let sum = sums[t];
    workgroupBarrier();
    sums[t] = sum + select(0u, before, t >= d);
  }
  return sums[t];
}

/// The workgroup's flagged so far, its scan's last sum, as every lane reads it.
fn scanned() -> u32 {
  return workgroupUniformLoad(&sums[WAY_LANES - 1u]);
}

/// Level word `i` as every lane reads it.
fn level_word(i: u32) -> u32 {
  return workgroupUniformLoad(&levels[i]);
}

/// Every lane's writes to storage seen by every lane of the workgroup after it: a level's
/// slots, written by the lanes of the one before, read by others.
fn settle() {
  storageBarrier();
}

// ---- the pool, its stack and the scratch ----

fn first_child(slot: u32) -> u32 {
  return pool[slot];
}

fn data(slot: u32) -> u32 {
  return pool[arrayLength(&pool) / 2u + slot];
}

fn set_first_child(slot: u32, v: u32) {
  pool[slot] = v;
}

fn set_data(slot: u32, v: u32) {
  pool[arrayLength(&pool) / 2u + slot] = v;
}

/// A level's candidate `c`: slot `c % 8` of the child group whose id is the level's
/// `c / 8`-th off the stack, from where its ids begin.
fn slot_of(ids_at: u32, c: u32) -> u32 {
  return CHILDREN * stack_id(ids_at + c / CHILDREN) + c % CHILDREN;
}

/// `n` child groups off the stack together for a brick that has taken `taken`: where their
/// ids begin in it, 0 where fewer are free or the brick would take more than its most
/// (`tree::fits`), none taken. Taken by an add; where they were not all free, given back by
/// lowering the count to where this pop found it. Only pops race in a pass (pushes are
/// `way_free_cs`'s and `way_return_cs`'s). A pop that adds while a failed pop's add stands
/// finds the count past the stack and fails too, and its own lowering stops above the
/// failed one's; a pop that adds after the failed one lowered the count finds it where that
/// one found it, and may fit in the slots it gave back. So the count comes to rest at the
/// end of what the pops that fit took. Both atomics asked of every pop, a no-op where it has
/// nothing to take or give back.
fn pop(taken: u32, n: u32) -> u32 {
  let len = arrayLength(&stack);
  let past = is_past(taken, n);
  let gone = atomicAdd(&stack[TAKEN_CNT], select(n, 0u, past));
  // Past the stack while another pop's add stands: none free.
  let ok = !past && gone < len && n <= len - 1u - gone;
  atomicMin(&stack[TAKEN_CNT], select(gone, 0xffffffffu, ok || past));
  return select(0u, 1u + gone, ok);
}

/// Whether `flagged` child groups take a brick that has taken `taken` past its most
/// (`tree::past`).
fn is_past(taken: u32, flagged: u32) -> bool {
  return flagged > BRICK_CHILD_GROUPS - taken;
}

fn put(i: u32, v: u32) {
  atomicStore(&scratch[i], v);
}

fn word(i: u32) -> u32 {
  return atomicLoad(&scratch[i]);
}

/// Child group `first / 8`'s record, its id and its sixteen words, the next of the records
/// at `records` (`tree::records_at`), as `tree::lay` reads them.
fn record(records: u32, first: u32) {
  let r = atomicAdd(&scratch[records], 1u);
  let w = records + 1u + r * RECORD_WORDS;
  put(w, first / CHILDREN);
  for (var k = 0u; k < CHILDREN; k++) {
    put(w + 1u + k, first_child(first + k));
    put(w + 1u + CHILDREN + k, data(first + k));
  }
}

/// Child group `child_group` the build gave back, into the scratch from `mine`.
fn give_back(mine: u32, child_group: u32) {
  let r = atomicAdd(&scratch[mine + FREED_AT], 1u);
  put(mine + FREED_AT + 1u + r, child_group);
}

// ---- way::tree ----

/// A candidate's word: its X and Y in the brick at its zoom, and its Z (`pack`).
fn pack(at: vec3<i32>) -> u32 {
  return (u32(at.z) << Z_SHIFT) | (u32(at.y) << BRICK_SHIFT) | u32(at.x);
}

fn unpack(w: u32) -> vec3<i32> {
  return vec3<i32>(i32(w & REL_MASK), i32((w >> BRICK_SHIFT) & REL_MASK), i32(w) >> Z_SHIFT);
}

/// The first level: the brick's own nodes along z from `z0`, in the child group at `root`
/// (`tree::root`).
fn tree_root(z0: i32, root: u32) {
  for (var k = 0u; k < ROOT_NODES; k++) {
    set_first_child(root + k, 0u);
    set_data(root + k, pack(vec3<i32>(0i, 0i, z0 + i32(k))));
  }
}

/// Whether the candidate at `slot`, a node of `zoom`, takes a child group for its children:
/// coarser than the brick's cells, its Z meeting the ways' bound of the brick's levels at its
/// size (`tree::flag`).
fn flag(base: u32, cell: u32, zoom: u32, slot: u32) -> bool {
  let at = unpack(data(slot));
  if (zoom >= cell) {
    return false;
  }
  let e = base + LEVELS_AT + LEVEL_ENTRY_WORDS * (level_at(cell - zoom) + cell_id(vec2<u32>(at.xy)));
  let ways = bound_of(i32(ground[e + 1u]));
  let d = LEAF_CELL - zoom;
  return ways.x < ((at.z + 1i) << d) && ways.y > (at.z << d);
}

/// The flagged candidate at `slot`, a branch whose child group begins at `first`: its
/// eight, the next level's candidates (`tree::branch`).
fn branch(slot: u32, first: u32) {
  let at = unpack(data(slot));
  set_first_child(slot, BRANCH | first);
  for (var k = 0u; k < CHILDREN; k++) {
    let child = 2i * at + vec3<i32>(i32(k & 1u), i32((k >> 1u) & 1u), i32(k >> 2u));
    set_first_child(first + k, 0u);
    set_data(first + k, pack(child));
  }
}

/// A candidate not flagged: nothing under it (`tree::empty`).
fn empty(slot: u32) {
  set_first_child(slot, 0u);
  set_data(slot, VALUE_VOID);
}

/// The candidate at `slot`, a cell of the brick: a leaf of what the ways write there, a
/// bore's air placed air, nothing where they write none (`tree::leaf`).
fn leaf(mine: u32, cell: u32, slot: u32) {
  let at = unpack(data(slot));
  let val = way_cell(mine, at.xy, cell, at.z);
  var w = val & 0xffffu;
  if (val == WAY_NOTHING) {
    w = VALUE_VOID;
  } else if (val == WAY_AIR) {
    w = VALUE_AIR;
  }
  set_first_child(slot, 0u);
  set_data(slot, w);
}

/// A leaf's word in its parent's fold: a voxel's fills it whole, any other none
/// (`overwrite::share`).
fn share(w: u32) -> u32 {
  let id = w & 0xffffu;
  return select(0u, id | (FULL << 16u), w != 0u && voxel(id));
}

/// Child `k`'s fold word in a parent's fold: a leaf's share, a branch's fold.
fn share_at(first: u32, k: u32) -> u32 {
  let d = data(first + k);
  return select(d, share(d), first_child(first + k) == 0u);
}

/// Child `k` of the eight fold words `lo` and `hi`.
fn child_of(lo: vec4<u32>, hi: vec4<u32>, k: u32) -> u32 {
  return select(lo[k & 3u], hi[k & 3u], k >= 4u);
}

/// A cube's eight children folded by volume (`overwrite::fold`): the share of the cube
/// filled, the mean of theirs, and the kind most of it is, a tie to the corner the zoom
/// alternates.
fn fold_of(lo: vec4<u32>, hi: vec4<u32>, zoom: u32) -> u32 {
  var sum = 0u;
  for (var k = 0u; k < 8u; k++) {
    sum += child_of(lo, hi, k) >> 16u;
  }
  var best = 0u;
  var most = 0u;
  for (var j = 0u; j < 8u; j++) {
    let c = child_of(lo, hi, select(j, 7u - j, (zoom & 1u) == 0u));
    var w = 0u;
    for (var i = 0u; i < 8u; i++) {
      let o = child_of(lo, hi, i);
      if ((o & 0xffffu) == (c & 0xffffu)) {
        w += o >> 16u;
      }
    }
    if ((c >> 16u) > 0u && w > most) {
      best = c & 0xffffu;
      most = w;
    }
  }
  return (best & 0xffffu) | ((sum / 8u) << 16u);
}

/// The branch at `slot`, a node of `zoom`, its children written: where its eight are leaves
/// of one word, nothing included, that leaf, and the first slot of the child group it gives
/// back; else its eight folded, and `TREE_NONE` (`tree::fold`).
fn fold(zoom: u32, slot: u32) -> u32 {
  let first = first_child(slot) & ~BRANCH;
  let w = data(first);
  var one = true;
  for (var k = 0u; k < CHILDREN; k++) {
    one = one && first_child(first + k) == 0u && data(first + k) == w;
  }
  if (one) {
    set_first_child(slot, 0u);
    set_data(slot, w);
    return first;
  }
  let lo = vec4<u32>(share_at(first, 0u), share_at(first, 1u), share_at(first, 2u), share_at(first, 3u));
  let hi = vec4<u32>(share_at(first, 4u), share_at(first, 5u), share_at(first, 6u), share_at(first, 7u));
  set_data(slot, fold_of(lo, hi, zoom));
  return TREE_NONE;
}

/// The first slot of the child group under the branch at `slot`, `TREE_NONE` under a leaf
/// (`tree::under`).
fn under(slot: u32) -> u32 {
  let h = first_child(slot);
  return select(h & ~BRANCH, TREE_NONE, h == 0u);
}

// ---- way.rs: what the ways write in a cell, off the spans kept ----

/// A head word's face cell (fill.rs `Scratch::head`).
fn head_face(w: u32) -> i32 {
  let face = i32(w) >> 16u;
  return select(face, FACE_NONE, face == -32768i);
}

/// Where column `c`'s spans begin in the scratch from `mine`, `c` in the brick's cells,
/// its ring one past each side (fill.rs `Scratch::column`).
fn spans_at(mine: u32, c: vec2<i32>) -> u32 {
  let i = u32(c.y + 1i) * SPANS_ACROSS + u32(c.x + 1i);
  return mine + SPANS_AT + i * SPANS_COLUMN_WORDS;
}

/// What the ways write in cell `z` at zoom `cell` of column `c`, its `n` spans kept in the
/// scratch from `mine`: a deck's slab or its pier, the highest of those over it keeping what
/// two share; else the first air over it (`AIR_OF` over its way's palette index); else
/// `WAY_NOTHING` (`way::at`).
fn way_at(mine: u32, c: vec2<i32>, n: u32, cell: u32, z: i32) -> u32 {
  let at = spans_at(mine, c);
  var best = vec2<i32>(FACE_NONE);
  var palette_idx = WAY_NOTHING;
  var air = WAY_NOTHING;
  for (var j = 0u; j < n; j++) {
    let run = bound_of(i32(word(at + 1u + 2u * j)));
    let v = word(at + 2u + 2u * j);
    if (fills(cell, run, z)) {
      if ((v & AIR_OF) != 0u) {
        if (air == WAY_NOTHING) {
          air = v;
        }
      } else if (run.x > best.x || (run.x == best.x && run.y >= best.y)) {
        best = run;
        palette_idx = v;
      }
    }
  }
  return select(air, palette_idx, palette_idx != WAY_NOTHING);
}

/// The `k`-th of a cell and the six about it, from its own: the row under it, the column
/// before it, the cell under it, the cell over it, the column after it, the row over it
/// (`way::near`).
fn near(k: u32) -> vec3<i32> {
  var i = 6u - k;
  var s = 1i;
  if (k < 4u) {
    i = k - 1u;
    s = -1i;
  }
  let on = (vec3<u32>(1u, 0u, 2u) == vec3<u32>(i)) & vec3<bool>(k != 0u);
  return select(vec3<i32>(0i), vec3<i32>(s), on);
}

/// What the ways write in cell `z` of column `c` at zoom `cell` (`way::way_cell`): its own
/// spans' slab or pier, else a bore's air (`WAY_AIR`); else, under its face cell, the way's
/// palette index where a bore's air is beside it, the first in `near`'s order; else
/// `WAY_NOTHING`.
fn way_cell(mine: u32, c: vec2<i32>, cell: u32, z: i32) -> u32 {
  let head = word(spans_at(mine, c));
  let v = way_at(mine, c, head & SPANS_CNT, cell, z);
  if (v != WAY_NOTHING) {
    return select(v, WAY_AIR, (v & AIR_OF) != 0u);
  }
  let face = head_face(head);
  for (var m = 1u; z < face && m < 7u; m++) {
    let d = near(m);
    let b = c + d.xy;
    let u = way_at(mine, b, word(spans_at(mine, b)) & SPANS_CNT, cell, z + d.z);
    if (u != WAY_NOTHING && (u & AIR_OF) != 0u) {
      return u & ~AIR_OF;
    }
  }
  return WAY_NOTHING;
}

// ---- the build ----

@compute @workgroup_size(WAY_LANES)
fn way_tree_cs(@builtin(workgroup_id) group: vec3<u32>, @builtin(local_invocation_id) lane: vec3<u32>) {
  let k = group.z;
  let t = lane.x;
  let rec = handed_at(k);
  let base = fresh[rec + FILL_BRICK] * BRICK_WORDS;
  let mine = scratch_at(arrayLength(&scratch), k);
  // The child groups a brick's scratch holds, every brick's alike (`tree::scratch_child_groups`),
  // and where its records begin (`tree::records_at`).
  let room = scratch_child_groups(arrayLength(&scratch) / FILL_CAP);
  let records = mine + records_at(room);
  if (t == 0u) {
    put(mine + FREED_AT, 0u);
    put(records, 0u);
    let cell = fresh[rec + FILL_CELL];
    // The brick's own nodes along z its ways meet (fill.rs `root_span`, `tree::span`).
    let top = base + LEVELS_AT + LEVEL_ENTRY_WORDS * level_at(BRICK_LEVELS);
    let ways = bound_of(i32(ground[top + 1u]));
    let d = LEAF_CELL - (cell - BRICK_SHIFT);
    var z = 0i;
    var nodes = 0u;
    if (ways.x < ways.y) {
      z = ways.x >> d;
      nodes = u32(((ways.y - 1i) >> d) - z + 1i);
    }
    var ids_at = 0u;
    var state = NO_WAYS;
    if (nodes > ROOT_NODES) {
      state = PAST;
    } else if (nodes != 0u) {
      ids_at = pop(0u, 1u);
      state = select(0u, FITS, ids_at != 0u);
      if (state == FITS) {
        tree_root(z, CHILDREN * stack_id(ids_at));
      }
    }
    let ok = state == FITS;
    levels[TAKEN_AT] = u32(ok);
    levels[0] = ids_at;
    levels[1] = select(0u, ROOT_NODES, ok);
    levels[Z0_AT] = u32(z);
    levels[CELL_AT] = cell;
    levels[STATE_AT] = state;
  }
  let cell = level_word(CELL_AT);
  let first = level_word(0u);
  var j = 0u;
  var zoom = cell - BRICK_SHIFT;
  var ok = level_word(STATE_AT) == FITS;
  // Down: each level's candidates flagged, counted, then given their child groups together.
  while (ok && zoom < cell) {
    settle();
    let ids_at = level_word(2u * j);
    let n = level_word(2u * j + 1u);
    var flagged = 0u;
    for (var c = 0u; c < n; c += WAY_LANES) {
      let i = c + t;
      var f = false;
      if (i < n) {
        f = flag(base, cell, zoom, slot_of(ids_at, i));
      }
      scan(t, u32(f));
      flagged += scanned();
    }
    if (t == 0u) {
      let had = levels[TAKEN_AT];
      // Past the brick's most, or past its scratch, none is taken: the pool is asked only
      // of what the scratch holds.
      var refused = 0u;
      if (is_past(had, flagged)) {
        refused = PAST;
      } else if (had + flagged > room) {
        refused = CRAMPED;
      }
      var next = 0u;
      if (refused == 0u) {
        next = pop(had, flagged);
      }
      levels[TAKEN_AT] = had + flagged;
      levels[2u * j + 2u] = next;
      levels[2u * j + 3u] = CHILDREN * flagged;
      levels[STATE_AT] = select(refused, FITS, next != 0u);
    }
    ok = level_word(STATE_AT) == FITS;
    let next = level_word(2u * j + 2u);
    var before = 0u;
    for (var c = 0u; ok && c < n; c += WAY_LANES) {
      let i = c + t;
      var f = false;
      var s = 0u;
      if (i < n) {
        s = slot_of(ids_at, i);
        f = flag(base, cell, zoom, s);
      }
      let rank = before + scan(t, u32(f)) - u32(f);
      if (f) {
        branch(s, CHILDREN * stack_id(next + rank));
      } else if (i < n) {
        empty(s);
      }
      before += scanned();
    }
    j += 1u;
    zoom += 1u;
  }
  // The pool or the scratch ran out: the child groups each level before took, given back.
  if (!ok && t == 0u) {
    for (var l = 0u; l < j; l++) {
      let ids_at = levels[2u * l];
      let n = levels[2u * l + 1u];
      for (var b = 0u; b < n / CHILDREN; b++) {
        give_back(mine, stack_id(ids_at + b));
      }
    }
  }
  settle();
  let ids_at = level_word(2u * j);
  let n = level_word(2u * j + 1u);
  // The cells: a leaf each.
  for (var c = 0u; ok && c < n; c += WAY_LANES) {
    if (c + t < n) {
      leaf(mine, cell, slot_of(ids_at, c + t));
    }
  }
  // Up: each level's branches folded over the level under them.
  while (ok && j > 0u) {
    settle();
    j -= 1u;
    zoom -= 1u;
    let ids_at = level_word(2u * j);
    let n = level_word(2u * j + 1u);
    for (var c = 0u; c < n; c += WAY_LANES) {
      if (c + t < n) {
        let s = slot_of(ids_at, c + t);
        let below = under(s);
        if (below != TREE_NONE) {
          let gave = fold(zoom, s);
          if (gave != TREE_NONE) {
            give_back(mine, gave / CHILDREN);
          } else {
            record(records, below);
          }
        }
      }
    }
  }
  settle();
  if (t == 0u) {
    let child_group = stack_id(first);
    let state = levels[STATE_AT];
    if (ok) {
      record(records, CHILDREN * child_group);
    } else if (state == 0u) {
      // Past what the pool holds: its count `TREE_STARVED`, the pool's to grow (pxl-render
      // `Short`).
      put(records, TREE_STARVED);
    } else if (state == PAST) {
      // Past the brick's own most, which no pool's growth lets it build: its count
      // `TREE_NONE`, refused at once (pxl-render `Resident`).
      put(records, TREE_NONE);
    } else if (state == CRAMPED) {
      // Past what its scratch holds: its count `TREE_CRAMPED`, the scratch's to grow.
      put(records, TREE_CRAMPED);
    }
    // Its root word (`tree::root_word`).
    ground[base + ROOT_AT] = select(0u, child_group | (levels[Z0_AT] << ROOT_ID_BITS), ok);
  }
}

/// The child groups a brick's scratch of `words` holds a tree of, no more than
/// `BRICK_CHILD_GROUPS` (`tree::scratch_child_groups`).
fn scratch_child_groups(words: u32) -> u32 {
  if (words < FREED_AT + 2u) {
    return 0u;
  }
  return min((words - FREED_AT - 2u) / (1u + RECORD_WORDS), BRICK_CHILD_GROUPS);
}

/// Where a brick's records begin in its scratch for a tree of `child_groups`
/// (`tree::records_at`).
fn records_at(child_groups: u32) -> u32 {
  return FREED_AT + 1u + child_groups;
}
