//! A file's bytes in this harness, standing in for the binding
//! ts/client/src/service/sys/fs/ gives on a device; xpute-runtime serves the file device
//! with them. The shipped assets come from the repository: the catalog
//! the client plugs in, and each file it names from asset/content/. The terrain is made
//! here tile by tile, encoded as the bake writes a tile (`terrain`). The other bakes have
//! no tiles, and the platform's storage and the objects' source hold nothing.

use std::borrow::Cow;
use std::path::Path;

use pxl_core::sys::file_spec::{path, CATALOG};
use pxl_spec::asset::cube::ground::{forest, grass};
use pxl_spec::asset::ingest::ASSET_PATHS;
use pxl_spec::bake::separation::Separation;
use pxl_spec::bake::tile::terrain::{self, TerrainTile, ELE_COLUMN, FOLD_COLUMN};
use pxl_spec::bake::tile::Occupancy;
use pxl_spec::space::coord::zoom::LEAF_ZOOM;
use pxl_spec::space::layer::lattice::TILE_CELLS;
use pxl_spec::space::layer::quadrant::Quadrant;
use xpute_core::io::fs::Token;
use xpute_kit::status::errno::Errno;

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

/// The whole file at `path`, a terrain tile (TERRAIN, quadkey) or a shipped file (ASSET,
/// number), or the errno the client would answer.
pub fn bytes(path: &[Token]) -> Result<Vec<u8>, Errno> {
    let n = match *path {
        [Token::Int(path::TERRAIN::ID), Token::Str(key)] => {
            return terrain(key).ok_or(Errno::ENOENT);
        }
        [Token::Int(path::ASSET::ID), Token::Int(n)] => n,
        _ => return Err(Errno::ENOENT),
    };
    if n as u32 == CATALOG {
        return Ok(catalog());
    }
    let file = ASSET_PATHS.get(n as usize - 1).ok_or(Errno::ENOENT)?;
    std::fs::read(Path::new(env!("PXL_REPO")).join("asset/content").join(file))
        .map_err(|_| Errno::ENOENT)
}
