// pxl-spec/bake/publication.spec.rs
//
// GENERATED from spec/spec/bake/publication.json (pxl-tool spec/bake.rs)

/// The format each bake writes now, which a published name begins with.
/// It is bumped by a change to how the client reads a tile, or to what a
/// tile guarantees it.
pub struct Format {
    pub terrain: u32,
    pub place: u32,
    pub division: u32,
    pub earth: u32,
}

pub const FORMAT: Format = Format {
    terrain: 12,
    place: 10,
    division: 9,
    earth: 2,
};

/// The longest version a declaration may name, in bytes: one segment of a tile's URL,
/// before its name (pxl-spec bake/source.rs `is_version`).
pub const VERSION_BYTES: usize = 32;

/// What a version is written in: one segment of a tile's URL, with no character it would
/// have to escape.
pub const VERSION_CHARS: &str = "ABCDEFGHIJKLMNOPQRSTUVWXYZabcdefghijklmnopqrstuvwxyz0123456789._-";

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

/// The Earth's picture whole, in place of a tile's name under the earth
/// layer's version: the sphere's, no tile of the pyramid.
pub const TILE_SPHERE: &str = "sphere";

/// The base layers a version is published for: the kinds of `FORMAT`,
/// numbered in their order.
#[expect(non_camel_case_types, reason = "generated from the spec")]
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
#[repr(u32)]
pub enum BaseLayer {
    terrain,
    place,
    division,
    earth,
}

impl BaseLayer {
    pub const ALL: [BaseLayer; 4] = [
        BaseLayer::terrain,
        BaseLayer::place,
        BaseLayer::division,
        BaseLayer::earth,
    ];

    /// The layer a number names, or none for a number nobody gave out.
    pub fn of(number: u32) -> Option<BaseLayer> {
        BaseLayer::ALL.into_iter().find(|k| *k as u32 == number)
    }

    pub fn name(self) -> &'static str {
        match self {
            BaseLayer::terrain => "terrain",
            BaseLayer::place => "place",
            BaseLayer::division => "division",
            BaseLayer::earth => "earth",
        }
    }
}

impl Format {
    /// `FORMAT[layer]`.
    pub fn of(&self, layer: BaseLayer) -> u32 {
        match layer {
            BaseLayer::terrain => self.terrain,
            BaseLayer::place => self.place,
            BaseLayer::division => self.division,
            BaseLayer::earth => self.earth,
        }
    }
}
