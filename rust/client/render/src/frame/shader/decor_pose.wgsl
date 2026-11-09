// pxl-render/decor_pose.wgsl

// Generates the decor of each record of a listed pair, one thread a record: its footing, then
// its pose (pxl-content `generation::placement`). Decor stands on the world with nothing
// placed, and nothing placed stays nothing down a descent (`Inheritance::UNPLACED`), so no line
// of the world is kept. A column's surface is read from the column (`surface`, which answers
// as `support::column_surface` does), and only a column under ways is walked cell by cell. The
// loop asks the world from one place (`world_cell`), as a GPU inlines a call at every place it
// is called from, and a thread then holds across the ask only what the loop reads after it. The
// pass keeps no stack, no array and no struct in a `var`.
//
// The footing and the pose are one pass, as neither holds a walk.
//
// A thread writes a record's first word, and its box's height and sink. The eye's words are
// declared in eye_bind.wgsl.

@group(0) @binding(9) var<storage, read_write> records: array<u32>;
@group(0) @binding(11) var<storage, read> list: array<u32>;

/// The values a `Surface`'s `kind` takes.
const SURFACE_NONE = 0u;
const SURFACE_MET = 1u;
const SURFACE_WALK = 2u;

/// What a column holds of its surface (`support::column_surface`): none; a surface met at `z`,
/// whose cell keeps `eighths` of its cube; or a surface to walk for from `z` down to `end`.
/// `end` is the ways' lowest where nothing but ways fills the column, and otherwise the
/// world's floor, as far as the ray down goes.
struct Surface {
  kind: u32,
  z: i32,
  eighths: u32,
  end: i32,
}

/// The surface of column `g` at `zoom`, as `cast::walk` down the column finds it, as far as
/// the column's own words tell.
fn surface(zoom: u32, g: vec2<i32>) -> Surface {
  let shard = shard_at(SHARD_UNKNOWN, zoom, g);
  // Past the service area lies its earth, on which no decor stands.
  if (is_past(shard)) {
    return Surface(SURFACE_NONE, 0i, 0u, 0i);
  }
  // A listed pair's brick is held, so the columns of its leaves are held too, at this zoom
  // and at the finest.
  let r = bound_over(LOOKUP_NONE, zoom, zoom, zoom, g);
  let col = ground_column(shard, r.place, zoom, g);
  let s = LEAF_CELL_ZOOM - zoom;
  if (r.b.z < r.b.w) {
    let floor = -(1i << (HEIGHT_BITS - 1u - s));
    let z = max((r.b.w - 1i) >> s, col.top - 1i);
    // A column at `zoom` is `1 << (HEIGHT_BITS - s)` cells tall from its floor up, so a walk
    // from a cell in it reaches the floor within that many cells. A longer walk would start
    // outside the world, and it is cut there, as a walk past its end is.
    let end = max(select(floor, r.b.z >> s, col.palette_idx == VAL_VOID), z - ((1i << (HEIGHT_BITS - s)) - 1i));
    return Surface(SURFACE_WALK, z, 0u, end);
  }
  if (col.palette_idx == VAL_VOID) {
    return Surface(SURFACE_NONE, 0i, 0u, 0i);
  }
  return Surface(SURFACE_MET, col.top - 1i, eighths(col.part), 0i);
}

/// `generation::overlaps_placed` for the box from `at`, `size` a side: whether a placed decor's
/// box meets it. Each placed unit leaf near enough is asked in turn. The leaves are found by one
/// descent, from each node at `PLACED_FIRST`, of the branches that meet the box
/// (`world::placed_in`), each child stepped once from its parent over the three levels to
/// `UNIT_ZOOM` (build.rs asserts there are three). A node at `PLACED_FIRST` that holds nothing
/// holds no leaf.
fn overlaps_placed(at: vec3<i32>, size: vec3<i32>) -> bool {
  let lo = at - CLAIM_EXTENT;
  let hi = at + size - 1i;
  let t = vec3<u32>(UNIT_ZOOM - PLACED_FIRST);
  let first = lo >> t;
  let last = hi >> t;
  // The box spans `hi - lo + 1` cells, so it meets at most `((hi - lo) >> t) + 2` nodes on an
  // axis. Each loop is bounded by that count as well as by `last`, so it ends however near the
  // integers' ends the box lies.
  let nodes = ((hi - lo) >> t) + 1i;
  for (var nz = first.z; nz <= last.z && nz - first.z <= nodes.z; nz += 1i) {
    for (var ny = first.y; ny <= last.y && ny - first.y <= nodes.y; ny += 1i) {
      for (var nx = first.x; nx <= last.x && nx - first.x <= nodes.x; nx += 1i) {
        let node = vec3<i32>(nx, ny, nz);
        let slot = world_root(PLACED_FIRST, node).x;
        if (!is_folded(slot)) {
          continue;
        }
        for (var o1 = 0u; o1 < 8u; o1 += 1u) {
          let c1 = child_at(node, o1);
          let s1 = placed_child(slot, PLACED_FIRST + 1u, c1, lo, hi);
          if (!is_folded(s1)) {
            continue;
          }
          for (var o2 = 0u; o2 < 8u; o2 += 1u) {
            let c2 = child_at(c1, o2);
            let s2 = placed_child(s1, PLACED_FIRST + 2u, c2, lo, hi);
            if (!is_folded(s2)) {
              continue;
            }
            for (var o3 = 0u; o3 < 8u; o3 += 1u) {
              let c = child_at(c2, o3);
              if (leaf_overlaps(placed_child(s2, UNIT_ZOOM, c, lo, hi), c, at, size)) {
                return true;
              }
            }
          }
        }
      }
    }
  }
  return false;
}

/// Child `o` of the node at `p`, whose bits from the lowest give X, Y and Z (`object::child`'s
/// order).
fn child_at(p: vec3<i32>, o: u32) -> vec3<i32> {
  return 2i * p + vec3<i32>(vec3<u32>(o & 1u, (o >> 1u) & 1u, (o >> 2u) & 1u));
}

/// The slot of node `c` at `zoom` under the folded slot `parent` where the node meets the
/// cells from `lo` to `hi`, and otherwise `SLOT_NOTHING`.
fn placed_child(parent: u32, zoom: u32, c: vec3<i32>, lo: vec3<i32>, hi: vec3<i32>) -> u32 {
  let t = vec3<u32>(UNIT_ZOOM - zoom);
  if (any(c < (lo >> t)) || any(c > (hi >> t))) {
    return SLOT_NOTHING;
  }
  return tree_child(PLACED, parent, zoom, c);
}

/// Whether unit cell `c`, whose slot is `s`, holds a placed decor whose box, posed as placed
/// (`placed_pose`), meets the box from `at`.
fn leaf_overlaps(s: u32, c: vec3<i32>, at: vec3<i32>, size: vec3<i32>) -> bool {
  if (s == SLOT_NOTHING || is_folded(s) || (s & SLOT_ZOOM_MASK) != UNIT_ZOOM) {
    return false;
  }
  let k = (tree_data(PLACED, s >> SLOT_SHIFT) & 0xffffu) - FIRST_VAL;
  if (k >= DECORS) {
    return false;
  }
  let h = node_seed(PLACED_TAG, UNIT_ZOOM, c);
  let variant = mul_hi(draw(h, 0u), row_word(k, ROW_VARIANTS));
  let box_size = cells(k, variant, draw(h, 1u) >> (32u - TURN_BITS));
  return all(c < at + size) && all(at < c + box_size);
}

fn list_word(k: u32) -> u32 {
  return list[k];
}

/// A cut as the eye holds it: its index, then its zoom.
fn eye_cut(k: u32) -> vec4<i32> {
  return vec4<i32>(i32(eye_word(k)), i32(eye_word(k + 1u)), i32(eye_word(k + 2u)), i32(eye_word(k + 3u)));
}

/// Where a record's loop stands: reading a column, walking it down, having met its surface, or
/// posing.
const STAGE_COLUMN = 0u;
const STAGE_WALK = 1u;
const STAGE_MET = 2u;
const STAGE_POSE = 3u;

/// Finds the footing and the pose of record `id.x` of a listed pair. `id.z` names the pair: the
/// pairs generated whole come first, then those in part (`generation::part_at`), of which only
/// a record that a box reaches is asked.
@compute @workgroup_size(LANES)
fn decor_cs(@builtin(global_invocation_id) id: vec3<u32>) {
  // How many of the pairs the list holds are generated whole (`generation::LIST_WORDS`).
  let whole = in_cap(list[THIN_ARGS + 2u], LIST_WORDS - LIST_AT);
  let part = id.z >= whole;
  let pair = list[select(LIST_AT + id.z, part_at(id.z - whole), part)];
  let brick = pair_entry(pair);
  let j = pair_decor(pair);
  if (id.x >= in_cap(records[cnt_at(brick, j)], decor_cap(j))) {
    return;
  }
  let r = entry_word(brick, j, id.x);
  let cell_zoom = FIRST_CELL_ZOOM + brick / CLIP_ENTRIES;
  let leaf = vec2<i32>(i32(records[r + 1u]), i32(records[r + 2u]));
  if (part && !is_changed(cell_zoom, vec4<i32>(leaf, leaf + 1i))) {
    return;
  }
  // The finest clip level that holds the record's leaf column (`DecorQuery::finest`). It is
  // the record's own clip level or finer, as a pair is listed only where its brick's row is
  // held (decor_mark.wgsl).
  let zoom = lookup_from(step_to(LOOKUP_NONE, LEAF_CELL_ZOOM, leaf).place);
  let s = LEAF_CELL_ZOOM - zoom;
  // The column surface asked, first at the record's clip level and then at the finest. The
  // pose is taken about the last one asked.
  var stage = STAGE_COLUMN;
  var column_zoom = cell_zoom;
  var z = 0i;
  var z_end = 0i;
  var eighths = 0u;
  // The pose: the seed's draws, the votes along transport, and the box's cells a side.
  var variant = 0u;
  var turn = 0u;
  var flip = 0u;
  var voting = 0i;
  var horizontal = 0i;
  var vertical = 0i;
  var n = vec3<i32>(0i);
  var base = 0i;
  // The ask: which cell of the vote or of the box is asked, and whether that cell is asked
  // again of the world as edited.
  var ask = 0i;
  var edited = false;
  loop {
    if (stage == STAGE_COLUMN) {
      let f = surface(column_zoom, vec2<i32>(wrap_idx(column_zoom, leaf.x >> (LEAF_CELL_ZOOM - column_zoom)), leaf.y >> (LEAF_CELL_ZOOM - column_zoom)));
      if (f.kind == SURFACE_NONE) {
        records[r] = UNGENERATED;
        return;
      }
      z = f.z;
      z_end = f.end;
      eighths = f.eighths;
      stage = select(STAGE_WALK, STAGE_MET, f.kind == SURFACE_MET);
    }
    if (stage == STAGE_MET) {
      if (column_zoom == cell_zoom) {
        let q = LEAF_CELL_ZOOM - cell_zoom;
        records[r + 3u] = u32((z + 1i) * (1i << q));
        records[r + 4u] = u32(sink_share(eighths) * f32(1u << (q + FRAC_W)) + 0.5);
      }
      if (column_zoom != zoom) {
        column_zoom = zoom;
        stage = STAGE_COLUMN;
        continue;
      }
      // The seed is the leaf cell's again, and its draws are taken in order
      // (`generation::asks`).
      base = z;
      let seed = draw(quad_seed(row_word(j, ROW_SEED), LEAF_CELL_ZOOM, leaf), 1u);
      variant = mul_hi(draw(seed, 0u), row_word(j, ROW_VARIANTS));
      if (row_word(j, ROW_ORIENT) == ALONG_TRANSPORT) {
        flip = draw(seed, 1u) >> 31u;
        voting = 4i * ALIGN_NEIGHBOR_RADIUS;
      } else {
        turn = draw(seed, 1u) >> (32u - TURN_BITS);
      }
      stage = STAGE_POSE;
    }
    var c = vec3<i32>(0i);
    var put = SLOT_NOTHING;
    var carve = NO_CUT;
    var hide = NO_CUT;
    var ask_zoom = zoom;
    let foot = vec3<i32>(leaf >> vec2<u32>(s), base);
    let voted = ask >= voting;
    let e = ask - voting * i32(voted);
    if (stage == STAGE_WALK) {
      ask_zoom = column_zoom;
      c = vec3<i32>(wrap_idx(column_zoom, leaf.x >> (LEAF_CELL_ZOOM - column_zoom)), leaf.y >> (LEAF_CELL_ZOOM - column_zoom), z);
    } else if (!voted) {
      let d = ask / 4i + 1i;
      let along = select(d, -d, (ask & 1i) == 0i);
      c = foot + select(vec3<i32>(0i, along, 0i), vec3<i32>(along, 0i, 0i), (ask & 2i) == 0i);
    } else {
      if (e == 0i && !edited) {
        // The vote is in. It gives the turn, and the turn gives the box's cells a side.
        if (voting > 0i) {
          turn = u32(horizontal > vertical) + 2u * flip;
        }
        let size = cells(j, variant, turn);
        let side = 1i << s;
        n = (vec3<i32>(leaf + size.xy - 1i, (base + 1i) * side + size.z - 1i) >> vec3<u32>(s)) - foot + 1i;
      }
      if (e >= n.x * n.y * n.z) {
        break;
      }
      c = foot + vec3<i32>((e / n.z) % n.x, e / (n.z * n.x), e % n.z);
      if (edited) {
        // Where nothing is placed or cut, the cell holds as the world unedited answered it,
        // and no ask is made.
        carve = eye_cut(CARVE);
        hide = eye_cut(HIDE);
        put = world_root(zoom, c).x;
        if (!cut_holds(carve, zoom, c) && tree_val(PLACED, put, hide, zoom, c) == VAL_VOID) {
          edited = false;
          ask += 1i;
          continue;
        }
      }
    }
    let v = world_cell(vec4<u32>(put, SHARD_UNKNOWN, LOOKUP_NONE, SLOT_NOTHING), shard_at(SHARD_UNKNOWN, ask_zoom, c.xy), ask_zoom, c, carve, hide).asset.x;
    if (stage == STAGE_WALK) {
      if (fills(v)) {
        // The eighths of its cube that the cell keeps, with 0 for all eight, which
        // `sink_share` reads as eight.
        eighths = v >> FILL_SHIFT;
        stage = STAGE_MET;
      } else {
        z -= 1i;
        if (z < z_end) {
          records[r] = UNGENERATED;
          return;
        }
      }
      continue;
    }
    if (edited) {
      if ((v & PLACED_ASSET) != 0u) {
        records[r] = UNGENERATED;
        return;
      }
      edited = false;
    } else if (voted) {
      // Under the base, the cell must be a palette entry the decor generates on. Over it,
      // the cell must be void and hold no edit.
      let under = c.z == base;
      if (!(select(v == VAL_VOID, allows(j, v & VAL_MASK), under))) {
        records[r] = UNGENERATED;
        return;
      }
      if (!under) {
        edited = true;
        continue;
      }
    } else if ((v & VAL_MASK) >= TRANSPORTS_FIRST && (v & VAL_MASK) <= TRANSPORTS_LAST) {
      if ((ask & 2i) == 0i) {
        horizontal += 1i;
      } else {
        vertical += 1i;
      }
    }
    ask += 1i;
  }
  let at_z = i32(records[r + 3u]);
  if (overlaps_placed(vec3<i32>(leaf, at_z), cells(j, variant, turn))) {
    records[r] = UNGENERATED;
  } else {
    records[r] = record_word(j, variant, turn, cell_zoom);
  }
}
