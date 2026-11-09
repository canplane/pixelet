// pxl-spec/value.rs

//! wgsl-port: the value space that every layer stores and the engine reads. A ground cell
//! is a byte. An object's value is a u16: its byte range is the ground's, and the range past
//! a byte holds what only an object can be. Decor values lie past the cube kind
//! (`asset/id.json`). This fixes the sentinels and the range each class of value takes; the
//! palette lies in those ranges (asset.rs).

/// Nothing painted: the ground before a bake writes it, and an object cell
/// no leaf holds.
pub const VOID: u32 = 0x00;

/// Where the world holds no more: the floor under the ground, the wall just inside the
/// service area, and any byte no bake wrote, so that memory filled with 0xFF reads as
/// unknown. Nothing is placed on it or taken from it.
pub const UNKNOWN: u32 = 0xff;

/// Explicit air, which only an object holds. It keeps its leaf in the tree and erases the
/// layers under it. It lies past a byte, where no ground value can be.
pub const AIR: u32 = 0x100;

/// The ground's palette entries, first and last: every byte but the
/// sentinels.
pub const GROUNDS: [u32; 2] = [0x01, 0xfe];

/// The ways' entries among the ground's, first and last. Decor faces along a way, and no
/// pier stands on one.
pub const TRANSPORTS: [u32; 2] = [37, 55];

/// The entry a pier is drawn as: a ground entry that is not a way's.
pub const PIER: u32 = 35;

/// The colors only an object takes, first and last: past air, within the
/// cube kind (`asset/id.json` `IDX_BITS`).
pub const OBJECTS: [u32; 2] = [0x101, 0x3ff];

/// Whether a value read into a word is a ground palette entry (`GROUNDS`). A GPU has no byte
/// type, so every value is tested as a word.
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
