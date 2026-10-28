// pxl-spec/asset.rs

//! The content library: every asset the specs declare (`spec/spec/asset/`),
//! each a definition of a kind — the cubes and their colors, the decors, the
//! marker, the actors, the sky — numbered by its id, its kind over its shape
//! (`id`), and each file a definition references a slot, which the catalog
//! the host plugs in fills (`catalog`). The texture pack is the cube kind's:
//! which of its tiles a cube wears on each face (`ingested`). A cube with no
//! image still draws as its catalog color.

#![allow(non_upper_case_globals)]

use std::sync::LazyLock;

#[path = "asset/actor.spec.rs"]
pub mod actor;
pub mod catalog;
#[path = "asset/color.spec.rs"]
pub mod color;
pub mod cube;
#[path = "asset/decor.spec.rs"]
pub mod decor;
pub mod id;
#[path = "asset/ingested.spec.rs"]
pub mod ingested;
pub mod marker;
#[path = "asset/sky.spec.rs"]
pub mod sky;
pub mod star;
#[path = "asset/symbol.spec.rs"]
pub mod symbol;

const _: () = assert!(
    symbol::SYMBOL_RATIO as f64 == crate::space::camera::grid::RATIO_MAX,
    "the symbols are rasterized at the finest ratio a surface is drawn at"
);

pub use decor::DecorId;
pub use id::AssetId;

use crate::space::coord::aabb::Aabb;
use crate::space::coord::zoom::units;
use catalog::Catalog;
use cube::Cube;
use ingested::{CubeFace, PACK};

/// A catalog color, packed `0xRRGGBB` by the generator.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Swatch {
    pub rgb: u32,
}

impl Swatch {
    pub const fn new(rgb: u32) -> Swatch {
        Swatch { rgb }
    }
}

impl decor::Variant {
    /// The largest dimension of its model, in units, as the catalog measures
    /// it, and none before one is read; NaN is kept rather than dropped, so
    /// a model the ingest could not measure is never taken for none.
    pub fn extent(&self, catalog: Option<&Catalog>) -> f64 {
        model_extent(self, catalog).into_iter().fold(0.0, largest)
    }
}

fn model_extent(v: &decor::Variant, catalog: Option<&Catalog>) -> [f64; 3] {
    catalog.map_or([0.0; 3], |c| c.extent(v.model))
}

impl decor::Decor {
    /// What a placed one is stored as.
    pub const fn value(&self) -> u16 {
        self.id as u16
    }

    /// The largest over every variant's model, in units.
    pub fn extent(&self, catalog: Option<&Catalog>) -> f64 {
        self.variants
            .iter()
            .map(|v| v.extent(catalog))
            .fold(0.0, largest)
    }

    /// How far any variant's box, or its model about the box's middle, reaches
    /// from the box's min corner however it turns, in units: its box alone
    /// before a catalog is read.
    pub fn reach(&self, catalog: Option<&Catalog>) -> f64 {
        let cell = units(self.row.cell_zoom());
        let reach = |(v, size): (&decor::Variant, &[u32; 3])| {
            let [ex, ey, ez] = model_extent(v, catalog);
            let [x, y, z] = size.map(|n| f64::from(n) * cell);
            let (across, up) = (x.max(y).max(ex).max(ey), z.max(ez));
            libm::sqrt(2.0 * across * across + up * up)
        };
        self.variants
            .iter()
            .zip(&self.row.sizes)
            .map(reach)
            .fold(0.0, largest)
    }

    /// The sides of `variant`'s box turned `turn`, in units.
    fn sides(&self, variant: u32, turn: u32) -> [f64; 3] {
        self.row
            .cells(variant, turn)
            .map(|n| f64::from(n) * units(self.row.cell_zoom()))
    }

    /// `variant` turned `turn` with its box's min corner at `corner`, in
    /// units: where it stands, its base's middle, which its model's origin is
    /// put at, and what it carries at `scale`.
    pub fn stand(
        &self,
        corner: [f64; 3],
        variant: u32,
        turn: u32,
        scale: f64,
    ) -> ([f64; 3], decor::DecorMeta) {
        let [x, y, _] = self.sides(variant, turn);
        let at = [corner[0] + x / 2.0, corner[1] + y / 2.0, corner[2]];
        (
            at,
            decor::DecorMeta {
                variant,
                turn,
                scale,
            },
        )
    }

    /// The box one standing at `at`, carrying `m`, claims: every cell in it is its.
    pub fn claim(&self, at: [f64; 3], m: &decor::DecorMeta) -> Aabb {
        let [x, y, z] = self.sides(m.variant, m.turn);
        let min = [at[0] - x / 2.0, at[1] - y / 2.0, at[2]];
        Aabb {
            min,
            max: [min[0] + x, min[1] + y, min[2] + z],
        }
    }

    /// Where its variants are numbered from as draw groups, as the GPU's
    /// decor numbers them (pxl-spec-core scatter.rs `first_group`).
    pub fn first_group(&self) -> u32 {
        let kind = decor::DECORS.iter().take_while(|d| d.id != self.id).count();
        pxl_spec_core::scatter::first_group(&pxl_spec_core::decor::DECORS, kind as u32)
    }

    /// The generator writes one entry a decor, so every id has one.
    pub fn of(id: DecorId) -> &'static decor::Decor {
        use xpute_kit::status::bug::OrBug;
        decor::DECORS
            .iter()
            .find(|d| d.id == id)
            .or_bug(xpute_kit::status::errno::Errno::ENOTRECOVERABLE)
    }
}

impl decor::DecorMeta {
    /// Its heading in radians.
    pub fn spin(&self) -> f64 {
        f64::from(self.turn % 4) * core::f64::consts::FRAC_PI_2
    }
}

fn largest(a: f64, b: f64) -> f64 {
    if a.is_nan() || b.is_nan() {
        f64::NAN
    } else {
        a.max(b)
    }
}

/// A decor's asset id, what a placed one is stored as.
pub fn decor_num_id(id: DecorId) -> AssetId {
    id as u32
}

/// The decor an asset id of the decor kind names.
pub fn decor_id_from_num(num_id: AssetId) -> DecorId {
    match u16::try_from(num_id).ok().and_then(DecorId::of) {
        Some(id) => id,
        None => xpute_kit::bug!(ENOTRECOVERABLE, num_id),
    }
}

/// In numeric order.
pub static decor_ids: LazyLock<Vec<DecorId>> = LazyLock::new(|| {
    let mut ids = DecorId::ALL.to_vec();
    ids.sort_by_key(|id| *id as u16);
    ids
});

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct TileOrigin {
    pub x: u32,
    pub y: u32,
}

/// The pack tile a face wears; None where the pack has no image.
fn tile_of(cube: Cube, face: CubeFace) -> Option<u32> {
    PACK.tiles
        .iter()
        .find(|(c, f, _)| *c == cube && *f == face)
        .map(|&(_, _, n)| n)
}

/// The pack tile a sentinel the world draws wears on every face; None where
/// the pack has none.
fn sentinel_tile_of(sentinel: u8) -> Option<u32> {
    PACK.sentinels
        .iter()
        .find(|(s, _)| *s == sentinel)
        .map(|&(_, n)| n)
}

/// The tile's mean color in linear light, seen from far enough that the tile
/// is gone.
pub fn shipped_tile_mean(cube: Cube, face: CubeFace) -> Option<[u8; 3]> {
    PACK.means.get(tile_of(cube, face)? as usize).copied()
}

/// A sentinel's tile's, as `shipped_tile_mean`.
pub fn shipped_sentinel_mean(sentinel: u8) -> Option<[u8; 3]> {
    PACK.means
        .get(sentinel_tile_of(sentinel)? as usize)
        .copied()
}

fn origin(n: u32) -> TileOrigin {
    TileOrigin {
        x: (n % PACK.columns) * PACK.cell,
        y: (n / PACK.columns) * PACK.cell,
    }
}

/// The tile's pixel origin in the pack's image.
pub fn shipped_texture_tile(cube: Cube, face: CubeFace) -> Option<TileOrigin> {
    tile_of(cube, face).map(origin)
}

/// A sentinel's tile's, as `shipped_texture_tile`.
pub fn shipped_sentinel_tile(sentinel: u8) -> Option<TileOrigin> {
    sentinel_tile_of(sentinel).map(origin)
}

/// The width and height a PNG's or a WebP's header gives; none for any other file.
pub fn image_size(file: &[u8]) -> Option<(u32, u32)> {
    let be = |at: usize| Some(u32::from_be_bytes(file.get(at..at + 4)?.try_into().ok()?));
    let le24 = |at: usize| {
        Some(u32::from_le_bytes([
            *file.get(at)?,
            *file.get(at + 1)?,
            *file.get(at + 2)?,
            0,
        ]))
    };
    let le16 = |at: usize| {
        Some(u32::from(u16::from_le_bytes(
            file.get(at..at + 2)?.try_into().ok()?,
        )))
    };
    if file.starts_with(b"\x89PNG\r\n\x1a\n") {
        return Some((be(16)?, be(20)?));
    }
    if !(file.starts_with(b"RIFF") && file.get(8..12) == Some(b"WEBP")) {
        return None;
    }
    match file.get(12..16)? {
        // Lossy: the frame's 14-bit width and height after its start code.
        b"VP8 " => Some((le16(26)? & 0x3fff, le16(28)? & 0x3fff)),
        // Lossless: 14 bits each, less one, after the signature byte.
        b"VP8L" => {
            let bits = u32::from_le_bytes(file.get(21..25)?.try_into().ok()?);
            Some(((bits & 0x3fff) + 1, (bits >> 14 & 0x3fff) + 1))
        }
        // Extended: the canvas's 24-bit width and height, less one.
        b"VP8X" => Some((le24(24)? + 1, le24(27)? + 1)),
        _ => None,
    }
}
