// pxl-render/decor_run.wgsl

// What a pass over the records reads, a workgroup an entry's or a run of placed decor's
// (tests/decor.rs `is_held`, `run_of`): the records laid, not the room.

/// `record_at`: record `n`'s first word, the entries' first, then placed decor's.
fn record_at(n: u32) -> u32 {
  if (n < SLOT_RECORDS) {
    return SLOTS_AT + n * RECORD_WORDS;
  }
  return PLACED_HEADER + PLACED_HEADER_WORDS + (n - SLOT_RECORDS) * RECORD_WORDS;
}

/// `is_held`: past the entries, placed decor's always; an entry's where the clip levels still
/// hold the brick they were generated for.
fn is_held(g: u32) -> bool {
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
    return vec2<u32>(SLOT_RECORDS + n, u32(n < records[PLACED_HEADER]));
  }
  let first = k * RUN;
  var j = 0u;
  while (j + 1u < DECORS && first >= decor_first_record(j + 1u)) {
    j += 1u;
  }
  let past = first - decor_first_record(j);
  let records_cnt = min(records[cnt_at(g, j)], decor_cap(j));
  let cnt = select(0u, records_cnt - past, records_cnt > past);
  return vec2<u32>(g * BRICK_RECORDS + decor_first_record(j) + past, min(cnt, RUN));
}

/// `record_of`'s fields of a record's first word.
fn record_decor(w: u32) -> u32 {
  return w & ((1u << VARIANT_SHIFT) - 1u);
}

fn record_variant(w: u32) -> u32 {
  return (w >> VARIANT_SHIFT) & ((1u << VARIANT_SHIFT) - 1u);
}

fn record_turn(w: u32) -> u32 {
  return (w >> TURN_SHIFT) & ((1u << TURN_BITS) - 1u);
}

fn record_cell_zoom(w: u32) -> u32 {
  return (w >> CELL_ZOOM_SHIFT) & ((1u << ZOOM_BITS) - 1u);
}
