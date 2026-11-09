//! Answers the core's files in this harness, standing in for how ts/client/src/service/sys/fs/
//! binds the roots on a device. The shipped assets come from the repository: the catalog
//! the client plugs in, and each file it names from asset/content/. The terrain is made
//! here tile by tile, encoded as the bake writes a tile (`terrain`). The other bakes have
//! no tiles, and the platform's storage and the objects' source hold nothing. A write is
//! accepted and dropped.

use std::borrow::Cow;
use std::collections::BTreeMap;
use std::path::Path;

use pxl_spec::asset::cube::ground::{forest, grass};
use pxl_spec::bake::publication::BaseLayer;
use pxl_spec::bake::separation::Separation;
use pxl_spec::bake::tile::terrain::{self, TerrainTile, ELE_COLUMN, FOLD_COLUMN};
use pxl_spec::bake::tile::Occupancy;
use pxl_spec::space::coord::zoom::LEAF_ZOOM;
use pxl_spec::space::layer::lattice::TILE_CELLS;
use pxl_spec::space::layer::quadrant::Quadrant;
use xpute_kit::status::errno::Errno;

use crate::spec;

/// The ground's height everywhere, in leaf cells over the sea.
const HEIGHT: i16 = 8;

/// The terrain tile named `key`: level ground at `HEIGHT`, with bands of forest across grass
/// so decor stands on it and casts shadows. Every tile has all four children, down to the
/// leaves.
fn terrain(key: &str) -> Option<Vec<u8>> {
    let q = Quadrant::of_key(key)?;
    let cell = |c: usize| {
        let [x, y] = [c % TILE_CELLS as usize, c / TILE_CELLS as usize];
        if (x / 8 + y / 8) % 3 == 0 {
            forest
        } else {
            grass
        }
    };
    let ele = vec![HEIGHT; ELE_COLUMN];
    let tile = if q.zoom == LEAF_ZOOM {
        let cells = (0..FOLD_COLUMN / 4).map(cell).collect::<Vec<_>>();
        TerrainTile::leaf(ele, Cow::Owned(cells), Separation::default())
    } else {
        // Each cell repeats its class in all four palette indices, as a fold of one class
        // does.
        let cells = (0..FOLD_COLUMN).map(|i| cell(i / 4)).collect::<Vec<_>>();
        TerrainTile::of(
            ele,
            Cow::Owned(cells),
            Separation::default(),
            Occupancy::of_slots([true; 4]),
        )
    };
    Some(terrain::encode(&tile).expect("a tile encodes"))
}

struct Open {
    /// What a read copies out; none for a file opened to write.
    bytes: Option<Vec<u8>>,
}

pub struct Fs {
    open: BTreeMap<u32, Open>,
    asset: u32,
    catalog: u32,
    /// Each shipped file's path below asset/content/, with file number n at index n - 1
    /// (spec/spec/asset/ingest.json).
    assets: Vec<String>,
    /// The flag FS_OPENAT passes to open a file for writing (xpute spec/ipc/fs.json).
    wronly: u32,
}

/// The catalog's bytes, read from the generated file the client plugs it in from
/// (ts/client/src/service/sys/fs/catalog.spec.ts).
fn catalog() -> Vec<u8> {
    let at = Path::new(env!("PXL_REPO")).join("ts/client/src/service/sys/fs/catalog.spec.ts");
    let text = std::fs::read_to_string(&at).unwrap_or_else(|e| panic!("{}: {e}", at.display()));
    let hex = text
        .split("CATALOG_HEX =")
        .nth(1)
        .and_then(|t| t.split('"').nth(1))
        .expect("the catalog's hex");
    hex::decode(hex).expect("the catalog is hex")
}

impl Fs {
    pub fn new() -> Fs {
        let ingest = spec::json("spec/spec/asset/ingest.json");
        let assets = ingest["files"]
            .as_array()
            .expect("the ingest's files")
            .iter()
            .map(|f| f["path"].as_str().expect("a file's path").to_string())
            .collect();
        let flags = spec::json("xpute/spec/ipc/fs.json");
        Fs {
            open: BTreeMap::new(),
            asset: spec::file_val(&["root", "ASSET"]),
            catalog: spec::file_val(&["CATALOG"]),
            assets,
            wronly: flags["flag"]["O_WRONLY"]["$val"]
                .as_u64()
                .expect("O_WRONLY") as u32,
        }
    }

    /// The whole file at `path` below `root`, or the errno the client would answer.
    fn bytes(&self, root: u32, path: &str) -> Result<Vec<u8>, Errno> {
        if root == BaseLayer::terrain as u32 {
            return terrain(path).ok_or(Errno::ENOENT);
        }
        if root != self.asset {
            return Err(Errno::ENOENT);
        }
        let n: u32 = path.parse().map_err(|_| Errno::ENOENT)?;
        if n == self.catalog {
            return Ok(catalog());
        }
        let file = self.assets.get(n as usize - 1).ok_or(Errno::ENOENT)?;
        std::fs::read(Path::new(env!("PXL_REPO")).join("asset/content").join(file))
            .map_err(|_| Errno::ENOENT)
    }

    /// Answers FS_OPENAT with its completion's result: the file's size, or an errno.
    pub fn open(&mut self, fd: u32, root: u32, flags: u32, path: &str) -> i32 {
        if flags == self.wronly {
            self.open.insert(fd, Open { bytes: None });
            return 0;
        }
        match self.bytes(root, path) {
            Ok(bytes) => {
                let n = bytes.len() as i32;
                self.open.insert(fd, Open { bytes: Some(bytes) });
                n
            }
            Err(e) => e as i32,
        }
    }

    pub fn read(&self, fd: u32) -> &[u8] {
        self.open
            .get(&fd)
            .and_then(|f| f.bytes.as_deref())
            .unwrap_or_else(|| panic!("a read of {fd}, which is open to read nothing"))
    }

    /// Answers FS_WRITE. The bytes are kept nowhere, because the file written is dropped
    /// when it closes.
    pub fn write(&self, fd: u32) {
        assert!(
            self.open.get(&fd).is_some_and(|f| f.bytes.is_none()),
            "a write to {fd}, which is not open to write"
        );
    }

    /// Answers FS_CLOSE with its completion's result.
    pub fn close(&mut self, fd: u32) -> i32 {
        match self.open.remove(&fd) {
            Some(_) => 0,
            None => Errno::EBADF as i32,
        }
    }
}
