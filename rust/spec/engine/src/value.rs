// pxl-engine/value.rs

//! gpu-portable: the value space every layer stores and the engine reads by:
//! a ground cell a byte, an object's value a u16 whose byte range is the
//! ground's and whose range past it is what only an object can be, and
//! decor's past the cube kind (pxl-spec id.json). The engine states its
//! sentinels and the slots each class of value takes; a content lays its
//! palette into them, every value where it is.

/// Nothing painted: the ground before a bake writes it, and an object cell
/// no leaf holds.
pub const VOID: u32 = 0x00;

/// Where the world holds no more: the floor under the ground and the wall
/// just inside the service area, and a byte no bake wrote, so memory filled
/// with 0xFF reads as it; nothing is placed on or taken from it.
pub const UNKNOWN: u32 = 0xff;

/// Explicit air, what only an object holds: it retains its leaf and erases
/// the layers under it, past a byte, where no ground cell lies.
pub const AIR: u32 = 0x100;

/// The ground's palette entries, first and last: every byte but the
/// sentinels.
pub const GROUNDS: [u32; 2] = [0x01, 0xfe];

/// The ways' entries among the ground's, first and last: decor faces along
/// them and no pier stands on them.
pub const TRANSPORTS: [u32; 2] = [37, 55];

/// What a pier is drawn as: a ground's entry, no way's.
pub const PIER: u32 = 35;

/// The colors only an object takes, first and last: past air, within the
/// cube kind (pxl-spec id.json `SHAPE_BITS`).
pub const OBJECTS: [u32; 2] = [0x101, 0x3ff];

/// Whether a value read into a word, as a GPU has it with no byte, is a
/// ground's palette entry (`GROUNDS`).
pub const fn grounds(val: u32) -> bool {
    val >= GROUNDS[0] && val <= GROUNDS[1]
}

/// Whether a value read into a word is a way's entry (`TRANSPORTS`).
pub const fn transports(val: u32) -> bool {
    val >= TRANSPORTS[0] && val <= TRANSPORTS[1]
}

/// Whether a value read into a word is a color only an object takes
/// (`OBJECTS`).
pub const fn objects(val: u32) -> bool {
    val >= OBJECTS[0] && val <= OBJECTS[1]
}

const _: () = assert!(
    !grounds(VOID) && !grounds(UNKNOWN) && !grounds(AIR) && !objects(AIR),
    "no class holds a sentinel or air"
);
const _: () = assert!(
    grounds(TRANSPORTS[0]) && grounds(TRANSPORTS[1]),
    "the ways' entries are the ground's"
);
const _: () = assert!(
    grounds(PIER) && !transports(PIER),
    "a pier is a ground's entry, no way's"
);
