// pxl-render/handed.wgsl

// Where a handed brick's record and scratch begin (pxl-engine clipmap.rs `handed_at`, fill.rs
// `scratch_at`): what each of the ways' passes finds its brick by.

/// Where the `k`-th brick a frame hands begins in its fresh list (`clipmap::handed_at`).
fn handed_at(k: u32) -> u32 {
  return k * HANDED_WORDS;
}

/// Where the `k`-th handed brick's scratch begins in a scratch of `len` words, every
/// brick's a `FILL_CAP`th of it (fill.rs `scratch_at`).
fn scratch_at(len: u32, k: u32) -> u32 {
  return k * (len / FILL_CAP);
}
