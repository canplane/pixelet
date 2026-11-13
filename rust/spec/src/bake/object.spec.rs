// pxl-spec/bake/object.spec.rs
//
// GENERATED from spec/spec/bake/object.json (xpute-gen data.rs)
//
// The objects a player places, as the bake and the UI count them.

// Names as the spec spells them, so one record has one name on every side.
// A table is generated whole, whether or not this crate calls an entry.
#![allow(
    non_upper_case_globals,
    non_snake_case,
    dead_code,
    reason = "generated from the spec"
)]

/// How many scales an object places at: a unit and its halvings, as a decor's
/// `scale_steps`, 0, -1, -2 and -3. There are four so that the smallest is 0.125u, an
/// eighth of the 1u cell. The UI names a size by its scale, never by a length.
pub const OBJECT_SCALES: u32 = 4;
