// pxl-render/decor_record.wgsl

// Where decor's records lie, as pxl-content generation.rs lays them: an entry's header, its
// records by decor, and a record's first word.

/// The word of entry `s`'s header that counts the records decor `k` laid (`cnt_at`).
fn cnt_at(s: u32, k: u32) -> u32 {
  return s * HEADER_WORDS + CNT_AT + k;
}

/// The first word of entry `s`'s `i`-th record of decor `k` (`entry_rec`, `rec_at`).
fn entry_word(s: u32, k: u32, i: u32) -> u32 {
  return ENTRY_RECS_AT + (s * BRICK_REC_CAP + decor_first_rec(k) + i) * REC_WORDS;
}

/// A record's first word, as pxl-content `generation::rec_word` packs it.
fn rec_word(decor: u32, variant: u32, turn: u32, cell_zoom: u32) -> u32 {
  return decor | (variant << VARIANT_SHIFT) | (turn << TURN_SHIFT) | (cell_zoom << CELL_ZOOM_SHIFT);
}

/// `part_at`: where the `n`-th pair generated in part is listed, from the list's end.
fn part_at(n: u32) -> u32 {
  return LIST_WORDS - 1u - n;
}

/// `pair_word`, `pair_of`: a pair's word in the list, which holds its entry in the low bits and
/// its decor above them.
fn pair_word(s: u32, j: u32) -> u32 {
  return s | (j << PAIR_DECOR_SHIFT);
}

fn pair_entry(w: u32) -> u32 {
  return w & ((1u << PAIR_DECOR_SHIFT) - 1u);
}

fn pair_decor(w: u32) -> u32 {
  return w >> PAIR_DECOR_SHIFT;
}

/// `generated_for`: whether entry `s`'s header says its records were generated for the brick
/// `q`.
fn generated_for(s: u32, q: vec2<i32>) -> bool {
  let h = s * HEADER_WORDS;
  return recs[h + 2u] == 1u && i32(recs[h]) == q.x && i32(recs[h + 1u]) == q.y;
}
