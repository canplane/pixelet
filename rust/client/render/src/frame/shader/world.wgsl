// pxl-render/world.wgsl

// The world's one query by hand (pxl-engine world.rs `node`), over what the Rust side
// wrote, read only: the clip levels, their bricks, the service area's index, what is
// placed and the ways' pool. Answers word for word as the Rust does (tests/world.rs).
// Scoped locals and small structs: no array, no atomic, nothing held across a loop
// past what it reads after. All integers: a height a word holds is exact through
// `ground::top`'s floats, so `top_of` is its integer form.
//
// What a node hands down is `vec4<u32>`, `Inheritance::words`: what is placed, its
// shard, where its ground's cells were held, its ways' cursor. A cut is `vec4<i32>` as
// the eye holds one, its index then its zoom (`overwrite::Cut`, `NO_CUT` for none).

@group(0) @binding(2) var<storage, read> ground: array<u32>;
@group(0) @binding(5) var<storage, read> shards: array<vec4<u32>>;
@group(0) @binding(6) var<storage, read> overwrites: array<u32>;
@group(0) @binding(7) var<storage, read> pool: array<u32>;

// ---- value.rs, answer.rs ----

fn fills(asset: u32) -> bool {
  let v = asset & VAL_MASK;
  return v != VALUE_VOID && v != VALUE_AIR;
}

struct Cover {
  fill: u32,
  theirs: bool,
}

/// `answer::cover`.
fn cover(under_fill: u32, kind: u32, over: u32) -> Cover {
  if (fills(kind)) {
    return Cover(min(under_fill + over, FULL), over >= under_fill);
  }
  let taken = (over / (FULL / EIGHTHS)) * (FULL / EIGHTHS);
  return Cover(select(under_fill - taken, 0u, taken > under_fill), false);
}

/// `v` four times, a cell's word's four palette indices (`ground::word`).
fn four_of(v: u32) -> u32 {
  return v | (v << 8u) | (v << 16u) | (v << 24u);
}

// ---- overwrite.rs: a tree of two columns, what is placed or the ways ----

const PLACED = 0u;
const WAYS = 1u;

/// Word `slot`'s child pointer in the ways' pool, the child groups whole, a child group's
/// eight child pointers then its eight data words (pxl-engine way/tree.rs `first_child_at`).
fn first_child_at(slot: u32) -> u32 {
  return 2u * slot - slot % 8u;
}

fn first_child(tree: u32, slot: u32) -> u32 {
  if (tree == WAYS) {
    return pool[first_child_at(slot)];
  }
  return overwrites[slot];
}

fn tree_data(tree: u32, slot: u32) -> u32 {
  if (tree == WAYS) {
    return pool[first_child_at(slot) + 8u];
  }
  return overwrites[arrayLength(&overwrites) / 2u + slot];
}

fn is_folded(s: u32) -> bool {
  return s != SLOT_NOTHING && (s & SLOT_FOLDED) != 0u;
}

/// `overwrite::of`.
fn slot_at(tree: u32, slot: u32, zoom: u32) -> u32 {
  if (first_child(tree, slot) != 0u) {
    return (slot << SLOT_SHIFT) | SLOT_FOLDED | zoom;
  }
  if (tree_data(tree, slot) == VALUE_VOID) {
    return SLOT_NOTHING;
  }
  return (slot << SLOT_SHIFT) | zoom;
}

fn y_start(zoom: u32) -> i32 {
  return select(i32(1u << ((zoom - 1u) & 31u)), 0i, zoom == 0u);
}

fn z_start(zoom: u32) -> i32 {
  return select(i32(1u << ((zoom - SHARD_ZOOM - 1u) & 31u)), 0i, zoom <= SHARD_ZOOM);
}

/// `overwrite::path`.
fn path(zoom: u32, at: vec3<i32>) -> vec3<i32> {
  return vec3<i32>(at.x & i32((1u << zoom) - 1u), at.y + y_start(zoom), at.z + z_start(zoom));
}

fn child_idx(at: vec3<i32>) -> u32 {
  return u32((at.x & 1i) | ((at.y & 1i) << 1u) | ((at.z & 1i) << 2u));
}

/// `overwrite::child`.
fn tree_child(tree: u32, parent: u32, zoom: u32, at: vec3<i32>) -> u32 {
  if (!is_folded(parent)) {
    return parent;
  }
  return slot_at(tree, (first_child(tree, parent >> SLOT_SHIFT) & ~BRANCH) + child_idx(at), zoom);
}

/// `overwrite::down`.
fn tree_down(tree: u32, slot: u32, top: u32, zoom: u32, at: vec3<i32>) -> u32 {
  var s = slot_at(tree, slot, top);
  var z = top;
  while (z < zoom && is_folded(s)) {
    z += 1u;
    s = tree_child(tree, s, z, at >> vec3<u32>(zoom - z));
  }
  return s;
}

/// `overwrite::holds`.
fn cut_holds(cut: vec4<i32>, zoom: u32, at: vec3<i32>) -> bool {
  let cz = u32(cut.w);
  if (cz > zoom) {
    return false;
  }
  return all(path(cz, at >> vec3<u32>(zoom - cz)) == path(cz, cut.xyz));
}

/// `overwrite::val`.
fn tree_val(tree: u32, s: u32, hide: vec4<i32>, zoom: u32, at: vec3<i32>) -> u32 {
  if (s == SLOT_NOTHING) {
    return VALUE_VOID;
  }
  let word = tree_data(tree, s >> SLOT_SHIFT);
  let v = word & 0xffffu;
  if (is_folded(s)) {
    if ((word >> 16u) == 0u) {
      return VALUE_VOID;
    }
  } else if (u32(hide.w) == (s & SLOT_ZOOM_MASK) && cut_holds(hide, zoom, at)) {
    return VALUE_VOID;
  }
  return select(VALUE_VOID, v, voxel(v));
}

/// `overwrite::fill_share` of the node holding slot `s` of `tree`, what is placed or the ways.
fn fill_share(tree: u32, s: u32) -> u32 {
  if (is_folded(s)) {
    return tree_data(tree, s >> SLOT_SHIFT) >> 16u;
  }
  return FULL;
}

/// `overwrite::palette_indices`, as the word `world::node` keeps of them: none where
/// one is past a byte.
fn placed_four(s: u32, hide: vec4<i32>, zoom: u32, at: vec3<i32>, v: u32) -> u32 {
  if (!is_folded(s)) {
    return select(0u, four_of(v), v < 256u);
  }
  let base = first_child(PLACED, s >> SLOT_SHIFT) & ~BRANCH;
  var word = 0u;
  for (var q = 0u; q < 4u; q += 1u) {
    let xy = vec2<i32>(2i * at.x + i32(q & 1u), 2i * at.y + i32(q >> 1u));
    // Its upper child where that fills, else its lower through it, else the node's own.
    var c = tree_val(PLACED, slot_at(PLACED, base + q + 4u, zoom + 1u), hide, zoom + 1u, vec3<i32>(xy, 2i * at.z + 1i));
    if (!fills(c)) {
      c = tree_val(PLACED, slot_at(PLACED, base + q, zoom + 1u), hide, zoom + 1u, vec3<i32>(xy, 2i * at.z));
      if (!fills(c)) {
        c = v;
      }
    }
    if (c >= 256u) {
      return 0u;
    }
    word |= c << (8u * q);
  }
  return word;
}

/// `way::tree::cursor`.
fn way_cursor(root: u32, cell: u32, zoom: u32, at: vec3<i32>) -> u32 {
  let top = cell - BRICK_SHIFT;
  if (root == 0u || zoom < top) {
    return SLOT_NOTHING;
  }
  let k = (at.z >> (zoom - top)) - (i32(root) >> ROOT_ID_BITS);
  if (k < 0i || k >= i32(ROOT_NODES)) {
    return SLOT_NOTHING;
  }
  return tree_down(WAYS, CHILDREN * (root & (POOL_CHILD_GROUPS - 1u)) + u32(k), top, zoom, at);
}

// ---- area.rs ----

/// `Shard::at`, the area's end known to be: a GPU's always is.
fn shard_at(shard: u32, zoom: u32, c: vec2<i32>) -> u32 {
  if ((shard & SHARD_KNOWN) != 0u || zoom < SHARD_ZOOM) {
    return shard;
  }
  let own = c >> vec2<u32>(zoom - SHARD_ZOOM);
  var nine = 0u;
  if (own.y >= 0i && own.y < (1i << SHARD_ZOOM)) {
    let key = cell_id(vec2<u32>(own) & vec2<u32>((1u << SHARD_ZOOM) - 1u));
    nine = shards[key / 4u][key % 4u];
  }
  return SHARD_KNOWN | (nine << SHARD_SERVED_SHIFT);
}

fn is_past(shard: u32) -> bool {
  return (shard & SHARD_KNOWN) != 0u && ((shard >> SHARD_SERVED_SHIFT) & SHARD_OWN) == 0u;
}

/// `clipmap::bound_over`: its `all` is left unread.
fn bound_over(place: u32, coarsest_cell: u32, finest: u32, zoom: u32, c: vec2<i32>) -> NodeBound {
  let top = held_to(finest);
  let bottom = held_to(coarsest_cell);
  var b = BOUNDS_EMPTY;
  var found = place;
  var z = top;
  while (z >= FIRST_CELL) {
    let got = bounded(found, z, zoom, c);
    if (z == top) {
      found = got.place;
    }
    if (!got.here) {
      return NodeBound(vec4<i32>(0i), z, false, false, found);
    }
    b = joined(b, got.b);
    if ((b.x <= UNBOUNDED_LO && b.y >= UNBOUNDED_HI) || (got.clip <= bottom && got.all)) {
      return NodeBound(b, z, true, true, found);
    }
    z = got.clip - 1u;
  }
  return NodeBound(BOUNDS_OPEN, FIRST_CELL, true, true, found);
}

/// `ground::Cell`, its fold a word.
struct Cell {
  palette_idx: u32,
  top: i32,
  part: u32,
  fold: u32,
}

/// `clipmap::ground`.
fn ground_column(shard: u32, place: u32, zoom: u32, b: vec2<i32>) -> Cell {
  let clip = held_to(zoom);
  let s = step_to(place, clip, cell_at(zoom, clip, b));
  var fold = 0u;
  var column = 0u;
  var own = VALUE_VOID;
  if (lookup_here(s.place)) {
    let slot = lookup_slot(s.place);
    let i = cell_id(s.at);
    let base = brick_at(slot);
    fold = palette_word(slot * BRICK_CELLS + i);
    column = ground[base + COLUMNS_AT + i];
    own = (ground[base + LAID_AT + i / LAID_CELLS] >> (8u * (i % LAID_CELLS))) & 0xffu;
  }
  let t = top_of(zoom, column_top(column));
  return Cell(select(own, VALUE_UNKNOWN, is_wall(shard, zoom, b)), t.cells, t.part, fold);
}

// ---- ground.rs ----

/// `strata::by_depth`.
fn by_depth(cell: u32, palette_idx: u32, t: i32, z: i32) -> u32 {
  if (z >= t) {
    return VALUE_VOID;
  }
  if (cell < LEAF_CELL || palette_idx == VALUE_UNKNOWN) {
    return palette_idx;
  }
  let s = leaf_cells(cell);
  var filler = STRATA_SOIL;
  var sandstone = 0i;
  if (palette_idx == STRATA_SAND || palette_idx == STRATA_BEACH) {
    filler = STRATA_SAND;
    sandstone = STRATA_SANDSTONE_DEPTH;
  } else if (palette_idx == STRATA_WATER) {
    filler = STRATA_WATER;
  }
  let face = t - s;
  let filled = face - STRATA_FILLER_DEPTH * s;
  if (z >= face) {
    return palette_idx;
  }
  if (z >= filled) {
    return filler;
  }
  if (z >= filled - sandstone * s) {
    return STRATA_SANDSTONE;
  }
  return select(VALUE_UNKNOWN, STRATA_STONE, z >= rock(cell, face_height(cell, t)));
}

/// `ground::rock`: the bedrock's top under a face, `strata::bedrock`.
fn rock(cell: u32, face: i32) -> i32 {
  return top_of(cell, min(face, SEA_LEVEL) - STRATA_BEDROCK_DEPTH).cells;
}

/// `Cell::opens`.
fn opens(col: Cell, cell: u32, z: i32) -> bool {
  let face = face_height(cell, col.top);
  let under = top_of(cell, face).cells - leaf_cells(cell) - i32(col.part < FULL);
  return col.palette_idx != VALUE_UNKNOWN && z < under && z >= rock(cell, face);
}

/// `ground::node` of a cell: its kind and asset, the column read (`Cell`), where it was held.
struct GroundCell {
  kind: u32,
  asset: vec2<u32>,
  palette_idx: u32,
  top: i32,
  part: u32,
  fold: u32,
  place: u32,
}

/// A cell of no column read (`Cell::NONE`).
fn ground_none(kind: u32, asset: vec2<u32>, place: u32) -> GroundCell {
  return GroundCell(kind, asset, VALUE_VOID, 0i, FULL, 0u, place);
}

/// `ground::node`, `is_cell`.
fn ground_cell(shard: u32, place: u32, zoom: u32, c: vec3<i32>) -> GroundCell {
  if (is_past(shard)) {
    let earth = select(VALUE_VOID, VALUE_UNKNOWN, c.z < SEA_LEVEL);
    return ground_none(KIND_CELL, vec2<u32>(earth, 0u), LOOKUP_NONE);
  }
  if (zoom < TOP_ZOOM) {
    return ground_none(KIND_SPLIT, BOX_WHOLE, place);
  }
  let r = bound_over(place, zoom, zoom, zoom, c.xy);
  if (!r.here) {
    return ground_none(KIND_LACKS, vec2<u32>(VALUE_VOID, 0u), r.place);
  }
  let col = ground_column(shard, r.place, zoom, c.xy);
  var asset = VALUE_VOID;
  if (col.palette_idx == VALUE_UNKNOWN || grounds(col.palette_idx)) {
    asset = by_depth(zoom, col.palette_idx, col.top, c.z);
  }
  var cave = 0u;
  if (asset != VALUE_VOID && opens(col, zoom, c.z)) {
    cave = hollow(zoom, c);
  }
  return GroundCell(KIND_CELL, vec2<u32>(asset, cave), col.palette_idx, col.top, col.part, col.fold, r.place);
}

// ---- pxl-content cave.rs ----

/// A tunnel's part in a node, in pipe cells: where it comes in and goes out, the sides
/// it crosses there (6 none, at its ends), and its blocks (`cave::Piece`).
struct Piece {
  enter: vec3<i32>,
  leave: vec3<i32>,
  side_in: u32,
  side_out: u32,
  blocks: u32,
  on: u32,
}

/// `cave::spawn`, a tunnel of class `k`.
fn spawn(zoom: u32, c: vec3<i32>, k: u32) -> Piece {
  let o = c << vec3<u32>(CAVE_PIPE - zoom);
  let s = 1i << (CAVE_PIPE - zoom);
  let h = node_seed(CAVE_TAG_SPAWN, zoom, c);
  let g = node_seed(CAVE_TAG_SPAWN ^ 1u, zoom, c);
  if (s < 4i) {
    let bits = vec3<i32>(i32(h & 1u), i32((h >> 1u) & 1u), i32((h >> 2u) & 1u));
    return Piece(o + bits, o + 1i - bits, 6u, 6u, tunnel_blocks(k), 0u);
  }
  let q = s / 4i;
  let qu = u32(q);
  let ex = i32(((h >> 1u) % qu) + select(3u * qu, 0u, (h & 1u) == 0u));
  let ey = i32(((h >> 9u) % qu) + select(3u * qu, 0u, ((h >> 8u) & 1u) == 0u));
  let ez = i32((h >> 16u) % u32(s));
  let reach = qu / 2u + 1u;
  let mx = s - 1i - ex;
  let my = s - 1i - ey;
  let dx = i32(g % reach);
  let dy = i32((g >> 8u) % reach);
  return Piece(
    o + vec3<i32>(ex, ey, ez),
    o + vec3<i32>(
      select(mx - dx, mx + dx, mx < 2i * q),
      select(my - dy, my + dy, my < 2i * q),
      clamp(ez + i32((g >> 16u) % reach) - q / 4i, 0i, s - 1i),
    ),
    6u, 6u, tunnel_blocks(k), 0u,
  );
}

fn is_aside(d: u32, h: u32) -> bool {
  return d != 0u && countOneBits(d) == 1u && (h & 1u) == 1u;
}

/// `cave::walk_at`: the `i`-th child of the walk from `u` to `v`.
fn tunnel_at(u: u32, v: u32, h: u32, i: u32) -> u32 {
  let d = u ^ v;
  if (is_aside(d, h)) {
    let b = select(((d << 2u) | (d >> 1u)) & 7u, ((d << 1u) | (d >> 2u)) & 7u, ((h >> 1u) & 1u) == 0u);
    switch i {
      case 0u: { return u; }
      case 1u: { return u ^ b; }
      case 2u: { return u ^ b ^ d; }
      default: { return v; }
    }
  }
  let r = (h >> 2u) % 3u;
  var cur = u;
  var flipped = 0u;
  for (var j = 0u; j < 3u && flipped < i; j += 1u) {
    let bit = 1u << ((j + r) % 3u);
    if ((d & bit) != 0u) {
      cur ^= bit;
      flipped += 1u;
    }
  }
  return cur;
}

fn manhattan(a: vec3<i32>, b: vec3<i32>) -> u32 {
  let d = abs(a - b);
  return u32(d.x + d.y + d.z);
}

/// `cave::hand_down`: child `want`'s part of the node's piece, and the children its walk
/// passes.
fn hand_down(zoom: u32, c: vec3<i32>, enter: vec3<i32>, leave: vec3<i32>, side_in: u32, side_out: u32, blocks: u32, want: u32) -> Piece {
  let mid = (c << vec3<u32>(CAVE_PIPE - zoom)) + (1i << (CAVE_PIPE - zoom)) / 2i;
  let u = u32(enter.x >= mid.x) | (u32(enter.y >= mid.y) << 1u) | (u32(enter.z >= mid.z) << 2u);
  let v = u32(leave.x >= mid.x) | (u32(leave.y >= mid.y) << 1u) | (u32(leave.z >= mid.z) << 2u);
  let wh = node_seed(CAVE_TAG_WALK, zoom, c);
  let d = u ^ v;
  let n = select(countOneBits(d) + 1u, 4u, is_aside(d, wh));
  var on = 0u;
  var want_enter = vec3<i32>(0i);
  var want_leave = vec3<i32>(0i);
  var want_in = 6u;
  var want_out = 6u;
  var at = 8u;
  var cells = vec4<u32>(0u);
  var cur = enter;
  var side = side_in;
  for (var i = 0u; i < n; i += 1u) {
    let k = tunnel_at(u, v, wh, i);
    on |= 1u << k;
    let part_enter = cur;
    let part_in = side;
    var part_leave = leave;
    var part_out = side_out;
    if (i + 1u < n) {
      let half = (1i << (CAVE_PIPE - zoom)) / 2i;
      let co = (c << vec3<u32>(CAVE_PIPE - zoom)) + half * vec3<i32>(i32(k & 1u), i32((k >> 1u) & 1u), i32(k >> 2u));
      let next = tunnel_at(u, v, wh, i + 1u);
      let axis = countTrailingZeros(k ^ next);
      let up = ((next >> axis) & 1u) == 1u;
      let h = node_seed(CAVE_TAG_CROSS ^ i, zoom, c);
      let rand = vec3<i32>(vec3<u32>(h, h >> 10u, h >> 20u) & vec3<u32>(0x3ffu));
      let was = (cur + leave) / 2i + (rand * (half / 2i + 1i)) / 1024i - half / 4i;
      part_leave = clamp(was, co, co + half - 1i);
      part_leave[axis] = select(co[axis], co[axis] + half - 1i, up);
      part_out = 2u * axis + u32(up);
      cur = part_leave;
      cur[axis] += select(-1i, 1i, up);
      side = 2u * axis + u32(!up);
    }
    cells[i] = manhattan(part_enter, part_leave) + 1u;
    if (k == want) {
      want_enter = part_enter;
      want_leave = part_leave;
      want_in = part_in;
      want_out = part_out;
      at = i;
    }
  }
  // Its blocks along the walk, each part its share of what is left.
  var want_blocks = 0u;
  var rest = cells.x + cells.y + cells.z + cells.w;
  var left = blocks;
  for (var i = 0u; i < n && i <= at; i += 1u) {
    let cnt = cells[i];
    rest -= cnt;
    let weight = min((cnt << 16u) / max(cnt + rest, 1u), 0xffffu);
    let got = halve(left, cnt * CELL_BLOCKS, rest * CELL_BLOCKS, (weight << 16u) | (rotl(wh, 7u * i + 3u) & 0xffffu));
    if (i == at) {
      want_blocks = got;
    }
    left -= got;
  }
  return Piece(want_enter, want_leave, want_in, want_out, want_blocks, on);
}

/// `cave::tunnels_down` of class `k`: the node's `n` not yet started, child `want`'s.
fn tunnels_down(zoom: u32, c: vec3<i32>, k: u32, tunnels: u32, on: u32, want: u32) -> u32 {
  if (tunnels == 0u) {
    return 0u;
  }
  let class_zoom = CAVE_ROOT + k;
  let child = zoom + 1u;
  let room = select(1u << (2u * ((class_zoom - child) & 15u)), 0u, child > class_zoom);
  var n = tunnels;
  let h = node_seed(CAVE_TAG_SPLIT ^ k, zoom, c);
  var keep = 0xffu & ~on;
  for (var axis = 0u; axis < 3u && n > 0u; axis += 1u) {
    let upper = select(select(0xf0u, 0xccu, axis == 1u), 0xaau, axis == 0u);
    let weight = 0x4000u + ((h >> (8u * axis)) & 0xffu) * 0x80u;
    let w = (weight << 16u) | (rotl(h, 11u * axis + 5u) & 0xffffu);
    let lo = halve(n, countOneBits(keep & ~upper) * room, countOneBits(keep & upper) * room, w);
    let up = ((want >> axis) & 1u) == 1u;
    n = select(lo, n - lo, up);
    keep &= select(~upper, upper, up);
  }
  return n;
}

/// What a node holds as its readers read it (`cave::Holds`): its piece's blocks and sides,
/// and the tunnels of each class not yet started in it, the first four then the last.
struct Holds {
  blocks: u32,
  side_in: u32,
  side_out: u32,
  tunnels: vec4<u32>,
  last: u32,
}

/// `cave::holds`.
fn cave_holds(zoom: u32, c: vec3<i32>) -> Holds {
  var enter = vec3<i32>(0i);
  var leave = vec3<i32>(0i);
  var side_in = 6u;
  var side_out = 6u;
  var blocks = 0u;
  var tunnels = TUNNELS_FIRST;
  var last = TUNNELS_LAST;
  var z = CAVE_ROOT;
  while (z <= CAVE_PIPE) {
    let here = c >> vec3<u32>(zoom - z);
    let k = z - CAVE_ROOT;
    if (k < 5u && blocks == 0u && ((k == 4u && last > 0u) || (k < 4u && tunnels[k] > 0u))) {
      let s = spawn(z, here, k);
      enter = s.enter;
      leave = s.leave;
      side_in = s.side_in;
      side_out = s.side_out;
      blocks = s.blocks;
      if (k == 4u) {
        last -= 1u;
      } else {
        tunnels[k] -= 1u;
      }
    }
    let none = tunnels.x + tunnels.y + tunnels.z + tunnels.w + last == 0u;
    if (z == zoom || (blocks == 0u && none)) {
      return Holds(blocks, side_in, side_out, tunnels, last);
    }
    let i = child_idx(c >> vec3<u32>(zoom - (z + 1u)));
    var on = 0u;
    if (blocks != 0u) {
      let p = hand_down(z, here, enter, leave, side_in, side_out, blocks, i);
      enter = p.enter;
      leave = p.leave;
      side_in = p.side_in;
      side_out = p.side_out;
      blocks = p.blocks;
      on = p.on;
    }
    tunnels = vec4<u32>(
      tunnels_down(z, here, 0u, tunnels.x, on, i),
      tunnels_down(z, here, 1u, tunnels.y, on, i),
      tunnels_down(z, here, 2u, tunnels.z, on, i),
      tunnels_down(z, here, 3u, tunnels.w, on, i),
    );
    last = tunnels_down(z, here, 4u, last, on, i);
    z += 1u;
  }
  return Holds(blocks, side_in, side_out, tunnels, last);
}

/// `cave::side_mid`.
fn side_mid(side: u32) -> vec3<i32> {
  var p = vec3<i32>(32i);
  if (side < 6u) {
    p[side / 2u] = select(0i, 64i, (side & 1u) == 1u);
  }
  return p;
}

/// `cave::key`.
fn cave_key(a: vec3<i32>, ab: vec3<i32>, l: i32, h: u32, b: u32) -> i32 {
  let ap = vec3<i32>(i32(b & 3u), i32((b >> 2u) & 3u), i32((b >> 4u) & 3u)) * 16i + 8i - a;
  let pp = dot(ap, ap);
  let t = dot(ap, ab);
  let one = select(l, 1i, l == 0i);
  var d = pp * l - t * t;
  if (t <= 0i) {
    d = pp * one;
  } else if (t >= l) {
    d = (pp - 2i * t + l) * l;
  }
  var n = h ^ (b * 0x9e3779b9u);
  n ^= n >> 15u;
  n *= 0x2c1b3c6du;
  n ^= n >> 12u;
  return d + i32(n % CAVE_NUDGE) * one;
}

/// `cave::hollow_from`: how many of a pipe cell's blocks `1 << t` a side from block `lo` are
/// hollow, each block's key read once and the node's ranked against them all at once, a
/// lane a block, four to a vector; a lane past the node's blocks starts ranked past.
fn hollow_from(h: u32, blocks: u32, side_in: u32, side_out: u32, lo: vec3<u32>, t: u32) -> u32 {
  let n = 1u << (3u * t);
  if (blocks >= CELL_BLOCKS) {
    return n;
  }
  let a = side_mid(side_in);
  let ab = side_mid(side_out) - a;
  let l = dot(ab, ab);
  // Lanes 0 to 3 the lower layer's x then y, 4 to 7 the upper's.
  let ids0 = vec4<u32>(lo.x | (lo.y << 2u) | (lo.z << 4u)) + vec4<u32>(0u, 1u, 4u, 5u) * t;
  let ids1 = ids0 + vec4<u32>(16u * t);
  let k0 = cave_key(a, ab, l, h, ids0.x);
  var m0 = vec4<i32>(k0);
  var m1 = vec4<i32>(k0);
  if (t != 0u) {
    m0 = vec4<i32>(k0, cave_key(a, ab, l, h, ids0.y), cave_key(a, ab, l, h, ids0.z), cave_key(a, ab, l, h, ids0.w));
    m1 = vec4<i32>(cave_key(a, ab, l, h, ids1.x), cave_key(a, ab, l, h, ids1.y), cave_key(a, ab, l, h, ids1.z), cave_key(a, ab, l, h, ids1.w));
  }
  let most = vec4<u32>(blocks);
  var b0 = select(most, vec4<u32>(0u), vec4<u32>(0u, 1u, 2u, 3u) < vec4<u32>(n));
  var b1 = select(most, vec4<u32>(0u), vec4<bool>(n > 4u));
  for (var o = 0u; o < CELL_BLOCKS && !(all(b0 >= most) && all(b1 >= most)); o += 1u) {
    let k = vec4<i32>(cave_key(a, ab, l, h, o));
    let ov = vec4<u32>(o);
    b0 += select(vec4<u32>(0u), vec4<u32>(1u), (b0 < most) & ((k < m0) | ((k == m0) & (ov < ids0))));
    b1 += select(vec4<u32>(0u), vec4<u32>(1u), (b1 < most) & ((k < m1) | ((k == m1) & (ov < ids1))));
  }
  return dot(select(vec4<u32>(0u), vec4<u32>(1u), b0 < most) + select(vec4<u32>(0u), vec4<u32>(1u), b1 < most), vec4<u32>(1u));
}

/// `cave::blocks`.
fn cave_blocks(zoom: u32, c: vec3<i32>) -> u32 {
  if (zoom <= CAVE_PIPE) {
    let h = cave_holds(zoom, c);
    var n = h.blocks + h.last * tunnel_blocks(4u);
    n += h.tunnels.x * tunnel_blocks(0u) + h.tunnels.y * tunnel_blocks(1u);
    return n + h.tunnels.z * tunnel_blocks(2u) + h.tunnels.w * tunnel_blocks(3u);
  }
  let p = cave_holds(CAVE_PIPE, c >> vec3<u32>(zoom - CAVE_PIPE));
  if (p.blocks == 0u) {
    return 0u;
  }
  let h = node_seed(CAVE_TAG_BLOCKS, CAVE_PIPE, c >> vec3<u32>(zoom - CAVE_PIPE));
  if (zoom >= CAVE_BLOCK) {
    let lo = vec3<u32>(c >> vec3<u32>(zoom - CAVE_BLOCK)) & vec3<u32>(3u);
    return hollow_from(h, p.blocks, p.side_in, p.side_out, lo, 0u);
  }
  let t = CAVE_BLOCK - zoom;
  return hollow_from(h, p.blocks, p.side_in, p.side_out, vec3<u32>(c << vec3<u32>(t)) & vec3<u32>(3u), t);
}

/// `cave::hollow`: the share of the cube `c` at `zoom` the caves take.
fn hollow(node_zoom: u32, node: vec3<i32>) -> u32 {
  var zoom = node_zoom;
  var c = node;
  if (zoom > LEAF_CELL) {
    c = c >> vec3<u32>(zoom - LEAF_CELL);
    zoom = LEAF_CELL;
  }
  var n = ROOT_BLOCKS;
  if (zoom <= CAVE_ROOT) {
    zoom = CAVE_ROOT;
  } else {
    n = cave_blocks(zoom, c);
  }
  // A block is 8 leaves; the node holds 2^(3(LEAF − zoom)) leaves.
  let shift = 3i + i32(FULL_BITS) - 3i * i32(LEAF_CELL - zoom);
  if (shift >= 0i) {
    return min(n << u32(shift), FULL);
  }
  return n >> u32(-shift);
}

// ---- world.rs ----

/// `Inheritance::of`: the node with nothing handed down, what is placed found down its
/// line from the top.
fn world_root(zoom: u32, c: vec3<i32>) -> vec4<u32> {
  let p = path(zoom, c);
  let slot = OBJECT_ROOT + child_idx(p >> vec3<u32>(zoom));
  return vec4<u32>(tree_down(PLACED, slot, 0u, zoom, p), SHARD_UNKNOWN, LOOKUP_NONE, SLOT_NOTHING);
}

/// The ways' cursor of a node whose ground's cells were found at `place`: its parent's
/// where that names the same brick, found from that brick's root where it names another or
/// where the node is the first the tree's nodes are the world's at, nothing coarser
/// (pxl-engine world.rs `WAYS_FROM`).
fn ways_at(place: u32, before: u32, trail: u32, zoom: u32, c: vec3<i32>) -> u32 {
  let enters = zoom == SHARD_ZOOM + 1u && lookup_here(place);
  if (((place ^ before) & LOOKUP_AT) == 0u && !enters) {
    return trail;
  }
  if (!lookup_here(place) || zoom <= SHARD_ZOOM) {
    return SLOT_NOTHING;
  }
  return way_cursor(ground[brick_at(lookup_slot(place)) + ROOT_AT], lookup_from(place), zoom, c);
}

/// `world::node`, a cell: what is placed, whole in its cube or over the ground's asset
/// and the ways.
fn world_cell(above: vec4<u32>, shard: u32, zoom: u32, c: vec3<i32>, carve: vec4<i32>, hide: vec4<i32>) -> Answer {
  let put = above.x;
  var over = VALUE_AIR;
  var over_fill = FULL;
  if (!cut_holds(carve, zoom, c)) {
    over = tree_val(PLACED, put, hide, zoom, c);
    over_fill = fill_share(PLACED, put);
  }
  var over_four = 0u;
  if (over != VALUE_VOID) {
    if (fills(over)) {
      over_four = placed_four(put, hide, zoom, c, over);
    }
    if (over_fill == FULL) {
      return Answer(KIND_CELL, vec2<u32>(PLACED_ASSET | over, over_four), vec4<u32>(put, shard, above.z, above.w));
    }
  }
  let g = ground_cell(shard, above.z, zoom, c);
  let trail = ways_at(g.place, above.z, above.w, zoom, c);
  let at = vec4<u32>(put, shard, g.place, trail);
  if (g.kind == KIND_LACKS && over == VALUE_VOID) {
    return Answer(g.kind, g.asset, at);
  }
  // The ground's asset, its share less what its caves take, its four palette indices;
  // then what the ways and what is placed write over it.
  var val = VALUE_VOID;
  if (g.kind == KIND_CELL) {
    val = g.asset.x;
  }
  var fill = 0u;
  var four = 0u;
  if (fills(val)) {
    let share = select(FULL, g.part, c.z == g.top - 1i);
    fill = cover(share, VALUE_AIR, g.asset.y).fill;
    four = select(g.fold, four_of(val), zoom >= LEAF_CELL || val != g.palette_idx);
  }
  if (g.kind == KIND_CELL && lookup_here(g.place)) {
    let way = tree_val(WAYS, trail, NO_CUT, zoom, c);
    if (way != VALUE_VOID) {
      let got = cover(fill, way, fill_share(WAYS, trail));
      if (got.theirs) {
        val = way;
        four = four_of(way);
      }
      fill = got.fill;
    }
  }
  if (over != VALUE_VOID) {
    let got = cover(fill, over, over_fill);
    if (got.theirs) {
      val = PLACED_ASSET | over;
      four = over_four;
    }
    fill = got.fill;
  }
  let filled = eighths(fill);
  if (filled == 0u) {
    if (g.kind == KIND_LACKS) {
      return Answer(g.kind, g.asset, at);
    }
    return Answer(KIND_CELL, vec2<u32>(select(VALUE_VOID, PLACED_ASSET | VALUE_AIR, over == VALUE_AIR), 0u), at);
  }
  let asset = val | ((filled % EIGHTHS) << FILL_SHIFT);
  return Answer(KIND_CELL, vec2<u32>(asset, four), at);
}

