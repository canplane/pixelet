// pxl-core/spec/file.spec.rs
//
// GENERATED from spec/client/core/sys/io/file.json (xpute-gen data.rs)
//
// The core's files: the tokens of their paths and what a file holds (pxl-core
// sys/io/file.rs, ts/client/src/service/sys/fs/).

/// The tree of the paths the core opens (xpute's FS_OPEN), as a URI's path is a
/// tree: each node takes one token, its `ID`, distinct among its siblings, and hands the
/// rest of the path to the child it names. A node that holds data takes the tokens left as
/// that data: a tile's quadkey (pxl-spec space/layer/quadrant.rs) as a string, or a load's id.
/// What a file is past its path, a tile's version and URL, a storage key, is the client's
/// binding (ts/client/src/service/sys/fs/).
pub mod path {
    /// The terrain bake: (TERRAIN, quadkey).
    pub mod terrain {
        pub const ID: i32 = 0;
    }
    /// The places bake: (PLACE, quadkey).
    pub mod place {
        pub const ID: i32 = 1;
    }
    /// The divisions bake: (DIVISION, quadkey).
    pub mod division {
        pub const ID: i32 = 2;
    }
    /// The Earth bake: (EARTH, quadkey), and the picture of the whole Earth, (EARTH, SPHERE).
    pub mod earth {
        pub const ID: i32 = 3;
        /// The picture of the whole Earth, served under the Earth bake's version as its
        /// `SPHERE` (spec/spec/bake/publication.json).
        pub mod sphere {
            pub const ID: i32 = 0;
        }
    }
    /// The platform's storage, where the platform holds a file being the client's choice. What a file holds is its owner's, written whole, its `format` first (pxl-core sys/io/file.rs).
    pub mod storage {
        pub const ID: i32 = 4;
        /// The settings' overrides, in the platform's storage (domain/setting/store.rs).
        pub mod settings {
            pub const ID: i32 = 0;
        }
    }
    /// The shipped assets: (ASSET, id), a load's id, the catalog's (`CATALOG`) or a file it names (sys/io/loader.rs).
    pub mod asset {
        pub const ID: i32 = 5;
    }
    /// The placed objects, their source's: a shard, read, (OBJECT, quadkey), its
    /// `format.OBJECT` first; the top of their tree, read, (OBJECT, TOP); and their edits,
    /// written, (OBJECT, EDIT) (domain/layer/object/repository.rs). A shard with none has no
    /// file. The source keeps the shards and the top and applies the edits; which source, a
    /// mock or a server, is the client's.
    pub mod object {
        pub const ID: i32 = 6;
        /// The top of the tree, read: what each node at `SHARD_ZOOM` holding objects folds to
        /// (`top`), so which shards hold any, its `format.OBJECT_TOP` first. Its source rebuilds
        /// it from the shards as an edit lands; none is no shard holding any.
        pub mod top {
            pub const ID: i32 = 0;
        }
        /// Edits, written, never read: a batch of writes in the order they were made (`write`),
        /// its `format.OBJECT_EDIT` first, which the source applies to its shards.
        pub mod edit {
            pub const ID: i32 = 1;
        }
    }
}

/// The load id of the catalog the client plugs in, (ASSET, CATALOG) (pxl-spec asset/catalog.rs):
/// the file each of the asset specs' references names. Any other id is a file,
/// its row in the asset ingest's table counted from 1 (spec/spec/asset/ingest.json).
pub const CATALOG: u32 = 0;

/// The finest zoom a tile's quadkey in a path is at, so the most digits it has:
/// pxl-spec's `LEAF_ZOOM`, which the core holds this to (sys/io/file.rs).
pub const LEAF_ZOOM: u32 = 15;

/// The zoom of the tile a shard, (OBJECT, quadkey), is: pxl-engine's `SHARD_ZOOM`, which the
/// core holds this to (domain/layer/object/repository.rs). A key's first digits to it name
/// the shard, and each past it is a zoom's 0 to 7, on three axes (pxl-spec
/// space/hierarchy/morton.rs).
pub const SHARD_ZOOM: u32 = 8;

/// The children of the tree packet a shard's file and an edit batch hold after their
/// format byte, by index: each row a write `<key, asset_id, ext>`, a leaf as the write that made
/// it in a shard's file, in the order made in a batch, where void erases (pxl-core
/// domain/layer/object/store.rs `Write`). A record a class's (pxl-spec object/ext.rs),
/// its row and its fields as columns.
pub mod write {
    /// A `strs` column holding each key as an octkey (pxl-spec space/hierarchy/morton.rs `name`, 3D): the shard's quadkey, then a digit 0 to 7 a zoom.
    pub const KEY: u32 = 0;
    /// A `u32[]` column holding each row's catalog value.
    pub const ASSET_ID: u32 = 1;
    /// A `u32[]` column holding, for each marker with a text, the row that text is for.
    pub const TEXT_ROW: u32 = 2;
    /// A `strs` column holding those markers' texts.
    pub const TEXT: u32 = 3;
}

/// The children of the tree packet the top, (OBJECT, TOP), holds after its format byte, by index:
/// each row a shard holding objects, which only its leaves are under.
pub mod top {
    /// A `strs` column holding each row's shard by its quadkey.
    pub const SHARD: u32 = 0;
    /// A `u32[]` column holding what each row's leaves fold to, its word in the objects' tree
    /// (pxl-engine object.rs `fold`, `fold_word`): the kind most of its fill is and its share filled.
    pub const FOLD: u32 = 1;
}

/// Each kept file's format, the first byte of what it holds: a stored file's by its
/// name, a shard's file as OBJECT, the top and an edit batch as theirs. The
/// core reads a file of its format and refuses any other, reading none of it and never
/// writing over it, as a driver refuses a volume whose superblock it does not know; what
/// to do with the file is the client's. Bump one whenever a change makes an older file mean
/// something other than it meant when it was written.
pub mod format {
    pub const SETTINGS: u8 = 1;
    pub const OBJECT: u8 = 19;
    pub const OBJECT_TOP: u8 = 22;
    pub const OBJECT_EDIT: u8 = 21;
}
