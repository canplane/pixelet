// @pxl/client/abi/file.spec.ts
//
// GENERATED from spec/client/core/sys/io/file.json5 (xpute-gen data.rs)
//
// The core's files: the tokens of their paths and what a file holds (pxl-core
// sys/io/file.rs, ts/client/src/service/sys/fs/).

/** The tree of the paths the core opens (xpute wit/fs/file.wit), as a URI's path is a
 * tree: each node takes one token, its `ID`, distinct among its siblings, and hands the
 * rest of the path to the child it names. A node that holds data takes the tokens left as
 * that data: a tile's quadkey (pxl-spec space/layer/quadrant.rs) as a string, or a load's id.
 * What a file is past its path, a tile's version and URL, a storage key, is the client's
 * binding (ts/client/src/service/sys/fs/). */
export const path = {
  /** A base layer's tile: (the layer, quadkey). Each `BaseKind` (spec/spec/bake/publication.json5
   * `BASE_KIND`) is a node of its own, named as the member is, its `ID` the member's number. */
  TERRAIN: {
    ID: 0 as number,
  },
  /** A base layer's tile: (the layer, quadkey). Each `BaseKind` (spec/spec/bake/publication.json5
   * `BASE_KIND`) is a node of its own, named as the member is, its `ID` the member's number. */
  PLACE: {
    ID: 1 as number,
  },
  /** A base layer's tile: (the layer, quadkey). Each `BaseKind` (spec/spec/bake/publication.json5
   * `BASE_KIND`) is a node of its own, named as the member is, its `ID` the member's number. */
  DIVISION: {
    ID: 2 as number,
  },
  /** A base layer's tile: (the layer, quadkey). Each `BaseKind` (spec/spec/bake/publication.json5
   * `BASE_KIND`) is a node of its own, named as the member is, its `ID` the member's number. */
  EARTH: {
    ID: 3 as number,
    /** The picture of the whole Earth, (EARTH, SPHERE), served under the Earth layer's
     * version as its `TILE_SPHERE` (spec/spec/bake/publication.json5). */
    SPHERE: {
      ID: 0 as number,
    },
  },
  /** The platform's storage, where the platform holds a file being the client's choice. What a file holds is its owner's, written whole, its `format` first (pxl-core sys/io/file.rs). */
  STORAGE: {
    ID: 4 as number,
    /** The settings' overrides, in the platform's storage (domain/setting.rs). */
    SETTINGS: {
      ID: 0 as number,
    },
  },
  /** The shipped assets: (ASSET, id), a load's id, the catalog's (`CATALOG`) or a file it names (sys/io/loader.rs). */
  ASSET: {
    ID: 5 as number,
  },
  /** The placed objects, their source's: a shard, read, (OBJECT, quadkey), its
   * `format.OBJECT` first; and the top of their tree, read, (OBJECT, TOP)
   * (domain/layer/object/repository.rs). A shard with none has no file. The source keeps
   * the shards and the top and applies the edits the core sends it (pxl:core object
   * `apply`); which source, a mock or a server, is the client's. */
  OBJECT: {
    ID: 6 as number,
    /** The top of the tree, read: what each node at `SHARD_ZOOM` holding objects folds to
     * (`top`), so which shards hold any, its `format.OBJECT_TOP` first. Its source rebuilds
     * it from the shards as an edit lands; none is no shard holding any. */
    TOP: {
      ID: 0 as number,
    },
  },
} as const;

/** The load id of the catalog the client plugs in, (ASSET, CATALOG) (pxl-spec asset/catalog.rs):
 * the file each of the asset specs' references names. Any other id is a file,
 * its row in the asset ingest's table counted from 1 (spec/spec/asset/ingest.json5). */
export const CATALOG = 0 as number;

/** The finest zoom a tile's quadkey in a path is at, so the most digits it has:
 * pxl-spec's `LEAF_ZOOM`, which the core holds this to (sys/io/file.rs). */
export const LEAF_ZOOM = 15 as number;

/** The zoom of the tile a shard, (OBJECT, quadkey), is: pxl-engine's `SHARD_ZOOM`, which the
 * core holds this to (domain/layer/object/repository.rs). A key's first digits to it name
 * the shard, and each past it is a zoom's 0 to 7, on three axes (pxl-spec
 * space/hierarchy/morton.rs). */
export const SHARD_ZOOM = 8 as number;

/** The children of the tree packet a shard's file and an edit batch hold after their
 * format byte, by index: each row a write `<key, asset_id, ext>`, a leaf as the write that made
 * it in a shard's file, in the order made in a batch, where void erases (pxl-core
 * domain/layer/object/store.rs `Write`). A record a class's (pxl-spec object.rs),
 * its row and its fields as lanes. */
export const write = {
  /** A `strs` lane holding each key as an octkey (pxl-spec space/hierarchy/morton.rs `name`, 3D): the shard's quadkey, then a digit 0 to 7 a zoom. */
  KEY: 0 as number,
  /** A `u32[]` lane holding each row's catalog value. */
  ASSET_ID: 1 as number,
  /** A `u32[]` lane holding, for each marker with a text, the row that text is for. */
  TEXT_ROW: 2 as number,
  /** A `strs` lane holding those markers' texts. */
  TEXT: 3 as number,
} as const;

/** The children of the tree packet the top, (OBJECT, TOP), holds after its format byte, by index:
 * each row a shard holding objects, which only its leaves are under. */
export const top = {
  /** A `strs` lane holding each row's shard by its quadkey. */
  SHARD: 0 as number,
  /** A `u32[]` lane holding what each row's leaves fold to, its word in the object tree
   * (pxl-engine object.rs `fold`, `fold_word`): the kind most of its fill is and its share filled. */
  FOLD: 1 as number,
} as const;

/** Each kept file's format, the first byte of what it holds: a stored file's by its
 * name, a shard's file as OBJECT, the top as its own; and an edit batch's, sent rather
 * than kept, as its own. The core reads a file of its format and refuses any other,
 * reading none of it and never writing over it, as a driver refuses a volume whose
 * superblock it does not know, and sets it aside (fs `set-aside`). Bump one whenever a
 * change makes an older file mean something other than it meant when it was written. */
export const format = {
  SETTINGS: 1 as number,
  OBJECT: 19 as number,
  OBJECT_TOP: 22 as number,
  OBJECT_EDIT: 21 as number,
} as const;
