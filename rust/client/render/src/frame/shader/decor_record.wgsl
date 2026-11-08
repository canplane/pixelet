// pxl-render/decor_record.wgsl

// Decor's records as the Rust lays them (pxl-content generation.rs): an entry's head and
// its records by decor, and a record's first word.

/// The word of entry `s`'s head counting the records decor `k` laid (`cnt_at`).
fn cnt_at(s: u32, k: u32) -> u32 {
  return s * HEADER_WORDS + CNT_AT + k;
}

/// The first word of entry `s`'s `i`-th record of decor `k` (`entry_record`, `record_at`).
fn entry_word(s: u32, k: u32, i: u32) -> u32 {
  return SLOTS_AT + (s * BRICK_RECORDS + decor_first_record(k) + i) * RECORD_WORDS;
}

/// `record_word`.
fn record_word(decor: u32, variant: u32, turn: u32, cell_zoom: u32) -> u32 {
  return decor | (variant << VARIANT_SHIFT) | (turn << TURN_SHIFT) | (cell_zoom << CELL_ZOOM_SHIFT);
}

/// `part_at`: where the `n`-th pair generated in part is listed, from the list's end.
fn part_at(n: u32) -> u32 {
  return LIST_WORDS - 1u - n;
}

/// `pair_word`, `pair_of`: a pair's word in the list, its entry and its decor.
fn pair_word(s: u32, j: u32) -> u32 {
  return s | (j << PAIR_DECOR_SHIFT);
}

fn pair_entry(w: u32) -> u32 {
  return w & ((1u << PAIR_DECOR_SHIFT) - 1u);
}

fn pair_decor(w: u32) -> u32 {
  return w >> PAIR_DECOR_SHIFT;
}

/// `generated_for`: whether entry `s`'s head says its records were generated for the brick `q`.
fn generated_for(s: u32, q: vec2<i32>) -> bool {
  let h = s * HEADER_WORDS;
  return records[h + 2u] == 1u && i32(records[h]) == q.x && i32(records[h + 1u]) == q.y;
}
