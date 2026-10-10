// pxl_shared/spec/cell/decor.rs
// [Layer: objects] User-placed decor kind catalog — cube.rs's own sibling, non-cube content (GLB instances, not a chunk_mesher.ts box).

// ============================================================================
// Type Definitions
// ============================================================================
//
// Numbered well clear of cube.rs's own values (128, 130-144, 255) even
// though the two never collide in practice (each placed leaf holds one
// value from exactly one of the two catalogs) — just so a raw byte's value
// alone still hints at which catalog it came from during debugging.
#[allow(non_upper_case_globals)]
pub mod decor_types {
    pub const tree: u8 = 228;

    pub const ENTRIES: &[(&str, u8)] = &[("tree", tree)];
}

pub type DecorId = &'static str;
