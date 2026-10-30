// pxl-spec/bake/object.spec.rs
//
// GENERATED from spec/spec/bake/object.json

/// How many sizes an object places at: a cell and its halvings, coarsest
/// first. Four, so the 0.125u tier kept working when the cell grew from 0.5u
/// to 1u. The shell names a size by its tier, never by a length.
pub const OBJECT_TIERS: usize = 4;
