// pxl-render/frame/shader/word.rs

//! What the host lays out and the shaders read, stated once: compiled into
//! the crate, and read by build.rs, which hands each to every shader
//! (constant.wgsl's tail), a uniform's floats as its vec4s, and each
//! uniform's fields as their accessors to the part that binds it.

/// A draw's uniform (frame/draw.rs): its translation and what its pipeline
/// reads past it.
pub const DRAW_FLOATS: usize = 8;

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
        /// The sun's light box from the eye's frame (gpu/shadow.rs `aim`),
        /// how much of the sun a shadow takes, none where no map is drawn,
        /// the normal offset a fragment takes, a texel at the map's reach,
        /// and the share of the map's half-width the casters reach, where
        /// the shadows end.
        LIGHT_VIEW_PROJ: Mat4 = 32,
        SHADOW_STRENGTH: F32 = 48,
        SHADOW_TEXEL: F32 = 49,
        SHADOW_RIM: F32 = 50,
        /// Whether the camera gizmo is drawn, its tint on the rays' cells
        /// with it (screen_lit.wgsl `cell_share`).
        GIZMO: F32 = 51,
        /// The canvas's device pixels, where `VIEWPORT` is the world's: what
        /// is drawn over the world, after the finish, is laid out in them.
        SCREEN: Vec2 = 52,
        /// Where this frame's pixels sample the world, off their middles, in
        /// clip units, X right and Y up: pxl-spec-core `eye::JITTER`'s offset
        /// `j`, in drawn pixels with Y down, as (2 j.x / width, -2 j.y /
        /// height) of `VIEWPORT`. What is drawn into the world's color is
        /// moved against it (frame.wgsl `jittered`), the sky aimed along it.
        JITTER: Vec2 = 54,
    }
}
pub const FRAME_FLOATS: usize = floats(frame::FIELDS);

uniform! {
    /// The sky's own (sky_word.wgsl): the ray through each point of the
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
    /// The air's (atmos_medium.wgsl, gpu/atmosphere.rs). Floats 8 to 10 are
    /// free.
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
        /// Where the eye's rays end, in units (pxl-spec-core `eye::REACH`):
        /// past it the land is the Earth's picture whole.
        DRAWN: F32 = 35,
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
    /// The cloud sheets' (cloud.wgsl, gpu/cloud.rs).
    clouds {
        /// How much of the clouds the kernel shows (pxl-kernel
        /// domain/scene/environment/cloud.rs `clouds_shown`).
        SHOWN: F32 = 0,
        /// How far the eye draws, in km: past it no sheet is drawn.
        REACH: F32 = 1,
        /// The lattice's units around; the cells the wind has moved it
        /// under the map; the field's lattice unit the eye is over and its
        /// share of one (gpu/cloud.rs `field`).
        COUNT: U32 = 2,
        DRIFT: Vec2 = 4,
        BASE: Vec2u = 6,
        FRAC: Vec2 = 8,
    }
}
pub const CLOUDS_FLOATS: usize = floats(clouds::FIELDS);

uniform! {
    /// The stars' (star.wgsl, gpu/star.rs).
    stars {
        VIEW_PROJ: Mat4 = 0,
        SKY: Mat4 = 16,
        /// Sized in CSS pixels so a dense screen does not shrink them.
        DENSITY: F32 = 32,
        INTENSITY: F32 = 33,
    }
}
pub const STARS_FLOATS: usize = floats(stars::FIELDS);

uniform! {
    /// The screen's occlusion's (ao_view.wgsl, gpu/ao.rs): the view's
    /// projection and its inverse, and the half-resolution target's size.
    ao {
        PROJ: Mat4 = 0,
        INV_PROJ: Mat4 = 16,
        HALF: Vec2 = 32,
    }
}
pub const AO_FLOATS: usize = floats(ao::FIELDS);

uniform! {
    /// A marker set's (marker.wgsl, gpu/pass/marker.rs): the view, the
    /// projection, the set's translation and its pixels a unit, and the
    /// markers before the set's first.
    marker {
        VIEW: Mat4 = 0,
        PROJ: Mat4 = 16,
        TRANSLATION: Vec3 = 32,
        PX_PER_UNIT: F32 = 35,
        BEFORE: F32 = 36,
    }
}
pub const MARKER_FLOATS: usize = floats(marker::FIELDS);

/// A marker's instance and a puck zone's box's, in floats, as the kernel
/// lays them (pxl-kernel domain/scene/marker/set.rs, location_puck.rs) and
/// marker.wgsl and puck_zone.wgsl read them.
pub const MARKER_INSTANCE_FLOATS: u32 = 11;
pub const PUCK_ZONE_INSTANCE_FLOATS: u32 = 8;

/// A star's record as the sky reads it (gpu/star.rs, star.wgsl): its
/// direction and flux, then its colour and a pad.
pub const STAR_RECORD_FLOATS: usize = 8;

/// The raster's view's words (gpu/ray/raster.rs, raster.wgsl `Raster`),
/// which build.rs holds the struct to.
pub const RASTER_FLOATS: usize = 24;

/// The occlusion's passes' resolution, the screen's over this (gpu/ao.rs):
/// the downsample keeps one depth of each 2×2 and the upsample blends the 2×2
/// about each pixel (ao_downsample.wgsl, ao_upsample.wgsl), so it is two.
pub const AO_DOWNSCALE: u32 = 2;

/// N8AO's blue noise a side (gpu/ao.rs), which a pixel reads tiled.
pub const AO_NOISE_SIDE: u32 = 128;

/// The air's tables' sizes (gpu/atmosphere.rs `SIZES`, atmos_medium.wgsl).
pub const TRANSMITTANCE_SIZE: (u32, u32) = (256, 64);
pub const MULTISCATTER_SIZE: (u32, u32) = (32, 32);
pub const SKY_VIEW_SIZE: (u32, u32) = (192, 108);
pub const IRRADIANCE_SIZE: (u32, u32) = (64, 16);

/// The decor's draw groups (gpu/pass/decor.rs), each an impostor's fit
/// (impostor.wgsl `fits`).
pub const DECOR_GROUPS: usize = 64;

/// A sheet's shapes (gpu/sheet.rs, sheet.wgsl `fs`).
pub const SHEET_FILL: u32 = 0;
pub const SHEET_STROKE: u32 = 1;
pub const SHEET_TRIANGLE: u32 = 2;
pub const SHEET_TEXT: u32 = 3;
pub const SHEET_HALO: u32 = 4;

/// A box's steps a cell across (mesh/ghost.rs, box.wgsl); its z is held in
/// its mesh's own.
pub const BOX_XY_STEPS: u32 = 256;

/// A box's vertices, two triangles each of its six faces (pxl-spec-core
/// raster.rs `corner`, box.wgsl).
pub const BOX_VERTICES: u32 = 36;

/// The uniforms whose fields are stated here.
pub const UNIFORMS: [(&str, &[Field]); 7] = [
    (frame::NAME, frame::FIELDS),
    (sky::NAME, sky::FIELDS),
    (atmos::NAME, atmos::FIELDS),
    (clouds::NAME, clouds::FIELDS),
    (stars::NAME, stars::FIELDS),
    (ao::NAME, ao::FIELDS),
    (marker::NAME, marker::FIELDS),
];

/// The cloud sheets' tile (gpu/cloud.rs), one fetch a pixel and a mip chain
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
/// The Moon's angular radius: its equatorial radius, 1738.1 km, over its
/// mean distance, 384,400 km.
pub const MOON_ANGULAR_RADIUS: f32 = 1738.1 / 384_400.0;
/// The Moon's geometric albedo.
pub const MOON_ALBEDO: f32 = 0.12;
/// The map's mean reflectance over the disc, as download_moon prints it from
/// the raw map (word.test.rs holds the shipped map to it): the map is scaled
/// by `MOON_ALBEDO` over it.
pub const MOON_MAP_MEAN: f32 = 0.2459;

/// The air's froxels a side and deep (gpu/atmosphere.rs `froxel`):
/// Hillaire 2020's aerial-perspective volume, 32³, as Unreal's
/// SkyAtmosphere sizes it.
pub const AIR_FROXELS: u32 = 32;

/// Half an 8-bit step of an opacity: a cloud sheet's under it moves no
/// pixel by a step, so it is not drawn.
pub const HALF_STEP: f32 = 0.5 / 255.0;

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

#[cfg(test)]
#[path = "word.test.rs"]
mod test;
