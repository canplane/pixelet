// pxl-spec/bake/publication.spec.rs
//
// GENERATED from spec/spec/bake/publication.json (xpute-gen data.rs)
//
// The format each kind is baked and read in, the code's. Which version of
// each is served is the bucket's alone: its `source`, which the client reads
// at boot and script/publish.py writes, so a publish changes nothing here.

// Names as the spec spells them, so one record has one name on every side.
// A table is generated whole, whether or not this crate calls an entry.
#![allow(
    non_upper_case_globals,
    non_snake_case,
    non_camel_case_types,
    dead_code,
    reason = "generated from the spec"
)]

/// The format each bake writes now, which a published name begins with.
/// It is bumped by a change to how the client reads a tile, or to what a
/// tile guarantees it.
pub mod FORMAT {
    pub const terrain: u32 = 12;
    pub const place: u32 = 10;
    pub const division: u32 = 9;
    pub const earth: u32 = 2;
}

/// What a version is written in: one segment of a tile's URL, with no character it would
/// have to escape.
pub const VERSION_CHARS: &str = "ABCDEFGHIJKLMNOPQRSTUVWXYZabcdefghijklmnopqrstuvwxyz0123456789._-";

/// The longest version a declaration may name, in bytes: one segment of a tile's URL,
/// before its name (pxl-spec bake/source.rs `is_version`).
pub const VERSION_BYTES: usize = 32;

/// The Earth's picture whole, in place of a tile's name under the earth
/// layer's version: the sphere's, no tile of the pyramid.
pub const TILE_SPHERE: &str = "sphere";

/// The base layers a version is published for: the kinds of `FORMAT`, numbered in their order.
pub const BASE_LAYERS: &[&str] = &["terrain", "place", "division", "earth"];
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
#[repr(u32)]
pub enum BaseLayer {
    terrain = 0,
    place = 1,
    division = 2,
    earth = 3,
}

impl BaseLayer {
    /// Every member, in its source's own order.
    pub const ALL: [BaseLayer; 4] = [
        BaseLayer::terrain,
        BaseLayer::place,
        BaseLayer::division,
        BaseLayer::earth,
    ];

    /// The member `val` is, or none for a number no member has.
    pub const fn of(val: u32) -> Option<BaseLayer> {
        match val {
            0 => Some(BaseLayer::terrain),
            1 => Some(BaseLayer::place),
            2 => Some(BaseLayer::division),
            3 => Some(BaseLayer::earth),
            _ => None,
        }
    }

    /// The member's name, as its source spells it.
    pub const fn name(self) -> &'static str {
        match self {
            BaseLayer::terrain => "terrain",
            BaseLayer::place => "place",
            BaseLayer::division => "division",
            BaseLayer::earth => "earth",
        }
    }
}

/// A kind's object in a published set, the one place its segments are
/// ordered: the version its source declares, then the tile's name. A version
/// is a folder its tiles go up in whole (script/publish.py), so it comes
/// first. Under its kind it is a tile's address below the tile root
/// (route.json `TILE_ROOT`), and its key in the bucket.
/// Written into `w`, so a key is made where it is wanted, not allocated.
pub fn tile_object(
    w: &mut impl core::fmt::Write,
    version: &str,
    name: impl core::fmt::Display,
) -> core::fmt::Result {
    write!(w, "{version}/{name}")
}

/// `tile_object`'s version and name, none for what it never writes.
pub fn tile_object_parts(object: &str) -> Option<(&str, &str)> {
    let mut segment = object.split('/');
    let (version, name) = (segment.next()?, segment.next()?);
    segment.next().is_none().then_some((version, name))
}

impl BaseLayer {
    /// The format the layer is baked and read in, `FORMAT[layer]`.
    pub const fn format(self) -> u32 {
        match self {
            BaseLayer::terrain => FORMAT::terrain,
            BaseLayer::place => FORMAT::place,
            BaseLayer::division => FORMAT::division,
            BaseLayer::earth => FORMAT::earth,
        }
    }
}
