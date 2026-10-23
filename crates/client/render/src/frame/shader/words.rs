// pxl-render/frame/shader/words.rs

//! What the host lays out and the shaders read, stated once: compiled into
//! the crate, and read by build.rs, which hands each to every shader
//! (constant.wgsl's tail), a uniform's floats as its vec4s, and each
//! uniform's fields as their accessors to the part that binds it.

/// A draw's uniform (frame/draw.rs): its translation and what its pipeline
/// reads past it.
pub const DRAW_FLOATS: usize = 8;
/// The screen's occlusion's (ao_view.wgsl, gpu/ao.rs).
pub const AO_FLOATS: usize = 36;

/// What a uniform's field is to the shader that reads it off the vec4s.
#[derive(Clone, Copy)]
pub enum Kind {
    F32,
    U32,
    Vec2,
    Vec2u,
    Vec3,
    Vec4i,
    Vec4u,
    /// Three columns, a vec4 each, their `w` unread.
    Mat3,
    Mat4,
}

impl Kind {
    /// The floats it reaches past its first.
    pub const fn floats(self) -> usize {
        match self {
            Kind::F32 | Kind::U32 => 1,
            Kind::Vec2 | Kind::Vec2u => 2,
            Kind::Vec3 => 3,
            Kind::Vec4i | Kind::Vec4u => 4,
            Kind::Mat3 => 11,
            Kind::Mat4 => 16,
        }
    }
}

/// A uniform's field: its name, which the shaders read it by
/// (`<uniform>_<name>()`, build.rs `accessors`), its first float, and what
/// it is.
#[derive(Clone, Copy)]
pub struct Field {
    pub name: &'static str,
    pub at: usize,
    pub kind: Kind,
}

/// The floats a uniform takes: as far as its fields reach, whole vec4s.
pub const fn floats(fields: &[Field]) -> usize {
    let mut end = 0;
    let mut k = 0;
    while k < fields.len() {
        let reach = fields[k].at + fields[k].kind.floats();
        end = if reach > end { reach } else { end };
        k += 1;
    }
    end.div_ceil(4) * 4
}

/// A uniform's fields, each a float offset the host writes at, and `FIELDS`,
/// which build.rs writes each one's accessor from into the part that binds
/// the uniform's words (`<uniform>_words`): the one statement of its layout.
macro_rules! uniform {
    ($(#[$doc:meta])* $uniform:ident { $($(#[$field_doc:meta])* $name:ident: $kind:ident = $at:literal,)* }) => {
        $(#[$doc])*
        pub mod $uniform {
            use super::{Field, Kind};
            pub const NAME: &str = stringify!($uniform);
            $($(#[$field_doc])* pub const $name: usize = $at;)*
            pub const FIELDS: &[Field] = &[$(Field { name: stringify!($name), at: $name, kind: Kind::$kind }),*];
        }
    };
}

uniform! {
    /// The frame's (frame.wgsl), which every world pass reads. It holds no
    /// light: every light is the air's.
    frame {
        VIEW_PROJ: Mat4 = 0,
        CAMERA_POS: Vec3 = 16,
        EDGE_STRENGTH: F32 = 19,
        VIEWPORT: Vec2 = 20,
        SCREEN_AO: F32 = 22,
        VIGNETTE: F32 = 23,
        /// The eye's place on the map, Mercator's longitude and ordinate,
        /// and the map's radius in units.
        MAP: Vec3 = 24,
        CELL: F32 = 27,
        LOW_SPEC: F32 = 28,
        EDGE_WIDTH: F32 = 29,
        RIM_INTENSITY: F32 = 30,
        CORNER_AO: F32 = 31,
    }
}
pub const FRAME_FLOATS: usize = floats(frame::FIELDS);

uniform! {
    /// The sky's own (sky_words.wgsl): the ray through each point of the
    /// screen (gpu/renderer.rs `put_ray`), the celestial pole, and whether
    /// the Moon's face has arrived.
    sky {
        RAY: Mat3 = 0,
        POLE: Vec3 = 12,
        MAPPED: F32 = 15,
    }
}
pub const SKY_FLOATS: usize = floats(sky::FIELDS);

uniform! {
    /// The air's (atmos_medium.wgsl, gpu/atmosphere.rs). Floats 8 to 10 and
    /// 35 are free.
    atmos {
        RAYLEIGH_SCATTERING: Vec3 = 0,
        BOTTOM_RADIUS: F32 = 3,
        OZONE_ABSORPTION: Vec3 = 4,
        TOP_RADIUS: F32 = 7,
        RAYLEIGH_SCALE_HEIGHT: F32 = 11,
        SUN_DIR: Vec3 = 12,
        MIE_SCATTERING: F32 = 15,
        CAMERA_ALTITUDE: F32 = 16,
        KM_PER_U: F32 = 17,
        MIE_EXTINCTION: F32 = 18,
        MIE_SCALE_HEIGHT: F32 = 19,
        MIE_G: F32 = 20,
        OZONE_CENTER: F32 = 21,
        OZONE_HALF_WIDTH: F32 = 22,
        GROUND_ALBEDO: F32 = 23,
        SUN_ANGULAR_RADIUS: F32 = 24,
        EXPOSURE: F32 = 25,
        NIGHT_STOPS: F32 = 26,
        SHOWN_FALL: F32 = 27,
        MOON_DIR: Vec3 = 28,
        MOON_LIGHT: F32 = 31,
        /// As it reaches the air, against the sun's white.
        MOON_COLOR: Vec3 = 32,
        /// Lights the ground, never drawn: drawn, the sky went pale blue.
        NIGHT_SKY: Vec3 = 36,
        /// The Earth's face under the eye: at sea level the terrain floated on it.
        LAND_ALTITUDE_KM: F32 = 39,
        /// A lit city's radiance where the picture holds all of it.
        CITY_LIGHT: Vec3 = 40,
        /// In units.
        EARTH_REACH: F32 = 43,
        /// The south-west tile, tiles across the window and around the
        /// world; then a tile's layer plus one in `EARTH_TILE_BITS`, row by
        /// row, zero for the global picture.
        EARTH_WINDOW: Vec4i = 44,
        EARTH_TILES: Vec4u = 48,
    }
}
pub const ATMOS_FLOATS: usize = floats(atmos::FIELDS);

uniform! {
    /// The cloud sheets' (clouds.wgsl, gpu/clouds.rs); their ray is the sky's.
    clouds {
        /// Faded over the upper half of the climb to where the clouds are
        /// gone (look/cloud.rs `shown`).
        SHOWN: F32 = 0,
        /// How far the eye observes, in km: past it no sheet is drawn.
        REACH: F32 = 1,
        /// The lattice's units around; the cells the wind has moved it
        /// under the map; the field's lattice unit the eye is over and its
        /// share of one (gpu/clouds.rs `field`).
        COUNT: U32 = 2,
        DRIFT: Vec2 = 4,
        BASE: Vec2u = 6,
        FRAC: Vec2 = 8,
    }
}
pub const CLOUDS_FLOATS: usize = floats(clouds::FIELDS);

uniform! {
    /// The stars' (stars.wgsl, gpu/stars.rs).
    stars {
        VIEW_PROJ: Mat4 = 0,
        SKY: Mat4 = 16,
        /// Sized in CSS pixels so a dense screen does not shrink them.
        DENSITY: F32 = 32,
        INTENSITY: F32 = 33,
    }
}
pub const STARS_FLOATS: usize = floats(stars::FIELDS);

/// The uniforms whose fields are stated here.
pub const UNIFORMS: [(&str, &[Field]); 5] = [
    (frame::NAME, frame::FIELDS),
    (sky::NAME, sky::FIELDS),
    (atmos::NAME, atmos::FIELDS),
    (clouds::NAME, clouds::FIELDS),
    (stars::NAME, stars::FIELDS),
];

/// The cloud sheets' tile (look/cloud.rs), one fetch a pixel and a mip chain
/// in place of eight octaves of hashes: 32 lattice units is about 250 km,
/// further than the layer is drawn, and the lattice wraps with the tile.
pub const CLOUD_TILE_UNITS: u32 = 32;
pub const CLOUD_TILE_TEXELS: u32 = 1024;

/// The bits a held earth tile's layer takes in the air's window words
/// (gpu/atmosphere.rs `EARTH_WINDOW`).
pub const EARTH_TILE_BITS: u32 = 4;

/// The Moon's near side as download_moon cuts it, a side in texels; the
/// disc's radius is half of it.
pub const MOON_TEXELS: u32 = 512;

/// The air's froxels a side and deep (gpu/atmosphere.rs `froxel`):
/// Hillaire 2020's aerial-perspective volume, 32³, as Unreal's
/// SkyAtmosphere sizes it.
pub const AIR_FROXELS: u32 = 32;

/// A layer past the last: the face's swatch alone.
pub const NO_LAYER: u32 = 255;

/// A face's top, side and bottom images, a slot of the pack each, in
/// `SLOT_BITS` each, in that order; `NO_SLOT` until an image arrives, past
/// any array WebGPU gives (maxTextureArrayLayers is 256).
pub const SLOT_BITS: u32 = 10;
pub const NO_SLOT: u32 = (1 << SLOT_BITS) - 1;
pub const IMAGE_TOP: u32 = 0;
pub const IMAGE_SIDE: u32 = 1;
pub const IMAGE_BOTTOM: u32 = 2;
