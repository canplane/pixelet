// pxl-render/decor_cnt.wgsl

// Each record for the eye this frame, a workgroup an entry's or a run of placed decor's, its
// region written into its first word and counted: nothing where the rays ask another zoom at
// its distance, held to the clip levels holding its column (`generation::held_at`), than the
// one its record's clip level draws at (`generation::drawn_at`), or where it is the decor left
// out; else sized for the screen (`generation::drawn`), a model, an impostor or culled.

@group(0) @binding(9) var<storage, read_write> records: array<u32>;
@group(0) @binding(13) var<storage, read> drawing: array<u32>;
@group(0) @binding(14) var<storage, read_write> regions: array<atomic<u32>>;

/// `generation::distance2`: how far decor standing at `at` lies from `origin`, squared, from
/// its cell's middle at its base, X round the world as the steps wrap.
fn distance2(origin: vec3<i32>, at: vec3<i32>) -> f32 {
  let half = 1i << (FRAC_W - 1u);
  let steps = (at << vec3<u32>(FRAC_W)) + vec3<i32>(half, half, 0i) - origin;
  let d = vec3<f32>(steps) * (1.0 / f32(1u << FRAC_W));
  return d.x * d.x + d.y * d.y + d.z * d.z;
}

/// `generation::held_at`: the zoom asked, `ask_zoom`, that the clip levels hold at leaf cell `c`.
fn held_at(ask_zoom: u32, c: vec2<i32>) -> u32 {
  let z = held_to(ask_zoom);
  let s = LEAF_CELL - z;
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
  let placed = i >= PLACED_HEAD + PLACED_HEAD_WORDS;
  let ask_zoom = zoom_at(d2, 0u, vec2<u32>(eye_word(RANGE), eye_word(RANGE + 1u)));
  let zoom = select(held_at(ask_zoom, corner.xy), ask_zoom, placed);
  let cell = record_cell(w);
  var drawn_here = zoom == cell;
  if (placed) {
    drawn_here = zoom >= cell;
  } else if (cell >= LEAF_CELL) {
    drawn_here = zoom >= LEAF_CELL;
  }
  let group = decor_first_group(record_decor(w)) + record_variant(w);
  let hidden = drawing[DRAWN_HIDE + 3u] != 0u
    && i32(drawing[DRAWN_HIDE]) == corner.x
    && i32(drawing[DRAWN_HIDE + 1u]) == corner.y
    && i32(drawing[DRAWN_HIDE + 2u]) == corner.z;
  let across = bitcast<f32>(drawing[DRAWN_EXTENTS + group]) / max(sqrt(d2), NEAREST_U);
  var tier = DRAWN_CULLED;
  if (across >= bitcast<f32>(drawing[DRAWN_MODEL_ANGLE])) {
    tier = DRAWN_MODEL;
  } else if (across >= bitcast<f32>(drawing[DRAWN_IMPOSTOR_ANGLE])) {
    tier = DRAWN_IMPOSTOR;
  }
  let region = select(2u * group + tier, DRAWN_NONE, hidden || !drawn_here || tier == DRAWN_CULLED);
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
