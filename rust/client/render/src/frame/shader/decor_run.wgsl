// pxl-render/decor_run.wgsl

// What a pass over the records reads, one workgroup an entry's records or a run of placed
// decor (tests/decor.rs `holds_recs`, `run_of`). It reads the records laid, not the room for them.

/// `rec_at`: the first word of record `n`. The entries' records are numbered first, then
/// placed decor's.
fn rec_at(n: u32) -> u32 {
  if (n < ENTRY_RECS) {
    return ENTRY_RECS_AT + n * REC_WORDS;
  }
  return PLACED_HEADER + PLACED_HEADER_WORDS + (n - ENTRY_RECS) * REC_WORDS;
}

/// `holds_recs`: whether workgroup `g`'s records still hold. Placed decor's, past the entries,
/// always do; an entry's hold while the clip levels still hold the brick they were generated
/// for.
fn holds_recs(g: u32) -> bool {
  if (g >= EYE_ENTRIES) {
    return true;
  }
  let q = clipmap[g];
  return q.z != 0i && generated_for(g, q.xy);
}

/// `run_of`: the `k`-th run of workgroup `g`, as record numbers: the first and how many.
fn run_of(g: u32, k: u32) -> vec2<u32> {
  if (g >= EYE_ENTRIES) {
    let n = (g - EYE_ENTRIES) * RUNS + k;
    return vec2<u32>(ENTRY_RECS + n, u32(n < recs[PLACED_HEADER]));
  }
  let first = k * RUN;
  var j = 0u;
  while (j + 1u < DECORS && first >= decor_first_rec(j + 1u)) {
    j += 1u;
  }
  let past = first - decor_first_rec(j);
  let rec_cnt = recs[cnt_at(g, j)];
  let cnt = select(0u, rec_cnt - past, rec_cnt > past);
  return vec2<u32>(g * BRICK_REC_CAP + decor_first_rec(j) + past, min(cnt, RUN));
}

// The fields of a record's first word, as decor_record.wgsl `rec_word` packs them.
fn rec_decor(w: u32) -> u32 {
  return w & ((1u << VARIANT_SHIFT) - 1u);
}

fn rec_variant(w: u32) -> u32 {
  return (w >> VARIANT_SHIFT) & ((1u << VARIANT_SHIFT) - 1u);
}

fn rec_turn(w: u32) -> u32 {
  return (w >> TURN_SHIFT) & ((1u << TURN_BITS) - 1u);
}

fn rec_cell_zoom(w: u32) -> u32 {
  return (w >> CELL_ZOOM_SHIFT) & ((1u << ZOOM_BITS) - 1u);
}
