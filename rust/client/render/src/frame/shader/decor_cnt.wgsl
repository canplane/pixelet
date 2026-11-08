// pxl-render/decor_cnt.wgsl

// Picks the region each of the eye's records draws in this frame, writes it into the record's
// first word, and counts it. A workgroup takes an entry's records or a run of placed decor. A
// record draws nothing where the rays at its distance ask another zoom, held to the clip levels
// that hold its column (`generation::held_at`), than the one its record's clip level draws at
// (`generation::drawn_at`), or where it is the decor left out. Otherwise it is sized for the
// screen as a model, an impostor, or culled (`generation::drawn`).

@group(0) @binding(9) var<storage, read_write> records: array<u32>;
@group(0) @binding(13) var<storage, read> drawing: array<u32>;
@group(0) @binding(14) var<storage, read_write> regions: array<atomic<u32>>;

/// `generation::distance2`: the squared distance from `origin` to decor standing at `at`,
/// measured from the middle of its cell's base. X goes round the world as the steps wrap.
fn distance2(origin: vec3<i32>, at: vec3<i32>) -> f32 {
  let half = 1i << (FRAC_W - 1u);
  let steps = (at << vec3<u32>(FRAC_W)) + vec3<i32>(half, half, 0i) - origin;
  let d = vec3<f32>(steps) * (1.0 / f32(1u << FRAC_W));
  return d.x * d.x + d.y * d.y + d.z * d.z;
}

/// `generation::held_at`: the zoom asked, `ask_zoom`, held to what the clip levels hold at
/// leaf cell `c`.
fn held_at(ask_zoom: u32, c: vec2<i32>) -> u32 {
  let z = held_to(ask_zoom);
  let s = LEAF_CELL_ZOOM - z;
  let g = vec2<i32>(wrap_idx(z, c.x >> s), c.y >> s);
  return min(ask_zoom, lookup_from(step_to(LOOKUP_NONE, z, g).place));
}

/// `decor_cnt_cs` for the record at word `i`.
fn cnt_one(i: u32) {
  let w = records[i];
  if (w == UNGENERATED) {
    return;
  }
  let corner = vec3<i32>(i32(records[i + 1u]), i32(records[i + 2u]), i32(records[i + 3u]));
  let origin = vec3<i32>(i32(eye_word(ORIGIN)), i32(eye_word(ORIGIN + 1u)), i32(eye_word(ORIGIN + 2u)));
  let d2 = distance2(origin, corner);
  let placed = i >= PLACED_HEADER + PLACED_HEADER_WORDS;
  let ask_zoom = zoom_at(d2, vec2<u32>(eye_word(RANGE), eye_word(RANGE + 1u)));
  let zoom = select(held_at(ask_zoom, corner.xy), ask_zoom, placed);
  let cell_zoom = record_cell_zoom(w);
  var drawn_here = zoom == cell_zoom;
  if (placed) {
    drawn_here = zoom >= cell_zoom;
  } else if (cell_zoom >= LEAF_CELL_ZOOM) {
    drawn_here = zoom >= LEAF_CELL_ZOOM;
  }
  let group = decor_first_group(record_decor(w)) + record_variant(w);
  let hidden = drawing[DRAWN_HIDE + 3u] != 0u
    && i32(drawing[DRAWN_HIDE]) == corner.x
    && i32(drawing[DRAWN_HIDE + 1u]) == corner.y
    && i32(drawing[DRAWN_HIDE + 2u]) == corner.z;
  let across = bitcast<f32>(drawing[DRAWN_EXTENTS + group]) / max(sqrt(d2), NEAREST_U);
  var lod = DRAWN_CULLED;
  if (across >= bitcast<f32>(drawing[DRAWN_MODEL_ANGLE])) {
    lod = DRAWN_MODEL;
  } else if (across >= bitcast<f32>(drawing[DRAWN_IMPOSTOR_ANGLE])) {
    lod = DRAWN_IMPOSTOR;
  }
  let region = select(2u * group + lod, DRAWN_NONE, hidden || !drawn_here || lod == DRAWN_CULLED);
  records[i] = (w & ((1u << DRAWN_REGION_SHIFT) - 1u)) | (region << DRAWN_REGION_SHIFT);
  if (region != DRAWN_NONE) {
    atomicAdd(&regions[region], 1u);
  }
}

@compute @workgroup_size(LANES)
fn decor_cnt_cs(@builtin(workgroup_id) group: vec3<u32>, @builtin(local_invocation_id) lane: vec3<u32>) {
  if (!is_held(group.x)) {
    return;
  }
  for (var k = lane.x; k < RUNS; k += LANES) {
    let run = run_of(group.x, k);
    for (var n = 0u; n < run.y; n += 1u) {
      cnt_one(record_at(run.x + n));
    }
  }
}
