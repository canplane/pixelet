// pxl-core/sys/io/file.spec.rs
//
// GENERATED from spec/client/core/sys/io/file.json
//
// The core's files: the roots it opens them below, the names below each and what a
// file holds (pxl-core sys/io/file.rs, ts/client/src/sys/fs/).

// Names as the spec spells them, so one record has one name on every side.
// A table is generated whole, whether or not this crate calls an entry.
#![allow(
    non_upper_case_globals,
    non_snake_case,
    clippy::absolute_paths,
    dead_code,
    reason = "generated from the spec"
)]

/// The roots the core opens files below (xpute's OPENAT). Below STORAGE a root is a
/// bake, numbered by its kind (pxl-spec bake/publication.spec.rs). Below every root a path is
/// one name: a tile's quadkey (pxl-spec space/layer/quadrant.rs), or what the root says. What it
/// is called and where past that, a tile's version and URL, a storage key, is what the client
/// (ts/client/src/sys/fs/) binds the root to, `@xpute/runtime` carrying the open there.
pub mod root {
    /// The platform's storage; a path is a storage file's id in decimal (`storage`); where the platform holds a slot is the client's choice.
    pub const STORAGE: u32 = 256;
    /// The shipped assets; a path is a load's id in decimal, the catalog's (`CATALOG`) or a file it names (sys/io/loader.rs).
    pub const ASSET: u32 = 257;
    /// The placed objects, their source's: a file a shard, read, its path the shard's quadkey,
    /// as a bake's tile is named, its `format.OBJECT` first; the top of their tree, read
    /// (`object.TOP`); and their edits, written (`object.EDITS`)
    /// (domain/layer/object/repository.rs). A shard with none has no file. The source keeps the
    /// shards and the top and applies the edits; which source, a mock or a server, is the client's.
    pub const OBJECT: u32 = 258;
}

/// Below the asset root, the catalog the client plugs in (pxl-spec asset/catalog.rs): the
/// file each of the asset specs' references names. Any other id below it is a file,
/// its row in the asset ingest's table counted from 1 (spec/spec/asset/ingest.json).
pub const CATALOG: u32 = 0;

/// Below OBJECT, the files that are no shard's; no quadkey has a letter.
pub mod object {
    /// The top of the tree, read: what each node at `SHARD_ZOOM` holding objects folds to
    /// (`top`), so which shards hold any, its `format.OBJECT_TOP` first. Its source rebuilds
    /// it from the shards as an edit lands; none is no shard holding any.
    pub const TOP: &str = "top";
    /// Edits, written, never read: a batch of writes in the order they were made (`write`),
    /// its `format.OBJECT_EDITS` first, which the source applies to its shards.
    pub const EDITS: &str = "edits";
}

/// The zoom of the tile a shard below OBJECT is: pxl-engine's `SHARD_ZOOM`, which the
/// core holds this to (domain/layer/object/repository.rs). A key's first digits to it name
/// the shard, and each past it is a zoom's 0 to 7, on three axes (pxl-spec
/// space/hierarchy/morton.rs).
pub const SHARD_ZOOM: u32 = 8;

/// The children of the tree packet a shard's file and an edit batch hold after their
/// format byte, by index: each row a write `<key, id, ext>`, a leaf as the write that made
/// it in a shard's file, in the order made in a batch, where VOID erases (pxl-core
/// domain/layer/object/store.rs `Write`). A record a class's (pxl-spec object/ext.rs),
/// its row and its fields as columns.
pub mod write {
    /// strs: each key as an octkey (pxl-spec space/hierarchy/morton.rs `name`, 3D): the shard's quadkey, then a digit 0 to 7 a zoom.
    pub const KEY: u32 = 0;
    /// u32[]: each catalog value.
    pub const ID: u32 = 1;
    /// u32[]: the rows a marker's text is for, a marker's each.
    pub const TEXT_ROW: u32 = 2;
    /// strs: those markers' texts.
    pub const TEXT: u32 = 3;
}

/// The children of the tree packet `object.TOP` holds after its format byte, by index:
/// each row a shard holding objects, which only its leaves are under.
pub mod top {
    /// strs: the shard it is, by quadkey.
    pub const SHARD: u32 = 0;
    /// u32[]: what its leaves fold to, its word in the objects' tree (pxl-engine
    /// object.rs `fold`, `fold_word`): the kind most of its fill is and its share filled.
    pub const FOLD: u32 = 1;
}

/// The files the core keeps below STORAGE, by slot. What a slot holds is its
/// owner's, written whole, its slot's `format` first (pxl-core sys/io/file.rs).
pub mod storage {
    /// The settings' overrides (domain/view/setting/store.rs).
    pub const SETTINGS: u32 = 0;
}

/// Each kept file's format, the first byte of what it holds: a storage slot's by its
/// name, a shard's file below OBJECT as OBJECT, the top and an edit batch as theirs. The
/// core reads a file of its format and refuses any other, reading none of it and never
/// writing over it, as a driver refuses a volume whose superblock it does not know; what
/// to do with the file is the client's. Bump one whenever a change makes an older file mean
/// something other than it meant when it was written.
pub mod format {
    pub const SETTINGS: u8 = 1;
    pub const OBJECT: u8 = 19;
    pub const OBJECT_TOP: u8 = 22;
    pub const OBJECT_EDITS: u8 = 21;
}
