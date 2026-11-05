// pxl-render/way.wgsl

// The ways of the bricks `fresh` names, by hand (pxl-engine fill.rs `spans`, way.rs
// `column`): one a `z`, a thread a column of the brick and its ring, row by row from the
// ring's first, each its spans once off the brick's lines into the brick's scratch, which
// its levels and its ways' tree read its cells and its neighbors' off. Over `height_cs`'s
// tops: a way is told from their faces. Each brick's record as handed whole: where it lies
// and its shard say the wall (`area::is_wall`). A column's spans written as each is made,
// none held. Floats as the Rust takes them, in its order: a column's ring height
// (`ground::height`) and a segment's crossing (`way::crossing`).

@group(0) @binding(0) var<storage, read> fresh: array<u32>;
@group(0) @binding(1) var<storage, read> ground: array<u32>;
@group(0) @binding(2) var<storage, read_write> scratch: array<u32>;

// ---- way.rs, fill.rs: a run's cells, and a column's spans as kept ----

/// Whether the leaf cells `run` fill any cell of zoom `cell` (`way::fills_any`).
fn fills_any(cell: u32, run: vec2<i32>) -> bool {
  let r = fill_range(cell, run);
  return r.x <= r.y;
}

/// A column's spans' head word (fill.rs `head_word`): its count, `SPANS_PAST` where they
/// did not fit, its face cell over them.
fn head_word(face: i32, n: u32, fits: bool) -> u32 {
  let held = select(clamp(face, -32767i, 32767i), -32768i, face == FACE_NONE);
  return n | select(SPANS_PAST, 0u, fits) | (u32(held) << 16u);
}

// ---- clipmap.rs: a brick as its fill reads it (`Brick`, `Columns`) ----

/// Word `k` of the lines of the brick whose record begins at `rec`.
fn line(rec: u32, k: u32) -> u32 {
  return fresh[rec + LINES_AT + k];
}

fn line_float(rec: u32, k: u32) -> f32 {
  return bitcast<f32>(line(rec, k));
}

fn brick_cell(rec: u32) -> u32 {
  return fresh[rec + FILL_CELL];
}

/// The brick's first word in the ground's buffer.
fn brick_base(rec: u32) -> u32 {
  return fresh[rec + FILL_BRICK] * BRICK_WORDS;
}

/// Where the brick lies in its tile, in bricks (`place_of`).
fn brick_place(rec: u32) -> vec2<u32> {
  let w = fresh[rec + FILL_PLACE];
  return vec2<u32>(w & 0xffffu, w >> 16u);
}

/// Whether column `c` is the brick's or its ring's (`is_held`).
fn is_held(c: vec2<i32>) -> bool {
  return all(c >= vec2<i32>(-1i)) && all(c <= vec2<i32>(i32(BRICK)));
}

/// Whether column `c` is the brick's own.
fn is_own(c: vec2<i32>) -> bool {
  return all(c >= vec2<i32>(0i)) && all(c < vec2<i32>(i32(BRICK)));
}

/// Column `c`'s bin word, a ring column's the edge bin it borders: its first entry and its
/// count; none past the ring (`bin_of`).
fn bin_of(rec: u32, c: vec2<i32>) -> vec2<u32> {
  if (!is_held(c)) {
    return vec2<u32>(0u);
  }
  let x = vec2<u32>(clamp(c, vec2<i32>(0i), vec2<i32>(i32(BRICK) - 1i)));
  let b = (x.y >> WAYS_SHIFT) * WAYS_BINS_ACROSS + (x.x >> WAYS_SHIFT);
  let w = line(rec, HEADER_WORDS + b);
  return vec2<u32>(w & 0xffffu, w >> 16u);
}

/// Entry `e` of the brick's lines: the place of a segment among its (`entry_of`).
fn entry_of(rec: u32, e: u32) -> u32 {
  return (line(rec, LINES_ENTRIES_AT + e / 2u) >> (16u * (e % 2u))) & 0xffffu;
}

/// The words of the brick's segment at `at` (`segment_at`), read as `way::Segment::of` reads
/// them by the `seg_` functions below.
fn segment_at(rec: u32, at: u32) -> vec4<u32> {
  let b = LINES_SEGMENTS_AT + at * SEGMENT_WORDS;
  return vec4<u32>(line(rec, b), line(rec, b + 1u), line(rec, b + 2u), line(rec, b + 3u));
}

/// The `k`-th segment over column `c` (`Columns::segment`).
fn segment(rec: u32, c: vec2<i32>, k: u32) -> vec4<u32> {
  return segment_at(rec, entry_of(rec, bin_of(rec, c).x + k));
}

/// Whether the `k`-th segment over column `c` continues the one before it there
/// (`Columns::joins`).
fn joins(rec: u32, c: vec2<i32>, k: u32) -> bool {
  if (k == 0u) {
    return false;
  }
  let first = bin_of(rec, c).x;
  let e = entry_of(rec, first + k);
  return e == entry_of(rec, first + k - 1u) + 1u && seg_continues(segment_at(rec, e)) == 1u;
}

/// Column `c`'s center in its tile's cells (`Columns::center`).
fn center(rec: u32, c: vec2<i32>) -> vec2<f32> {
  return vec2<f32>(brick_place(rec) * BRICK) + vec2<f32>(c) + CELL_CENTER;
}

/// Whether column `c` is the service area's wall (`Columns::is_walled`).
fn is_walled(rec: u32, c: vec2<i32>) -> bool {
  let cell = brick_cell(rec);
  let first = vec2<i32>(i32(fresh[rec + FILL_FIRST]), i32(fresh[rec + FILL_FIRST + 1u]));
  return is_wall(fresh[rec + FILL_SHARD], cell, vec2<i32>(wrap_idx(cell, first.x + c.x), first.y + c.y));
}

/// Column `c`'s own palette index as its fill laid it, the ring's void
/// (`Columns::palette_idx`, fill.rs `laid_of`).
fn palette_idx(rec: u32, c: vec2<i32>) -> u32 {
  if (!is_own(c)) {
    return VALUE_VOID;
  }
  let i = cell_id(vec2<u32>(c));
  return (ground[brick_base(rec) + LAID_AT + i / LAID_CELLS] >> (8u * (i % LAID_CELLS))) & 0xffu;
}

/// Column `c`'s face in leaf cells (`Columns::face`): a column of the brick's own as
/// `height_cs` wrote it, the ring off its tile's samples; none past the ring.
fn face(rec: u32, c: vec2<i32>) -> i32 {
  if (!is_held(c)) {
    return 0i;
  }
  var h = 0i;
  if (is_own(c)) {
    h = (i32(ground[brick_base(rec) + COLUMNS_AT + cell_id(vec2<u32>(c))]) << 16u) >> 16u;
  } else {
    h = ground_height(rec, vec2<i32>(brick_place(rec) * BRICK) + c);
  }
  let cell = brick_cell(rec);
  return face_height(cell, top_of(cell, h).cells);
}

// ---- way.rs: a segment, its crossing of a column, what a visit writes ----

fn seg_byte(s: vec4<u32>, k: u32) -> u32 {
  return (s.w >> (8u * k)) & 0xffu;
}

/// A word's two `i16`, low first.
fn i16_pair(w: u32) -> vec2<f32> {
  return vec2<f32>(f32(i32(w << 16u) >> 16u), f32(i32(w) >> 16u));
}

fn seg_a(s: vec4<u32>) -> vec2<f32> {
  return i16_pair(s.x) / POINT_SCALE;
}

fn seg_b(s: vec4<u32>) -> vec2<f32> {
  return i16_pair(s.y) / POINT_SCALE;
}

fn seg_tops(s: vec4<u32>) -> vec2<f32> {
  return i16_pair(s.z);
}

fn seg_width(s: vec4<u32>) -> f32 {
  return f32(seg_byte(s, 0u)) / POINT_SCALE;
}

fn seg_kind(s: vec4<u32>) -> u32 {
  return seg_byte(s, 1u);
}

fn seg_form(s: vec4<u32>) -> u32 {
  return seg_byte(s, 2u) & 3u;
}

fn seg_first(s: vec4<u32>) -> u32 {
  return (seg_byte(s, 2u) >> 2u) & 1u;
}

fn seg_last(s: vec4<u32>) -> u32 {
  return (seg_byte(s, 2u) >> 3u) & 1u;
}

fn seg_continues(s: vec4<u32>) -> u32 {
  return (seg_byte(s, 2u) >> 4u) & 1u;
}

fn seg_clear(s: vec4<u32>) -> u32 {
  return seg_byte(s, 3u);
}

/// A vector's length, as the Rust takes it.
fn span_length(v: vec2<f32>) -> f32 {
  return sqrt(v.x * v.x + v.y * v.y);
}

/// The road surface of `s` at the share `t` of it, clamped to its ends (`surface`).
fn surface(s: vec4<u32>, t: f32) -> f32 {
  let tops = seg_tops(s);
  return tops.x + (tops.y - tops.x) * clamp(t, 0.0, 1.0);
}

/// Bent column `i` of a deck `half` cells wide a side, across it from its line (`bent`).
fn bent_side(half: i32, i: u32) -> i32 {
  if (half <= 2i) {
    return 0i;
  }
  if (half <= 4i) {
    return select(1i, -1i, i == 0u);
  }
  return i32(i) - 1i;
}

/// How many columns a bent of a deck `half` cells wide a side has (`bent`).
fn bent_cnt(half: i32) -> u32 {
  return select(select(3u, 2u, half <= 4i), 1u, half <= 2i);
}

/// Whether the column about `at` holds the pier at the station nearest `near` on
/// `[a, a + d)`, a cell a side (`pier_covers`); the stations off the brick's lines.
fn pier_covers(rec: u32, width: f32, a: vec2<f32>, d: vec2<f32>, near: vec2<f32>, at: vec2<f32>) -> bool {
  let k = select(1u, 0u, abs(d.x) >= abs(d.y));
  // The leaf cells the column spans a side, a cell's.
  let leaves = line_float(rec, HEADER_LEAF);
  if (d[k] == 0.0 || leaves > 2.0) {
    return false;
  }
  let half = i32(round(width * line_float(rec, HEADER_LEAF) / 2.0));
  let w = select(1u, 0u, half <= 2i);
  let step = line_float(rec, HEADER_STEP + w);
  let phase = line_float(rec, HEADER_PHASE + 2u * w + k);
  let station = round((phase + near[k]) / step) * step - phase;
  let t = (station - a[k]) / d[k];
  if (!(t >= 0.0 && t < 1.0)) {
    return false;
  }
  let len = span_length(d);
  let dir = vec2<f32>(d.x / len, d.y / len);
  let foot = vec2<f32>(a.x + d.x * t, a.y + d.y * t);
  // A leaf cell, and the centers of the leaf cells the column holds: its own where it is
  // one or finer, else its four.
  let leaf = 1.0 / line_float(rec, HEADER_LEAF);
  var off = 0.0;
  var m = 1u;
  if (leaves >= 2.0) {
    off = leaf / 2.0;
    m = 4u;
  }
  let n = bent_cnt(half);
  var covered = false;
  for (var j = 0u; j < m; j++) {
    let p = vec2<f32>(at.x + select(off, -off, (j & 1u) == 0u), at.y + select(off, -off, (j & 2u) == 0u));
    let rel = vec2<f32>(p.x - foot.x, p.y - foot.y);
    let along = abs(rel.x * dir.x + rel.y * dir.y);
    let across = rel.x * dir.y - rel.y * dir.x;
    // an edge column stands a leaf cell in from the edge
    for (var i = 0u; i < n; i++) {
      covered = covered || (along <= leaf / 2.0
        && abs(across - f32(bent_side(half, i) * max(half - 1i, 0i)) * leaf) <= leaf / 2.0);
    }
  }
  return covered;
}

/// A segment over one column: the lowest and highest its road surface runs across it, in
/// leaf cells, and whether its pier's section covers it (`Crossing`); `kind` `WAY_NOTHING`
/// for none. A visit's: where the next starts (`visit`).
struct Crossing {
  low: f32,
  top: f32,
  kind: u32,
  form: u32,
  pier: u32,
  clear: u32,
  next: u32,
}

/// What segment `s` is over the column about `at`, a cell a side (`crossing`, `on_way`).
fn crossing(rec: u32, s: vec4<u32>, at: vec2<f32>) -> Crossing {
  let a = seg_a(s);
  let d = seg_b(s) - a;
  let len2 = d.x * d.x + d.y * d.y;
  var t = 0.0;
  if (len2 > 0.0) {
    t = ((at.x - a.x) * d.x + (at.y - a.y) * d.y) / len2;
  }
  let cut = (seg_first(s) == 1u && t < 0.0) || (seg_last(s) == 1u && t > 1.0);
  t = clamp(t, 0.0, 1.0);
  let near = vec2<f32>(a.x + d.x * t, a.y + d.y * t);
  let width = seg_width(s);
  let dist = span_length(vec2<f32>(at.x - near.x, at.y - near.y));
  if (cut || !(dist <= max(width * 0.5, HALF_DIAGONAL))) {
    return Crossing(0.0, 0.0, WAY_NOTHING, 0u, 0u, 0u, 0u);
  }
  // the column's brick spans this much of the segment either side of `t`
  var half_span = 0.0;
  if (len2 > 0.0) {
    half_span = 1.0 / 2.0 * (abs(d.x) + abs(d.y)) / len2;
  }
  let z0 = surface(s, t - half_span);
  let z1 = surface(s, t + half_span);
  let form = seg_form(s);
  let pier = form != TUNNEL && pier_covers(rec, width, a, d, near, at);
  return Crossing(min(z0, z1), max(z0, z1), seg_kind(s), form, u32(pier), seg_clear(s), 0u);
}

/// The visit of a line over column `c` from its `k`-th segment there: the run of segments
/// each the next of its line and over the column, as one crossing, and where the next visit
/// starts (`visit`).
fn visit(rec: u32, c: vec2<i32>, k: u32) -> Crossing {
  let at = center(rec, c);
  let x = crossing(rec, segment(rec, c, k), at);
  if (x.kind == WAY_NOTHING) {
    return Crossing(x.low, x.top, x.kind, x.form, x.pier, x.clear, k + 1u);
  }
  var low = x.low;
  var top = x.top;
  var pier = x.pier;
  var j = k + 1u;
  let n = bin_of(rec, c).y;
  while (j < n && joins(rec, c, j)) {
    let y = crossing(rec, segment(rec, c, j), at);
    if (y.kind == WAY_NOTHING) {
      break;
    }
    low = min(low, y.low);
    top = max(top, y.top);
    pier = pier | y.pier;
    j += 1u;
  }
  return Crossing(low, top, x.kind, x.form, pier, x.clear, j);
}

/// `m` ground meters in leaf cells, `per_m` a meter (`cells_of_m`).
fn cells_of_m(m: f32, per_m: f32) -> i32 {
  return i32(round(m * per_m));
}

/// A deck's slab in leaf cells, at least one (`slab_depth`).
fn slab_depth(per_m: f32) -> i32 {
  return max(cells_of_m(DECK_THICKNESS_M, per_m), 1i);
}

const GRADE = 0u;
const OVER = 1u;
const UNDER = 2u;

/// What a crossing occupies of its column over ground `ground` leaf cells high
/// (`Occupancy`): `GRADE`, `OVER` or `UNDER` (`Occupy`), its span and its pier, each
/// `[lo, hi)`.
struct Occupancy {
  what: u32,
  span: vec2<i32>,
  pier: vec2<i32>,
}

/// `occupies`.
fn occupies(x: Crossing, ground: i32, per_m: f32) -> Occupancy {
  let low = i32(round(x.low));
  let top = i32(round(x.top));
  let none = vec2<i32>(0i);
  if (x.form == BRIDGE) {
    let lo = low - slab_depth(per_m);
    let pier = select(none, vec2<i32>(ground, lo), x.pier == 1u && lo > ground);
    return Occupancy(OVER, vec2<i32>(lo, top), pier);
  }
  if (x.form == TUNNEL) {
    return Occupancy(UNDER, vec2<i32>(low, top + i32(x.clear)), none);
  }
  // a way on the ground moves the ground with it: filled up to it, cut down to it
  if (top > ground) {
    return Occupancy(OVER, vec2<i32>(ground, top), none);
  }
  if (top < ground) {
    return Occupancy(UNDER, vec2<i32>(low, ground), none);
  }
  return Occupancy(GRADE, none, none);
}

fn transports(v: u32) -> bool {
  return v >= TRANSPORTS_FIRST && v <= TRANSPORTS_LAST;
}

/// The pier under a deck of column `c`, over its face `ground`, where one stands; none
/// where the deck is near the ground, the column a way's or no ground's, or a deck stands
/// under its own (`pier_stands`).
fn pier_stands(rec: u32, c: vec2<i32>, pier: vec2<i32>, ground: i32, per_m: f32) -> vec2<i32> {
  let none = vec2<i32>(0i);
  let p = palette_idx(rec, c);
  if (!grounds(p) || transports(p)) {
    return none;
  }
  let surface = pier.y + slab_depth(per_m);
  if (surface - pier.x <= cells_of_m(PIER_MIN_RISE_M, per_m)) {
    return none;
  }
  let gap = cells_of_m(LOWER_DECK_GAP_M, per_m);
  var k = 0u;
  while (k < bin_of(rec, c).y) {
    let x = visit(rec, c, k);
    k = x.next;
    if (x.kind != WAY_NOTHING) {
      let o = occupies(x, ground, per_m);
      if (o.what == OVER && o.span.y <= surface - gap) {
        return none;
      }
    }
  }
  return pier;
}

/// Column `c`'s spans into the scratch at `at` (`column`, fill.rs `keep`): what each visit
/// writes that fills a cell, a span each as it is made, the head word last.
fn column(rec: u32, c: vec2<i32>, at: u32) {
  if (palette_idx(rec, c) == VALUE_UNKNOWN || is_walled(rec, c)) {
    scratch[at] = head_word(FACE_NONE, 0u, true);
    return;
  }
  let cell = brick_cell(rec);
  let ground = face(rec, c);
  let per_m = line_float(rec, HEADER_PER_M);
  var n = 0u;
  var fits = true;
  var k = 0u;
  while (k < bin_of(rec, c).y) {
    let x = visit(rec, c, k);
    k = x.next;
    if (x.kind == WAY_NOTHING || !grounds(x.kind)) {
      continue;
    }
    let o = occupies(x, ground, per_m);
    if (o.what == GRADE) {
      continue;
    }
    var val = x.kind;
    var pier = o.pier;
    if (o.what == UNDER) {
      val = AIR_OF | x.kind;
    } else if (o.pier.x < o.pier.y) {
      pier = pier_stands(rec, c, o.pier, ground, per_m);
    }
    for (var w = 0u; w < 2u; w++) {
      let run = select(o.span, pier, w == 1u);
      if (fills_any(cell, run)) {
        if (n < SPANS) {
          let s = at + 1u + 2u * n;
          scratch[s] = bound_word(clamp(run, vec2<i32>(-32768i), vec2<i32>(32767i)));
          scratch[s + 1u] = select(val, VALUE_PIER, w == 1u);
          n += 1u;
        } else {
          fits = false;
        }
      }
    }
  }
  scratch[at] = head_word(top_of(cell, ground).cells - leaf_cells(cell), n, fits);
}

@compute @workgroup_size(SPANS_LANES)
fn way_cs(@builtin(global_invocation_id) id: vec3<u32>) {
  let i = id.x;
  if (i >= SPANS_COLUMNS) {
    return;
  }
  let rec = handed_at(id.z);
  let c = vec2<i32>(i32(i % SPANS_ACROSS) - 1i, i32(i / SPANS_ACROSS) - 1i);
  let at = scratch_at(arrayLength(&scratch), id.z) + SPANS_AT + i * SPANS_COLUMN_WORDS;
  // A corner of the ring, beside no column of the brick: none.
  if (all((c == vec2<i32>(-1i)) | (c == vec2<i32>(i32(BRICK))))) {
    scratch[at] = head_word(FACE_NONE, 0u, true);
    return;
  }
  column(rec, c, at);
}
