// pxl-geo-bake/terrain/tile_set.rs

//! A terrain version on disk, laid out as the bucket holds it: a directory a
//! level, an object a page named by its key, and the ocean page beside the
//! levels. The offline bake writes one whole; the dev tile server fills one a
//! page at a time as pages are wanted. Either way what is on disk is what is
//! served, and a level's pages are read back from here to fold the next.

use std::fs;
use std::path::{Path, PathBuf};

use pxl_spec::bake::format::{decode_terrain, Terrain};
use pxl_spec::space::pvf3::ivec::IVec;

use crate::terrain::leaf::OCEAN_KEY;
use crate::util::{unpack, write_atomic};

/// The marker a page that does not exist leaves, so that asking again does
/// not ask the database again. Local to a tile set and never uploaded.
const ABSENT: &str = "empty";

pub struct TileSet {
    pub root: PathBuf,
}

impl TileSet {
    pub fn new(root: impl Into<PathBuf>) -> TileSet {
        TileSet { root: root.into() }
    }

    pub fn level_dir(&self, level: u32) -> PathBuf {
        self.root.join(level.to_string())
    }

    pub fn path(&self, level: u32, page: IVec) -> PathBuf {
        self.level_dir(level).join(page.to_string())
    }

    pub fn ocean_path(&self) -> PathBuf {
        self.root.join(OCEAN_KEY)
    }

    /// Stores a page's packed object.
    pub fn write(&self, level: u32, page: IVec, packed: &[u8]) -> Result<(), String> {
        let path = self.path(level, page);
        fs::create_dir_all(self.level_dir(level)).map_err(|e| e.to_string())?;
        write_atomic(&path, packed).map_err(|e| format!("{}: {e}", path.display()))
    }

    /// Records that a page does not exist.
    pub fn write_absent(&self, level: u32, page: IVec) -> Result<(), String> {
        fs::create_dir_all(self.level_dir(level)).map_err(|e| e.to_string())?;
        let path = self.path(level, page).with_extension(ABSENT);
        fs::write(&path, b"").map_err(|e| format!("{}: {e}", path.display()))
    }

    /// Whether a page is settled on disk: some when it is (whether it exists),
    /// none when nothing has been decided about it yet.
    pub fn settled(&self, level: u32, page: IVec) -> Option<bool> {
        let path = self.path(level, page);
        if path.exists() {
            Some(true)
        } else if path.with_extension(ABSENT).exists() {
            Some(false)
        } else {
            None
        }
    }

    /// A stored page, decoded, or none where the page does not exist.
    pub fn read(&self, level: u32, page: IVec) -> Result<Option<Terrain>, String> {
        let path = self.path(level, page);
        if !path.exists() {
            return Ok(None);
        }
        read_page(&path).map(Some)
    }

    /// The pages a level holds on disk, in key order.
    pub fn pages(&self, level: u32) -> Result<Vec<IVec>, String> {
        let dir = self.level_dir(level);
        if !dir.exists() {
            return Ok(Vec::new());
        }
        let mut out = Vec::new();
        for entry in fs::read_dir(&dir).map_err(|e| format!("{}: {e}", dir.display()))? {
            let name = entry.map_err(|e| e.to_string())?.file_name();
            if let Some(page) = name.to_str().and_then(|n| n.parse::<IVec>().ok()) {
                out.push(page);
            }
        }
        out.sort_unstable();
        Ok(out)
    }
}

pub fn read_page(path: &Path) -> Result<Terrain, String> {
    let stored = fs::read(path).map_err(|e| format!("{}: {e}", path.display()))?;
    let raw = unpack(&stored).map_err(|e| format!("{}: {e}", path.display()))?;
    decode_terrain(&raw).map_err(|e| format!("{}: {e:?}", path.display()))
}
