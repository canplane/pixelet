// pxl-kernel/sys/io/host.spec.rs
//
// GENERATED from spec/client/kernel/sys/io/host.json
//
// The kernel's own hostcalls, numbered beside xpute's (xpute/spec/ipc/sys.json,
// pxl-kernel sys/io/host.rs, ts/client/src/driver/text.ts) and made as hostcalls, answered
// before they return; and the range of the stream xpute's calls are recorded in, which
// a hostcall's record lies past while it runs.

// Names as the spec spells them, so one record has one name on every side.
// A table is generated whole, whether or not this crate calls an entry.
#![allow(
    non_upper_case_globals,
    non_snake_case,
    clippy::absolute_paths,
    dead_code,
    reason = "generated from the spec"
)]

/// The stream's range, laid out by the kernel's map.
pub mod stream {
    /// The range's size. A record names what it moves rather than carrying it, so
    /// a turn's records are words, and a hostcall's text bytes the most of what it holds.
    pub const BYTES: u32 = 65536;
}

/// The kernel's calls, each with xpute's PROGRAM bit and made as a hostcall; a record's call, then its arguments.
pub mod op {
    /// weight, px (f32 bits), max width (f32 bits, 0 for one line), at, lines, then the
    /// text's bytes: the text in the platform's own family at the weight and size, broken
    /// into lines no wider than the width where it can be; at `at`, how many lines, then
    /// a line's end (a byte offset) and its width (f32) apiece, `lines` of them at most
    pub const TEXT_LAYOUT: u32 = 2147483649;
    /// weight, px (f32 bits), color (0xRRGGBBAA), at, width, height, x (f32 bits), then
    /// the text's bytes: one line drawn in the platform's own family, starting at x with
    /// the middle of its em box on the middle row, into `width` by `height` pixels at
    /// `at`, RGBA a byte a channel, top row first, alpha not premultiplied
    pub const TEXT_RASTER: u32 = 2147483650;
}

/// TEXT_LAYOUT's arguments, a word each after its call, then the text's bytes at
/// `WORDS`: the weight, px and max width as f32 bits, where its answer goes and
/// how many lines it may hold.
pub mod LAYOUT {
    pub const weight: u32 = 0;
    pub const px: u32 = 1;
    pub const max_width: u32 = 2;
    pub const at: u32 = 3;
    pub const lines: u32 = 4;
    pub const WORDS: u32 = 5;
}

/// TEXT_LAYOUT's answer at its `at`: how many lines, then `LINE` apiece.
pub mod LAYOUT_ANSWER {
    pub const cnt: u32 = 0;
    pub const WORDS: u32 = 1;
}

/// A line of TEXT_LAYOUT's answer: its end, a byte offset, and its width as f32 bits.
pub mod LINE {
    pub const end: u32 = 0;
    pub const width: u32 = 1;
    pub const WORDS: u32 = 2;
}

/// TEXT_RASTER's arguments, a word each after its call, then the text's bytes at
/// `WORDS`: the weight and px as f32 bits, the color, where its pixels go, their
/// width and height, and x as f32 bits.
pub mod RASTER {
    pub const weight: u32 = 0;
    pub const px: u32 = 1;
    pub const color: u32 = 2;
    pub const at: u32 = 3;
    pub const width: u32 = 4;
    pub const height: u32 = 5;
    pub const x: u32 = 6;
    pub const WORDS: u32 = 7;
}

/// The roots the kernel opens files below (xpute's OPENAT). Below STORAGE a root is a
/// bake, numbered by its kind (pxl-spec bake/publication.spec.rs). Below every root a path is
/// an object's number in decimal, its one name; what that object is called and where, a
/// tile's version and URL, a storage key, is what the host (ts/client/src/driver/namespace/)
/// binds the root to, `@xpute/host` carrying the open there.
pub mod root {
    /// The platform's storage; a path is a storage file's number in decimal (`storage`); where the platform holds a slot is the host's choice.
    pub const STORAGE: u32 = 256;
    /// The shipped assets; a path is a load's number in decimal, the catalog's (`CATALOG`) or a file it names (sys/io/loader.rs).
    pub const ASSET: u32 = 257;
    /// The placed objects, their source's: a file a shard, read, its path the shard's subject in
    /// decimal, as a bake's tile is named (pxl-spec space/layer/tile.rs), its `format.OBJECT`
    /// first; the top of their tree, read (`object.TOP`); and their edits, written
    /// (`object.EDITS`) (domain/layer/object/repository.rs). A shard with none has no file. The
    /// source keeps the shards and the top and applies the edits; which source, the platform's
    /// storage or a server, is the host's.
    pub const OBJECT: u32 = 258;
}

/// Below the earth's root, the Earth's picture whole (pxl-spec route.json's `TILE_SPHERE`),
/// under the earth's own version. Any other number below a bake's root is a tile's subject
/// (pxl-spec space/layer/tile.rs), which is never 0: a tile's key ends in a 1 within its
/// high half.
pub const SPHERE: u32 = 0;

/// Below the asset root, the catalog the host plugs in (pxl-spec asset/catalog.rs): the
/// file each of the asset specs' references names. Any other number below it is a file,
/// its row in the asset ingest's table counted from 1 (spec/spec/asset/ingest.json).
pub const CATALOG: u32 = 0;

/// Below OBJECT, the files that are no shard's; any other number below it is a shard's
/// subject, which is neither: a tile's key ends in a 1 within its high half, at bit 32 or
/// above for a tile the pyramid holds.
pub mod object {
    /// The top of the tree, read: what each node at `SHARD_ZOOM` holding objects folds to
    /// (`top`), so which shards hold any, its `format.OBJECT_TOP` first. Its source rebuilds
    /// it from the shards as an edit lands; none is no shard holding any.
    pub const TOP: u32 = 0;
    /// Edits, written, never read: a batch of writes in the order they were made (`write`),
    /// its `format.OBJECT_EDITS` first, which the source applies to its shards.
    pub const EDITS: u32 = 1;
}

/// The zoom of the tile a shard below OBJECT is: pxl-engine's `SHARD_ZOOM`, which the
/// kernel holds this to (domain/layer/object/repository.rs). A key's digits to it name the
/// shard, and past it each zoom adds a digit on each of three axes (pxl-spec
/// space/hierarchy/morton.rs).
pub const SHARD_ZOOM: u32 = 8;

/// The children of the tree packet a shard's file and an edit batch hold after their
/// format byte, by index: each row a write `<key, id, ext>`, a leaf as the write that made
/// it in a shard's file, in the order made in a batch, where VOID erases (pxl-kernel
/// domain/layer/object/store.rs `Write`). A record a class's (pxl-spec object/ext.rs),
/// its row and its fields as columns.
pub mod write {
    /// u64[]: each key's high half (pxl-spec space/hierarchy/morton.rs `key_of`, 3D).
    pub const KEY_HI: u32 = 0;
    /// u64[]: each key's low half.
    pub const KEY_LO: u32 = 1;
    /// u32[]: each catalog value.
    pub const ID: u32 = 2;
    /// u32[]: the rows a marker's text is for, a marker's each.
    pub const TEXT_ROW: u32 = 3;
    /// strs: those markers' texts.
    pub const TEXT: u32 = 4;
}

/// The children of the tree packet `object.TOP` holds after its format byte, by index:
/// each row a node at `SHARD_ZOOM` holding objects, which only shards' leaves are under;
/// a shard has two, below sea level and above.
pub mod top {
    /// u64[]: the shard it is, by subject.
    pub const SHARD: u32 = 0;
    /// i32[]: its index on Z, -1 below sea level and 0 above.
    pub const Z: u32 = 1;
    /// u32[]: what its leaves fold to, its word in the objects' tree (pxl-engine
    /// overwrite.rs `fold`, `fold_word`): the kind most of its fill is and its share filled.
    pub const FOLD: u32 = 2;
}

/// The files the kernel keeps below STORAGE, by slot. What a slot holds is its
/// owner's, written whole, its slot's `format` first (pxl-kernel sys/io/file.rs).
pub mod storage {
    /// The settings' overrides (domain/view/setting/store.rs).
    pub const SETTINGS: u32 = 0;
}

/// Each kept file's format, the first byte of what it holds: a storage slot's by its
/// name, a shard's file below OBJECT as OBJECT, the top and an edit batch as theirs. The
/// kernel reads a file of its format and refuses any other, reading none of it and never
/// writing over it, as a driver refuses a volume whose superblock it does not know; what
/// to do with the file is the host's. Bump one whenever a change makes an older file mean
/// something other than it meant when it was written.
pub mod format {
    pub const SETTINGS: u8 = 1;
    pub const OBJECT: u8 = 16;
    pub const OBJECT_TOP: u8 = 17;
    pub const OBJECT_EDITS: u8 = 18;
}
