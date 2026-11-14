// pxl-spec/bake/object.spec.rs
//
// GENERATED from spec/spec/bake/object.json (xpute-gen data.rs)
//
// The objects a player places, as the bake and the UI count them.

/// How many scales an object places at: a unit and its halvings, as a decor's
/// `scale_step`, 0, -1, -2 and -3. There are four so that the smallest is 0.125u, an
/// eighth of the 1u cell. The UI names a size by its scale, never by a length.
pub const OBJECT_SCALE_CNT: u32 = 4;
