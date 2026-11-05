// pxl-render/decor_pose.wgsl

// The decor each record of a listed pair generates, a thread a record: its footing, then
// its pose (pxl-content `generation::placement`), no line of the world kept, as what is
// handed the world unplaced is nothing (`Inheritance::UNPLACED`). A column's surface is read
// off the column (`surface`, the answer of `support::column_surface`); only one under ways
// is walked cell by cell. One ask of the world, in the loop (`world_cell`), so a thread
// holds what the loop reads after and no more. No stack, no array, no struct of `var`.
//
// Footing and pose are one pass, as neither holds a walk.
//
// Of a record a thread writes its first word, and its box's height and sink. The eye's
// words are eye_bind.wgsl's.

@group(0) @binding(9) var<storage, read_write> records: array<u32>;
@group(0) @binding(11) var<storage, read> list: array<u32>;

/// What a column's surface is.
const SURFACE_NONE = 0u;
const SURFACE_MET = 1u;
const SURFACE_WALK = 2u;

/// What a column holds of its surface: none, met at `z` keeping `eighths` of its cube, or
/// to be walked from `z` down to `end`: the ways' lowest where nothing but ways fills it,
/// else the world's floor, as far as the ray down goes (`support::column_surface`).
struct Surface {
  kind: u32,
  z: i32,
  eighths: u32,
  end: i32,
}

/// `cast::walk` down the column `g` at `zoom`, as far as a column tells.
fn surface(zoom: u32, g: vec2<i32>) -> Surface {
  let shard = shard_at(SHARD_UNKNOWN, zoom, g);
  // Past the area its earth, on which no decor stands.
  if (is_past(shard)) {
    return Surface(SURFACE_NONE, 0i, 0u, 0i);
  }
  // A listed pair's brick is held, so its leaves' columns are, here and at the finest.
  let r = bound_over(LOOKUP_NONE, zoom, zoom, zoom, g);
  let col = ground_column(shard, r.place, zoom, g);
  let s = LEAF_CELL - zoom;
  if (r.b.z < r.b.w) {
    let floor = -(1i << (HEIGHT_BITS - 1u - s));
    let end = select(floor, r.b.z >> s, col.palette_idx == VALUE_VOID);
    return Surface(SURFACE_WALK, max((r.b.w - 1i) >> s, col.top - 1i), 0u, end);
  }
  if (col.palette_idx == VALUE_VOID) {
    return Surface(SURFACE_NONE, 0i, 0u, 0i);
  }
  return Surface(SURFACE_MET, col.top - 1i, eighths(col.part), 0i);
}

/// `generation::overlaps_placed` of the box from `at`, `size` a side: a placed decor's box
/// meets it, each placed unit leaf near enough asked in turn. A node of `PLACED_FIRST`
/// holding nothing holds no leaf.
fn overlaps_placed(at: vec3<i32>, size: vec3<i32>) -> bool {
  let lo = at - CLAIM_EXTENT;
  let hi = at + size - 1i;
  let t = vec3<u32>(UNIT_ZOOM - PLACED_FIRST);
  let first = lo >> t;
  let last = hi >> t;
  for (var nz = first.z; nz <= last.z; nz += 1i) {
    for (var ny = first.y; ny <= last.y; ny += 1i) {
      for (var nx = first.x; nx <= last.x; nx += 1i) {
        let node = vec3<i32>(nx, ny, nz);
        let slot = world_root(PLACED_FIRST, node).x;
        if (slot == SLOT_NOTHING) {
          continue;
        }
        let start = max(lo, node << t);
        let stop = min(hi, ((node + 1i) << t) - 1i);
        for (var z = start.z; z <= stop.z; z += 1i) {
          for (var y = start.y; y <= stop.y; y += 1i) {
            for (var x = start.x; x <= stop.x; x += 1i) {
              if (leaf_overlaps(slot, vec3<i32>(x, y, z), at, size)) {
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

/// Whether the unit cell `c` under the node whose slot is `slot` holds a placed decor whose
/// box, posed as placed (`placed_pose`), meets the box from `at`.
fn leaf_overlaps(slot: u32, c: vec3<i32>, at: vec3<i32>, size: vec3<i32>) -> bool {
  var s = slot;
  var q = PLACED_FIRST;
  while (q < UNIT_ZOOM && is_folded(s)) {
    q += 1u;
    s = tree_child(PLACED, s, q, c >> vec3<u32>(UNIT_ZOOM - q));
  }
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

/// Where a record's loop is: reading a column, walking it down, its surface met, posing.
const STAGE_COLUMN = 0u;
const STAGE_WALK = 1u;
const STAGE_MET = 2u;
const STAGE_POSE = 3u;

/// The footing and pose of a listed pair's record `id.x`: `id.z` the pair, those generated
/// whole first, then those in part (`generation::part_at`), of which only a record a box
/// reaches is asked.
@compute @workgroup_size(LANES)
fn decor_cs(@builtin(global_invocation_id) id: vec3<u32>) {
  let whole = list[THIN_ARGS + 2u];
  let part = id.z >= whole;
  let pair = list[select(LIST_AT + id.z, part_at(id.z - whole), part)];
  let brick = pair_entry(pair);
  let j = pair_decor(pair);
  if (id.x >= min(records[cnt_at(brick, j)], decor_cap(j))) {
    return;
  }
  let r = entry_word(brick, j, id.x);
  let cell = FIRST_CELL + brick / CLIP_ENTRIES;
  let leaf = vec2<i32>(i32(records[r + 1u]), i32(records[r + 2u]));
  if (part && !is_changed(cell, vec4<i32>(leaf, leaf + 1i))) {
    return;
  }
  // The finest clip level holding its leaf column, never coarser than its own
  // (`DecorQuery::finest`).
  let found = step_to(LOOKUP_NONE, LEAF_CELL, leaf).place;
  let zoom = select(cell, lookup_from(found), lookup_here(found) && lookup_from(found) > cell);
  let s = LEAF_CELL - zoom;
  // The surface asked, its clip level's and then the finest's; the last its pose's.
  var stage = STAGE_COLUMN;
  var column_zoom = cell;
  var z = 0i;
  var z_end = 0i;
  var eighths = 0u;
  // The pose: its draws, the votes along transport, the box's cells a side.
  var variant = 0u;
  var turn = 0u;
  var flip = 0u;
  var voting = 0i;
  var horizontal = 0i;
  var vertical = 0i;
  var n = vec3<i32>(0i);
  var base = 0i;
  // The ask: the vote's or the box's cell, and whether this one is asked as edited.
  var ask = 0i;
  var edited = false;
  loop {
    if (stage == STAGE_COLUMN) {
      let f = surface(column_zoom, vec2<i32>(wrap_idx(column_zoom, leaf.x >> (LEAF_CELL - column_zoom)), leaf.y >> (LEAF_CELL - column_zoom)));
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
      if (column_zoom == cell) {
        let q = LEAF_CELL - cell;
        records[r + 3u] = u32((z + 1i) * (1i << q));
        records[r + 4u] = u32(sink_share(eighths) * f32(1u << (q + FRAC_W)) + 0.5);
      }
      if (column_zoom != zoom) {
        column_zoom = zoom;
        stage = STAGE_COLUMN;
        continue;
      }
      // Its seed the leaf cell's again; its draws in order (`generation::asks`).
      base = z;
      let seed = draw(quad_seed(row_word(j, ROW_SEED), LEAF_CELL, leaf), 1u);
      variant = mul_hi(draw(seed, 0u), row_word(j, ROW_VARIANTS));
      if (row_word(j, ROW_ORIENTATION) == ALONG_TRANSPORT) {
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
      c = vec3<i32>(wrap_idx(column_zoom, leaf.x >> (LEAF_CELL - column_zoom)), leaf.y >> (LEAF_CELL - column_zoom), z);
    } else if (!voted) {
      let d = ask / 4i + 1i;
      let along = select(d, -d, (ask & 1i) == 0i);
      c = foot + select(vec3<i32>(0i, along, 0i), vec3<i32>(along, 0i, 0i), (ask & 2i) == 0i);
    } else {
      if (e == 0i && !edited) {
        // The vote is in: the turn it gives, then the box it turns.
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
        // Nothing placed nor cut here: it holds, no ask made.
        carve = eye_cut(CARVE);
        hide = eye_cut(HIDE);
        put = world_root(zoom, c).x;
        if (!cut_holds(carve, zoom, c) && tree_val(PLACED, put, hide, zoom, c) == VALUE_VOID) {
          edited = false;
          ask += 1i;
          continue;
        }
      }
    }
    let v = world_cell(vec4<u32>(put, SHARD_UNKNOWN, LOOKUP_NONE, SLOT_NOTHING), shard_at(SHARD_UNKNOWN, ask_zoom, c.xy), ask_zoom, c, carve, hide).asset.x;
    if (stage == STAGE_WALK) {
      if (fills(v)) {
        eighths = fill_eighths(v);
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
      // Under its base a palette entry it generates on; over it void, and no edit.
      let under = c.z == base;
      if (!(select(v == VALUE_VOID, allows(j, v & VAL_MASK), under))) {
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
    records[r] = record_word(j, variant, turn, cell);
  }
}

/// `answer::fill_eighths`.
fn fill_eighths(asset: u32) -> u32 {
  let k = (asset >> FILL_SHIFT) & (EIGHTHS - 1u);
  return select(k, EIGHTHS, k == 0u);
}
