// pxl-spec/bake/publication.spec.rs
//
// GENERATED from spec/spec/bake/publication.json5 (xpute-gen data.rs)
//
// The format each kind is baked and read in, the code's. Which version of
// each is served is the bucket's alone: its `source`, which the client reads
// at boot and script/publish.py writes, so a publish changes nothing here.

/// The base layers a version is published for, each a kind a baked page is published under.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
#[repr(u32)]
pub enum BaseKind {
    Terrain = 0,
    Place = 1,
    Division = 2,
    Imagery = 3,
}

impl BaseKind {
    /// Every member, in its source's own order.
    pub const ALL: [BaseKind; 4] = [
        BaseKind::Terrain,
        BaseKind::Place,
        BaseKind::Division,
        BaseKind::Imagery,
    ];

    /// The member `val` is, or none for a number no member has.
    pub const fn of(val: u32) -> Option<BaseKind> {
        match val {
            0 => Some(BaseKind::Terrain),
            1 => Some(BaseKind::Place),
            2 => Some(BaseKind::Division),
            3 => Some(BaseKind::Imagery),
            _ => None,
        }
    }

    /// The member's name, as its source spells it.
    pub const fn name(self) -> &'static str {
        match self {
            BaseKind::Terrain => "terrain",
            BaseKind::Place => "place",
            BaseKind::Division => "division",
            BaseKind::Imagery => "imagery",
        }
    }
}

/// The format each bake writes now, which a published name begins with.
/// It is bumped by a change to how the client reads a tile, or to what a
/// tile guarantees it.
/// By `BaseKind`'s number.
pub const FORMAT: [u32; 4] = [12, 10, 9, 2];

/// What a version is written in: one segment of a tile's URL, with no character it would
/// have to escape.
pub const VERSION_CHARS: &str = "ABCDEFGHIJKLMNOPQRSTUVWXYZabcdefghijklmnopqrstuvwxyz0123456789._-";

/// The longest version a declaration may name, in bytes: one segment of a tile's URL,
/// before its name (pxl-spec bake/source.rs `is_version`).
pub const VERSION_SZ: usize = 32;

/// The imagery's global image, in place of a tile's name under the imagery
/// layer's version: no tile of the pyramid.
pub const TILE_GLOBAL: &str = "global";

/// A kind's object in a published set, the one place its segments are
/// ordered: the version its source declares, then the tile's name. A version
/// is a folder its tiles go up in whole (script/publish.py), so it comes
/// first. Under its kind it is a tile's address below the tile root
/// (route.json5 `TILE_ROOT`), and its key in the bucket.
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
