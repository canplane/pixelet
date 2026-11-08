// pxl-spec/asset.rs

//! The content library: every asset the specs declare (`spec/spec/asset/`), each a
//! definition of a kind (the cubes and their colors, the decors, the marker, the characters,
//! the sky), numbered by its id, which is its kind over its shape (`id`). Each file a
//! definition references is a slot, which the catalog the client plugs in fills (`catalog`).
//! The texture pack belongs to the cube kind: it says which of its tiles a cube wears on each
//! face (`ingest`). A cube with no image still draws in its catalog color.

pub mod catalog;
#[path = "asset/character.spec.rs"]
pub mod character;
#[path = "asset/color.spec.rs"]
pub mod color;
pub mod cube;
#[path = "asset/decor.spec.rs"]
pub mod decor;
pub mod id;
#[path = "asset/ingest.spec.rs"]
pub mod ingest;
pub mod marker;
#[path = "asset/sky.spec.rs"]
pub mod sky;
pub mod star;
#[path = "asset/symbol.spec.rs"]
pub mod symbol;

pub use id::AssetId;

use crate::space::coord::aabb::Aabb;
use crate::space::coord::zoom::{units, UNIT_ZOOM};
use catalog::Catalog;
use ingest::{CubeFace, PACK};
use pxl_content::decor::DECORS;
use pxl_content::generation;
use xpute_kit::status::errno::Errno;

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
    /// The largest dimension of its model, in units, as the catalog measures it, or none before
    /// one is read. NaN is kept rather than dropped, so a model the ingest could not measure is
    /// never taken for none.
    pub fn extent(&self, catalog: Option<&Catalog>) -> f64 {
        model_extent(self, catalog).into_iter().fold(0.0, largest)
    }
}

fn model_extent(v: &decor::Variant, catalog: Option<&Catalog>) -> [f64; 3] {
    catalog.map_or([0.0; 3], |c| c.extent(v.model))
}

impl decor::Decor {
    /// What a placed one is stored as.
    pub const fn val(&self) -> u16 {
        self.id as u16
    }

    /// The largest over every variant's model, in units.
    pub fn extent(&self, catalog: Option<&Catalog>) -> f64 {
        self.variants
            .iter()
            .map(|v| v.extent(catalog))
            .fold(0.0, largest)
    }

    /// How far any variant's box reaches from its min corner however it
    /// turns, in units.
    pub fn box_extent(&self) -> f64 {
        let cell = units(UNIT_ZOOM);
        let extent = |size: &[u32; 3]| {
            let [x, y, z] = size.map(|n| f64::from(n) * cell);
            let across = x.max(y);
            libm::sqrt(2.0 * across * across + z * z)
        };
        self.row.sizes.iter().map(extent).fold(0.0, largest)
    }

    /// The sides of `variant`'s box turned `turn`, in units.
    fn sides(&self, variant: u32, turn: u32) -> [f64; 3] {
        self.row
            .cells(variant, turn)
            .map(|n| f64::from(n) * units(UNIT_ZOOM))
    }

    /// `variant` turned `turn` with its box's min corner at `corner`, in units: where it stands,
    /// which is its base's middle, where its model's origin is put, and what it carries at
    /// `scale`.
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

    /// The box that a decor standing at `at` and carrying `m` claims: every cell in it is the
    /// decor's.
    pub fn claim(&self, at: [f64; 3], m: &decor::DecorMeta) -> Aabb {
        let [x, y, z] = self.sides(m.variant, m.turn);
        let min = [at[0] - x / 2.0, at[1] - y / 2.0, at[2]];
        Aabb {
            min,
            max: [min[0] + x, min[1] + y, min[2] + z],
        }
    }

    /// Where its variants are numbered from as draw groups, as the GPU's
    /// decor numbers them (pxl-content generation.rs `first_group`).
    pub fn first_group(&self) -> u32 {
        let kind = decor::DECORS.iter().take_while(|d| d.id != self.id).count();
        generation::first_group(&DECORS, kind as u32)
    }

    /// The decor an id names, or none for an id the catalog does not give out.
    pub fn find(id: AssetId) -> Option<&'static decor::Decor> {
        decor::DECORS.iter().find(|d| d.id == id)
    }

    /// The decor of an id known to be one, as a tool holds it.
    pub fn of(id: AssetId) -> &'static decor::Decor {
        use xpute_kit::status::bug::OrBug;
        decor::Decor::find(id).or_bug(Errno::ENOTRECOVERABLE)
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

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct TileOrigin {
    pub x: u32,
    pub y: u32,
}

/// The pack tile a face wears; None where the pack has no image.
fn tile_of(cube: AssetId, face: CubeFace) -> Option<u32> {
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
pub fn shipped_tile_mean(cube: AssetId, face: CubeFace) -> Option<[u8; 3]> {
    PACK.means.get(tile_of(cube, face)? as usize).copied()
}

/// A sentinel's tile's mean color, as `shipped_tile_mean` gives a cube's.
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
pub fn shipped_texture_tile(cube: AssetId, face: CubeFace) -> Option<TileOrigin> {
    tile_of(cube, face).map(origin)
}

/// A sentinel's tile's pixel origin, as `shipped_texture_tile` gives a cube's.
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
