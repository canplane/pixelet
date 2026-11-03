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

@group(0) @binding(1) var<storage, read> clipmap: array<vec4<i32>>;
@group(0) @binding(2) var<storage, read> ground: array<u32>;
@group(0) @binding(4) var<storage, read> palette_indices: array<u32>;
@group(0) @binding(5) var<storage, read> shards: array<vec4<u32>>;
@group(0) @binding(6) var<storage, read> overwrites: array<u32>;
@group(0) @binding(7) var<storage, read> pool: array<u32>;

/// What is in a node (`world::node`) and what it hands its children.
struct Answer {
  kind: u32,
  asset: vec2<u32>,
  at: vec4<u32>,
}

// ---- value.rs, answer.rs ----

fn grounds(v: u32) -> bool {
  return v >= GROUNDS_FIRST && v <= GROUNDS_LAST;
}

fn voxel(v: u32) -> bool {
  return v == VALUE_AIR || grounds(v) || (v >= OBJECTS_FIRST && v <= OBJECTS_LAST);
}

fn fills(asset: u32) -> bool {
  let v = asset & VAL_MASK;
  return v != VALUE_VOID && v != VALUE_AIR;
}

fn eighths(fill: u32) -> u32 {
  return min(fill / (FULL / EIGHTHS), EIGHTHS);
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

/// `answer::box_word`.
fn box_word(lo: vec3<u32>, hi: vec3<u32>) -> vec2<u32> {
  let l = min(lo, vec3<u32>(BOX_STEPS - 1u));
  let h = select(min(hi, vec3<u32>(BOX_STEPS)), l + 1u, hi <= l) - 1u;
  return vec2<u32>(
    l.x | (h.x << BOX_BITS) | (l.y << (2u * BOX_BITS)),
    h.y | (l.z << BOX_BITS) | (h.z << (2u * BOX_BITS)),
  );
}

fn box_none(b: vec2<u32>) -> bool {
  return all(b == BOX_NONE);
}

/// `answer::box_join`.
fn box_join(a: vec2<u32>, b: vec2<u32>) -> vec2<u32> {
  if (box_none(a)) {
    return b;
  }
  if (box_none(b)) {
    return a;
  }
  return box_word(min(box_first(a), box_first(b)), max(box_past(a), box_past(b)));
}

fn box_first(w: vec2<u32>) -> vec3<u32> {
  let m = BOX_STEPS - 1u;
  return vec3<u32>(w.x & m, (w.x >> (2u * BOX_BITS)) & m, (w.y >> BOX_BITS) & m);
}

fn box_past(w: vec2<u32>) -> vec3<u32> {
  let m = BOX_STEPS - 1u;
  return vec3<u32>((w.x >> BOX_BITS) & m, w.y & m, (w.y >> (2u * BOX_BITS)) & m) + 1u;
}

// ---- overwrite.rs: a tree of two columns, what is placed or the ways ----

const PLACED = 0u;
const WAYS = 1u;

fn first_child(tree: u32, slot: u32) -> u32 {
  if (tree == WAYS) {
    return pool[slot];
  }
  return overwrites[slot];
}

fn tree_data(tree: u32, slot: u32) -> u32 {
  if (tree == WAYS) {
    return pool[arrayLength(&pool) / 2u + slot];
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

/// `overwrite::fill_share` of what is placed; a way's at any cell asked is whole.
fn fill_share(s: u32) -> u32 {
  if (is_folded(s)) {
    return tree_data(PLACED, s >> SLOT_SHIFT) >> 16u;
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

/// `overwrite::object_box`, under a split.
fn object_box(s: u32, zoom: u32) -> vec2<u32> {
  if (s == SLOT_NOTHING) {
    return BOX_NONE;
  }
  if (!is_folded(s)) {
    return BOX_WHOLE;
  }
  let base = first_child(PLACED, s >> SLOT_SHIFT) & ~BRANCH;
  var low = 0u;
  var high = 0u;
  for (var k = 0u; k < 8u; k += 1u) {
    if (slot_at(PLACED, base + k, zoom + 1u) != SLOT_NOTHING) {
      low |= ~k & 7u;
      high |= k;
    }
  }
  if ((low | high) == 0u) {
    return BOX_NONE;
  }
  let half = BOX_STEPS / 2u;
  let lows = vec3<u32>(low & 1u, (low >> 1u) & 1u, low >> 2u);
  let highs = vec3<u32>(high & 1u, (high >> 1u) & 1u, high >> 2u);
  return box_word(select(vec3<u32>(half), vec3<u32>(0u), lows != vec3<u32>(0u)), select(vec3<u32>(half), vec3<u32>(BOX_STEPS), highs != vec3<u32>(0u)));
}

/// `overwrite::cells_by`.
fn cells_by(s: u32, d: u32) -> bool {
  if (s == SLOT_NOTHING) {
    return false;
  }
  let word = tree_data(PLACED, s >> SLOT_SHIFT);
  if (!is_folded(s)) {
    return voxel(word & 0xffffu);
  }
  let shift = FILL_BITS * (d + 1u);
  return (word >> 16u) + 2u > select(0u, FULL >> (shift & 31u), shift <= FULL_BITS);
}

/// `overwrite::whole`.
fn whole(s: u32, carve: vec4<i32>, zoom: u32, at: vec3<i32>) -> bool {
  if (is_folded(s)) {
    return false;
  }
  let cz = u32(carve.w);
  if (carve.w == NO_CUT.w || cz <= zoom) {
    return true;
  }
  return any(path(zoom, carve.xyz >> vec3<u32>(cz - zoom)) != path(zoom, at));
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
    let key = morton(vec2<u32>(own) & vec2<u32>((1u << SHARD_ZOOM) - 1u));
    nine = shards[key / 4u][key % 4u];
  }
  return SHARD_KNOWN | (nine << SHARD_SERVED_SHIFT);
}

fn is_past(shard: u32) -> bool {
  return (shard & SHARD_KNOWN) != 0u && ((shard >> SHARD_SERVED_SHIFT) & SHARD_OWN) == 0u;
}

/// `area::is_wall`, off the shard the node `(cell, b)` knows.
fn is_wall(shard: u32, cell: u32, b: vec2<i32>) -> bool {
  let served = shard >> SHARD_SERVED_SHIFT;
  if (cell < LEAF_CELL) {
    return false;
  }
  let leaf = b >> vec2<u32>(cell - LEAF_CELL);
  let own = b >> vec2<u32>(cell - SHARD_ZOOM);
  let to = LEAF_CELL - SHARD_ZOOM;
  // The shards of the leaf cells about it: two a side at most.
  let lo = ((leaf - 1i) >> vec2<u32>(to)) - own;
  let hi = ((leaf + 1i) >> vec2<u32>(to)) - own;
  let dx = vec2<i32>(lo.x, hi.x) + 1i;
  let dy = vec2<i32>(lo.y, hi.y) + 1i;
  let at = vec4<u32>(vec4<i32>(dy.x * 3i + dx.x, dy.x * 3i + dx.y, dy.y * 3i + dx.x, dy.y * 3i + dx.y));
  return any(((vec4<u32>(served) >> at) & vec4<u32>(1u)) == vec4<u32>(0u));
}

/// `wrap_idx`.
fn wrap_idx(zoom: u32, x: i32) -> i32 {
  if (zoom == 0u) {
    return 0i;
  }
  return (x << (32u - zoom)) >> (32u - zoom);
}

// ---- clipmap.rs ----

fn spread(v: u32) -> u32 {
  var s = (v | (v << 8u)) & 0x00ff00ffu;
  s = (s | (s << 4u)) & 0x0f0f0f0fu;
  s = (s | (s << 2u)) & 0x33333333u;
  return (s | (s << 1u)) & 0x55555555u;
}

/// `clipmap::idx_of`.
fn morton(at: vec2<u32>) -> u32 {
  return spread(at.x) | (spread(at.y) << 1u);
}

fn lookup(cell: u32, clip: u32, slot: u32, here: bool) -> u32 {
  return (cell << LOOKUP_CELL_SHIFT) | (clip << LOOKUP_FROM_SHIFT) | (u32(here) << LOOKUP_SLOT_BITS) | slot;
}

fn lookup_cell(l: u32) -> u32 {
  return l >> LOOKUP_CELL_SHIFT;
}

fn lookup_clip(l: u32) -> u32 {
  return (l >> LOOKUP_FROM_SHIFT) & ((1u << ZOOM_BITS) - 1u);
}

fn lookup_slot(l: u32) -> u32 {
  return l & ((1u << LOOKUP_SLOT_BITS) - 1u);
}

fn lookup_here(l: u32) -> bool {
  return (l & (1u << LOOKUP_SLOT_BITS)) != 0u;
}

/// `clipmap::cell_at`.
fn cell_at(zoom: u32, clip: u32, c: vec2<i32>) -> vec2<i32> {
  let s = zoom - clip;
  return vec2<i32>(wrap_idx(clip, c.x >> s), c.y >> s);
}

struct Step {
  place: u32,
  at: vec2<u32>,
}

/// `clipmap::step`: the zooms `above` leaves unknown, finest first.
fn step_to(above: u32, cell: u32, g: vec2<i32>) -> Step {
  let known = above != LOOKUP_NONE && cell >= lookup_clip(above);
  var stop = FIRST_CELL - 1u;
  var f = lookup(cell, FIRST_CELL, 0u, false);
  if (known) {
    stop = lookup_cell(above);
    f = (above & LOOKUP_AT) | (cell << LOOKUP_CELL_SHIFT);
  }
  for (var z = cell; z > stop; z -= 1u) {
    let q = cell_at(cell, z, g) >> vec2<u32>(BRICK_SHIFT);
    let m = vec2<i32>(i32(ACROSS) - 1i);
    let e = vec2<u32>(q & m);
    let row = clipmap[(z - FIRST_CELL) * CLIP_ENTRIES + e.y * ACROSS + e.x];
    if (row.z != 0i && row.x == q.x && row.y == q.y) {
      f = lookup(cell, z, u32(row.z - 1i), true);
      break;
    }
  }
  return Step(f, vec2<u32>(cell_at(cell, lookup_clip(f), g) & vec2<i32>(i32(BRICK) - 1i)));
}

fn held_to(cell: u32) -> u32 {
  return clamp(cell, FIRST_CELL, LEAF_CELL);
}

/// A bound word's lowest and highest (`answer::bound_of`).
fn bound_of(w: i32) -> vec2<i32> {
  return vec2<i32>((w << 16u) >> 16u, w >> 16u);
}

/// `ground::Bounds`: the ground's lowest and highest, then the ways'.
fn bounds_of(a: i32, b: i32) -> vec4<i32> {
  return vec4<i32>(bound_of(a), bound_of(b));
}

const BOUNDS_EMPTY = vec4<i32>(EMPTY_LO, EMPTY_HI, EMPTY_LO, EMPTY_HI);
const BOUNDS_OPEN = vec4<i32>(UNBOUNDED_LO, UNBOUNDED_HI, UNBOUNDED_LO, UNBOUNDED_HI);

/// What the clip levels hold of a node (`clipmap::bounded`, `clipmap::bound_over`): its
/// bounds, the clip level read, whether one held it, whether that one holds all of it, and
/// where its cells were found.
struct NodeBound {
  b: vec4<i32>,
  clip: u32,
  here: bool,
  all: bool,
  place: u32,
}

/// `clipmap::held_of`: the eye's clip level at `cell` of a node `2^l` bricks a side.
fn held_of(place: u32, cell: u32, g: vec2<i32>, l: u32) -> NodeBound {
  let origin = clipmap[ORIGIN_AT + cell - FIRST_CELL];
  if (origin.z == 0i) {
    return NodeBound(BOUNDS_EMPTY, cell, true, false, place);
  }
  let q = vec2<i32>(wrap_idx(cell, g.x), g.y) >> vec2<u32>(BRICK_SHIFT);
  let d = vec2<i32>(wrap_idx(cell - BRICK_SHIFT, q.x - origin.x), q.y - origin.y);
  let side = 1i << l;
  let across = i32(ACROSS);
  if (d.x >= across || d.y >= across || d.x + side <= 0i || d.y + side <= 0i) {
    return NodeBound(BOUNDS_EMPTY, cell, true, false, place);
  }
  let level = min(l, JOINED_LEVELS);
  let at = vec2<u32>(q & vec2<i32>(across - 1i)) >> vec2<u32>(level);
  let under = ACROSS >> (level - 1u);
  let e = clipmap[JOINED_AT + (cell - FIRST_CELL) * JOINED_ENTRIES + (ACROSS * ACROSS - under * under) / 3u + morton(at)];
  if (u32(e.x) != (u32(origin.x) ^ 0x80000000u) || u32(e.y) != (u32(origin.y) ^ 0x80000000u)) {
    return NodeBound(BOUNDS_OPEN, cell, true, true, place);
  }
  let b = bounds_of(e.z, e.w);
  if (b.x == UNBOUNDED_LO && b.y == UNBOUNDED_HI) {
    return NodeBound(b, cell, true, true, place);
  }
  return NodeBound(b, cell, true, all(d >= vec2<i32>(0i)) && all(d + side <= vec2<i32>(across)), place);
}

/// `clipmap::bounded`.
fn bounded(place: u32, cell: u32, zoom: u32, c: vec2<i32>) -> NodeBound {
  // The node's first cell at `cell`, or the cell holding it (`idx_at`).
  let g = select(c >> vec2<u32>((zoom - cell) & 31u), c << vec2<u32>((cell - zoom) & 31u), cell >= zoom);
  if (cell > zoom + BRICK_LEVELS) {
    return held_of(place, cell, g, cell - zoom - BRICK_LEVELS);
  }
  let s = step_to(place, cell, g);
  let clip = lookup_clip(s.place);
  if (!lookup_here(s.place)) {
    return NodeBound(vec4<i32>(0i), clip, false, false, s.place);
  }
  let j = select(0u, clip - zoom, zoom < clip);
  let at = s.at >> vec2<u32>(j);
  let level_at = (4u * BRICK_CELLS - ((4u * BRICK_CELLS) >> (2u * j))) / 3u;
  let e = lookup_slot(s.place) * BRICK_WORDS + LEVELS_AT + LEVEL_ENTRY_WORDS * (level_at + morton(at));
  return NodeBound(bounds_of(i32(ground[e]), i32(ground[e + 1u])), clip, true, true, s.place);
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
    b = vec4<i32>(min(b.x, got.b.x), max(b.y, got.b.y), min(b.z, got.b.z), max(b.w, got.b.w));
    if ((b.x <= UNBOUNDED_LO && b.y >= UNBOUNDED_HI) || (got.clip <= bottom && got.all)) {
      return NodeBound(b, z, true, true, found);
    }
    z = got.clip - 1u;
  }
  return NodeBound(BOUNDS_OPEN, FIRST_CELL, true, true, found);
}

/// `ground::Cell`, its fold a word.
struct Cell {
  idx: u32,
  top: i32,
  part: u32,
  fold: u32,
}

/// `clipmap::ground`.
fn ground_cell(shard: u32, place: u32, zoom: u32, b: vec2<i32>) -> Cell {
  let clip = held_to(zoom);
  let s = step_to(place, clip, cell_at(zoom, clip, b));
  var fold = 0u;
  var column = 0u;
  var own = VALUE_VOID;
  if (lookup_here(s.place)) {
    let slot = lookup_slot(s.place);
    let i = morton(s.at);
    let base = slot * BRICK_WORDS;
    fold = palette_indices[slot * BRICK_CELLS + i];
    column = ground[base + COLUMNS_AT + i];
    own = (ground[base + LAID_AT + i / LAID_CELLS] >> (8u * (i % LAID_CELLS))) & 0xffu;
  }
  let t = top_of(zoom, i32(column << 16u) >> 16u);
  return Cell(select(own, VALUE_UNKNOWN, is_wall(shard, zoom, b)), t.cells, t.part, fold);
}

// ---- ground.rs ----

struct Top {
  cells: i32,
  part: u32,
}

/// `ground::top` of `h` leaf cells.
fn top_of(cell: u32, h: i32) -> Top {
  if (cell >= LEAF_CELL) {
    return Top(h << (cell - LEAF_CELL), FULL);
  }
  let n = LEAF_CELL - cell;
  let cells = h >> n;
  let rest = u32(h) & ((1u << n) - 1u);
  let part = select(rest >> ((n - FULL_BITS) & 31u), rest << ((FULL_BITS - n) & 31u), n <= FULL_BITS);
  if (eighths(part) == 0u) {
    return Top(cells, FULL);
  }
  return Top(cells + 1i, part);
}

fn face_height(cell: u32, t: i32) -> i32 {
  if (cell >= LEAF_CELL) {
    return t >> (cell - LEAF_CELL);
  }
  return t << (LEAF_CELL - cell);
}

fn leaf_cells(cell: u32) -> i32 {
  return select(1i, 1i << ((cell - LEAF_CELL) & 31u), cell >= LEAF_CELL);
}

fn coarsest(zoom: u32, far: u32) -> u32 {
  return max(far, zoom + 1u);
}

/// `ground::slab`: the highest its zooms round `hi` to.
fn slab_over(zoom: u32, relief_hi: i32, far: u32, near: u32) -> i32 {
  let hi = select(relief_hi, max(relief_hi, SEA_LEVEL), zoom < SHARD_ZOOM);
  var over = EMPTY_HI;
  for (var z = coarsest(zoom, far); z <= near; z += 1u) {
    over = max(over, face_height(z, top_of(z, hi).cells));
  }
  return over;
}

/// `ground::meets`.
fn meets(span: vec2<i32>, zoom: u32, z: i32, near: u32) -> bool {
  let e = max(near, LEAF_CELL) - LEAF_CELL;
  let s = LEAF_CELL + e - zoom;
  let first = ((z + z_start(zoom)) << s) - (SEA_CELLS << e);
  return first < (span.y << e) && first + (1i << s) > (span.x << e);
}

/// The step of a node `shift` over the leaves a leaf cell `v` lies in (`box_along_z`).
fn z_step(v: i32, z: i32, shift: u32) -> i32 {
  let first = v + SEA_CELLS;
  let scaled = select(first << ((BOX_BITS - shift) & 31u), first >> ((shift - BOX_BITS) & 31u), shift >= BOX_BITS);
  return scaled - (z << BOX_BITS);
}

/// `ground::box_along_z`.
fn box_along_z(span: vec2<i32>, zoom: u32, z_at: i32) -> vec2<u32> {
  if (span.x >= span.y) {
    return BOX_NONE;
  }
  if (zoom >= LEAF_CELL) {
    return BOX_WHOLE;
  }
  let shift = LEAF_CELL - zoom;
  let z = z_at + z_start(zoom);
  let first = z_step(span.x, z, shift);
  let past = select(z_step(span.y, z, shift), z_step(span.y - 1i, z, shift) + 1i, shift >= BOX_BITS);
  if (past <= 0i || first >= i32(BOX_STEPS)) {
    return BOX_NONE;
  }
  let lo = u32(clamp(first, 0i, i32(BOX_STEPS) - 1i));
  let hi = u32(clamp(past, 1i, i32(BOX_STEPS)));
  return box_word(vec3<u32>(0u, 0u, lo), vec3<u32>(BOX_STEPS, BOX_STEPS, hi));
}

/// `way::slab`.
fn way_slab(bound: vec2<i32>, first: u32, near: u32) -> vec2<i32> {
  if (bound.x >= bound.y) {
    return vec2<i32>(EMPTY_LO, EMPTY_HI);
  }
  var span = vec2<i32>(EMPTY_LO, EMPTY_HI);
  for (var z = first; z <= near; z += 1u) {
    var got = bound;
    if (z < LEAF_CELL) {
      let k = LEAF_CELL - z;
      let half = 1i << (k - 1u);
      got = vec2<i32>(((bound.x + half) >> k) << k, -(((half - bound.y) >> k) << k));
    }
    span = vec2<i32>(min(span.x, got.x), max(span.y, got.y));
  }
  return span;
}

/// `strata::by_depth`.
fn by_depth(cell: u32, idx: u32, t: i32, z: i32) -> u32 {
  if (z >= t) {
    return VALUE_VOID;
  }
  if (cell < LEAF_CELL || idx == VALUE_UNKNOWN) {
    return idx;
  }
  let s = leaf_cells(cell);
  var filler = STRATA_SOIL;
  var sandstone = 0i;
  if (idx == STRATA_SAND || idx == STRATA_BEACH) {
    filler = STRATA_SAND;
    sandstone = STRATA_SANDSTONE_DEPTH;
  } else if (idx == STRATA_WATER) {
    filler = STRATA_WATER;
  }
  let face = t - s;
  let filled = face - STRATA_FILLER_DEPTH * s;
  if (z >= face) {
    return idx;
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
  return col.idx != VALUE_UNKNOWN && z < under && z >= rock(cell, face);
}

/// `ground::node` of a cell: its kind and asset, the column read (`Cell`), where it was held.
struct GroundCell {
  kind: u32,
  asset: vec2<u32>,
  idx: u32,
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
fn ground_cell_node(shard: u32, place: u32, zoom: u32, c: vec3<i32>) -> GroundCell {
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
  let col = ground_cell(shard, r.place, zoom, c.xy);
  var asset = VALUE_VOID;
  if (col.idx == VALUE_UNKNOWN || grounds(col.idx)) {
    asset = by_depth(zoom, col.idx, col.top, c.z);
  }
  var cave = 0u;
  if (asset != VALUE_VOID && opens(col, zoom, c.z)) {
    cave = hollow(zoom, c);
  }
  return GroundCell(KIND_CELL, vec2<u32>(asset, cave), col.idx, col.top, col.part, col.fold, r.place);
}

/// `ground::node` of a node coarser than asked: its kind and asset, the ways' bound
/// read with its own, where it was held.
struct GroundSplit {
  kind: u32,
  asset: vec2<u32>,
  ways: vec2<i32>,
  place: u32,
}

/// `ground::node`, not `is_cell`.
fn ground_split_node(shard: u32, place: u32, zoom: u32, c: vec3<i32>, far: u32, near: u32) -> GroundSplit {
  let none = vec2<i32>(EMPTY_LO, EMPTY_HI);
  if (is_past(shard)) {
    if (c.z >= SEA_LEVEL && zoom > SHARD_ZOOM) {
      return GroundSplit(KIND_CELL, vec2<u32>(VALUE_VOID, 0u), none, LOOKUP_NONE);
    }
    return GroundSplit(KIND_SPLIT, BOX_WHOLE, none, LOOKUP_NONE);
  }
  if (zoom < TOP_ZOOM) {
    return GroundSplit(KIND_SPLIT, BOX_WHOLE, none, place);
  }
  let r = bound_over(place, min(coarsest(zoom, far), near), near, zoom, c.xy);
  if (!r.here) {
    return GroundSplit(KIND_SPLIT, BOX_WHOLE, none, r.place);
  }
  let span = vec2<i32>(UNBOUNDED_LO, slab_over(zoom, r.b.y, far, near));
  if (meets(span, zoom, c.z, near)) {
    return GroundSplit(KIND_SPLIT, box_along_z(span, zoom, c.z), r.b.zw, r.place);
  }
  return GroundSplit(KIND_CELL, vec2<u32>(VALUE_VOID, 0u), r.b.zw, r.place);
}

// ---- pxl-content cave.rs, share.rs ----

/// `xpute_kit::derive_seed_u32`.
fn seed_mix(base: u32, tag: u32) -> u32 {
  var z = (base ^ tag) + 0x9e3779b9u;
  z = (z ^ (z >> 16u)) * 0x85ebca6bu;
  z = (z ^ (z >> 13u)) * 0xc2b2ae35u;
  return z ^ (z >> 16u);
}

/// `seed::node_seed`.
fn node_seed(tag: u32, zoom: u32, c: vec3<i32>) -> u32 {
  var h = seed_mix(seed_mix(MASTER_SEED, tag), zoom);
  h = seed_mix(h, u32(wrap_idx(zoom, c.x)));
  h = seed_mix(h, u32(c.y));
  return seed_mix(h, u32(c.z));
}

fn rotl(v: u32, s: u32) -> u32 {
  return (v << (s & 31u)) | (v >> ((32u - s) & 31u));
}

/// `share::halve`.
fn halve(n: u32, room_0: u32, room_1: u32, w: u32) -> u32 {
  let scaled = n * (w >> 16u);
  let part = (scaled >> 16u) + u32((scaled & 0xffffu) > (w & 0xffffu));
  let least = select(0u, n - room_1, n > room_1);
  if (part < least) {
    return least;
  }
  return min(part, min(n, room_0));
}

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
  let h = node_seed(CAVE_SPAWN, zoom, c);
  let g = node_seed(CAVE_SPAWN ^ 1u, zoom, c);
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
fn walk_at(u: u32, v: u32, h: u32, i: u32) -> u32 {
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
  let wh = node_seed(CAVE_WALK, zoom, c);
  let d = u ^ v;
  let n = select(countOneBits(d) + 1u, 4u, is_aside(d, wh));
  var on = 0u;
  var mine_enter = vec3<i32>(0i);
  var mine_leave = vec3<i32>(0i);
  var mine_in = 6u;
  var mine_out = 6u;
  var at = 8u;
  var cells = vec4<u32>(0u);
  var cur = enter;
  var side = side_in;
  for (var i = 0u; i < n; i += 1u) {
    let k = walk_at(u, v, wh, i);
    on |= 1u << k;
    let part_enter = cur;
    let part_in = side;
    var part_leave = leave;
    var part_out = side_out;
    if (i + 1u < n) {
      let half = (1i << (CAVE_PIPE - zoom)) / 2i;
      let co = (c << vec3<u32>(CAVE_PIPE - zoom)) + half * vec3<i32>(i32(k & 1u), i32((k >> 1u) & 1u), i32(k >> 2u));
      let next = walk_at(u, v, wh, i + 1u);
      let axis = countTrailingZeros(k ^ next);
      let up = ((next >> axis) & 1u) == 1u;
      let h = node_seed(CAVE_CROSS ^ i, zoom, c);
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
      mine_enter = part_enter;
      mine_leave = part_leave;
      mine_in = part_in;
      mine_out = part_out;
      at = i;
    }
  }
  // Its blocks along the walk, each part its share of what is left.
  var mine_blocks = 0u;
  var rest = cells.x + cells.y + cells.z + cells.w;
  var left = blocks;
  for (var i = 0u; i < n && i <= at; i += 1u) {
    let cnt = cells[i];
    rest -= cnt;
    let weight = min((cnt << 16u) / max(cnt + rest, 1u), 0xffffu);
    let got = halve(left, cnt * CELL_BLOCKS, rest * CELL_BLOCKS, (weight << 16u) | (rotl(wh, 7u * i + 3u) & 0xffffu));
    if (i == at) {
      mine_blocks = got;
    }
    left -= got;
  }
  return Piece(mine_enter, mine_leave, mine_in, mine_out, mine_blocks, on);
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
  let h = node_seed(CAVE_SPLIT ^ k, zoom, c);
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
fn holds(zoom: u32, c: vec3<i32>) -> Holds {
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

/// `cave::block_hollow`.
fn block_hollow(h: u32, blocks: u32, side_in: u32, side_out: u32, b: u32) -> bool {
  if (blocks >= CELL_BLOCKS) {
    return true;
  }
  let a = side_mid(side_in);
  let ab = side_mid(side_out) - a;
  let l = dot(ab, ab);
  let mine = cave_key(a, ab, l, h, b);
  var before = 0u;
  for (var o = 0u; o < CELL_BLOCKS && before < blocks; o += 1u) {
    let k = cave_key(a, ab, l, h, o);
    if (k < mine || (k == mine && o < b)) {
      before += 1u;
    }
  }
  return before < blocks;
}

/// `cave::blocks`.
fn cave_blocks(zoom: u32, c: vec3<i32>) -> u32 {
  if (zoom <= CAVE_PIPE) {
    let h = holds(zoom, c);
    var n = h.blocks + h.last * tunnel_blocks(4u);
    n += h.tunnels.x * tunnel_blocks(0u) + h.tunnels.y * tunnel_blocks(1u);
    return n + h.tunnels.z * tunnel_blocks(2u) + h.tunnels.w * tunnel_blocks(3u);
  }
  let p = holds(CAVE_PIPE, c >> vec3<u32>(zoom - CAVE_PIPE));
  if (p.blocks == 0u) {
    return 0u;
  }
  let h = node_seed(CAVE_BLOCKS, CAVE_PIPE, c >> vec3<u32>(zoom - CAVE_PIPE));
  if (zoom >= CAVE_BLOCK) {
    let lo = vec3<u32>(c >> vec3<u32>(zoom - CAVE_BLOCK)) & vec3<u32>(3u);
    return u32(block_hollow(h, p.blocks, p.side_in, p.side_out, lo.x | (lo.y << 2u) | (lo.z << 4u)));
  }
  let t = CAVE_BLOCK - zoom;
  let side = 1u << t;
  let lo = vec3<u32>(c << vec3<u32>(t)) & vec3<u32>(3u);
  var n = 0u;
  for (var b = 0u; b < side * side * side; b += 1u) {
    let at = vec3<u32>(b & (side - 1u), (b >> t) & (side - 1u), b >> (2u * t));
    let sum = lo + at;
    n += u32(block_hollow(h, p.blocks, p.side_in, p.side_out, sum.x | (sum.y << 2u) | (sum.z << 4u)));
  }
  return n;
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
fn world_root(zoom: u32, at: vec3<i32>) -> vec4<u32> {
  let p = path(zoom, at);
  let slot = OBJECT_ROOT + child_idx(p >> vec3<u32>(zoom));
  return vec4<u32>(tree_down(PLACED, slot, 0u, zoom, p), SHARD_UNKNOWN, LOOKUP_NONE, SLOT_NOTHING);
}

/// `Inheritance::child`: what its child `c` at `zoom` is handed.
fn world_child(at: vec4<u32>, zoom: u32, c: vec3<i32>) -> vec4<u32> {
  return vec4<u32>(tree_child(PLACED, at.x, zoom, path(zoom, c)), at.y, at.z, tree_child(WAYS, at.w, zoom, c));
}

/// The ways' cursor of a node whose ground's cells were found at `place`: its parent's
/// where that names the same brick, found from that brick's root where it names another.
fn ways_at(place: u32, before: u32, trail: u32, zoom: u32, c: vec3<i32>) -> u32 {
  if (((place ^ before) & LOOKUP_AT) == 0u) {
    return trail;
  }
  if (!lookup_here(place)) {
    return SLOT_NOTHING;
  }
  return way_cursor(ground[lookup_slot(place) * BRICK_WORDS + ROOT_AT], lookup_clip(place), zoom, c);
}

/// `world::node`, a cell: what is placed, whole in its cube or over the ground's asset
/// and the ways.
fn world_cell(above: vec4<u32>, shard: u32, zoom: u32, c: vec3<i32>, carve: vec4<i32>, hide: vec4<i32>) -> Answer {
  let put = above.x;
  var over = VALUE_AIR;
  var over_fill = FULL;
  if (!cut_holds(carve, zoom, c)) {
    over = tree_val(PLACED, put, hide, zoom, c);
    over_fill = fill_share(put);
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
  let g = ground_cell_node(shard, above.z, zoom, c);
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
    four = select(g.fold, four_of(val), zoom >= LEAF_CELL || val != g.idx);
  }
  if (g.kind == KIND_CELL && lookup_here(g.place)) {
    let way = tree_val(WAYS, trail, NO_CUT, zoom, c);
    if (way != VALUE_VOID) {
      let got = cover(fill, way, FULL);
      if (got.theirs || !fills(val)) {
        val = way;
        four = four_of(way);
      }
      fill = got.fill;
    }
  }
  if (over != VALUE_VOID) {
    let got = cover(fill, over, over_fill);
    if (got.theirs || !fills(val)) {
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

/// `world::node`, coarser than asked: what is placed splits it where it may fill, and the
/// ways what the ground leaves air where they may; a split's box every source's own joined.
fn world_split(above: vec4<u32>, shard: u32, zoom: u32, c: vec3<i32>, far: u32, near: u32) -> Answer {
  let put = above.x;
  let splits = cells_by(put, near - zoom);
  var placed = BOX_NONE;
  if (splits) {
    placed = object_box(put, zoom);
  }
  let g = ground_split_node(shard, above.z, zoom, c, far, near);
  let at = vec4<u32>(put, shard, g.place, ways_at(g.place, above.z, above.w, zoom, c));
  let ways = way_slab(g.ways, coarsest(zoom, far), near);
  if (!splits && g.kind != KIND_SPLIT && !(g.kind == KIND_CELL && meets(ways, zoom, c.z, near))) {
    return Answer(g.kind, g.asset, at);
  }
  var own = BOX_NONE;
  if (g.kind == KIND_SPLIT) {
    own = g.asset;
  } else if (g.kind == KIND_LACKS) {
    own = BOX_WHOLE;
  }
  let joined = box_join(box_join(own, box_along_z(ways, zoom, c.z)), placed);
  if (box_none(joined)) {
    return Answer(KIND_CELL, vec2<u32>(VALUE_VOID, 0u), at);
  }
  return Answer(KIND_SPLIT, joined, at);
}

/// What is in the node `c` at `zoom`, asked from `far` to `near`, `above` what the
/// node over it handed it (`world::node`).
fn world_node(above: vec4<u32>, zoom: u32, c: vec3<i32>, far: u32, near: u32, carve: vec4<i32>, hide: vec4<i32>) -> Answer {
  let shard = shard_at(above.y, zoom, c.xy);
  if (zoom >= near || (zoom >= LEAF_CELL && whole(above.x, carve, zoom, c))) {
    return world_cell(above, shard, zoom, c, carve, hide);
  }
  return world_split(above, shard, zoom, c, far, near);
}
