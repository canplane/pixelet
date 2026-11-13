// pxl-spec/asset/symbol.spec.rs
//
// GENERATED from asset/raw/place/symbol/ (xpute-gen data.rs)
//
// Carto's place symbols (openstreetmap-carto `symbols/`, CC0), symbol n the (n - 1)th in
// spec/spec/asset/ingest.json's `symbols` order: in Rust each a mask of its coverage, a
// byte a pixel, `SYMBOL_PX` a side, row by row, in `symbol.mask`; in TypeScript its
// outline at `PLACE_SYMBOLS[n - 1]` (place_theme.spec.ts `SYMBOL`), its box and each
// shape's path data and whether it fills even-odd.

// Names as the spec spells them, so one record has one name on every side.
// A table is generated whole, whether or not this crate calls an entry.
#![allow(
    non_upper_case_globals,
    non_snake_case,
    dead_code,
    reason = "generated from the spec"
)]

/// A mask's side, in device pixels.
pub const SYMBOL_PX: u32 = 28;

/// The device pixels a pixel the masks are rasterized at.
pub const SYMBOL_RATIO: u32 = 2;

/// How many symbols there are.
pub const SYMBOL_CNT: usize = 68;

pub static SYMBOL_MASKS: &[u8] = include_bytes!("symbol.mask");
