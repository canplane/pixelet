// pxl-render/frame/shader/word.rs

//! The layouts the CPU writes and the shaders read, stated once. The crate
//! compiles this file, and build.rs includes it as a module of its own. From
//! it build.rs writes the constants the shaders name, and each uniform's size
//! in vec4s, into constant.wgsl, which every shader holds. It also writes each
//! uniform's fields as accessor functions into each shader that binds the
//! uniform.

/// The WGSL type of a uniform's field, which its accessor reads off the
/// uniform's vec4s.
#[derive(Clone, Copy)]
pub enum Kind {
    F32,
    U32,
    Vec2,
    Vec2u,
    Vec3,
    Vec3i,
    Vec4i,
    Vec4u,
    /// Three columns of one vec4 each, whose `w` lanes are not read.
    Mat3,
    Mat4,
}

impl Kind {
    /// The floats a field of this type spans, from its first to its last
    /// read.
    pub const fn floats(self) -> usize {
        match self {
            Kind::F32 | Kind::U32 => 1,
            Kind::Vec2 | Kind::Vec2u => 2,
            Kind::Vec3 | Kind::Vec3i => 3,
            Kind::Vec4i | Kind::Vec4u => 4,
            Kind::Mat3 => 11,
            Kind::Mat4 => 16,
        }
    }
}

/// A uniform's field. The shaders read it through an accessor named
/// `<uniform>_<name>()`, in lower case (build.rs `accessors`). `at` is the
/// index of its first float in the uniform.
#[derive(Clone, Copy)]
pub struct Field {
    pub name: &'static str,
    pub at: usize,
    pub kind: Kind,
}

/// The floats a uniform takes: up to the end of its last field, rounded up
/// to whole vec4s.
pub const fn floats(fields: &[Field]) -> usize {
    let mut end = 0;
    let mut k = 0;
    while k < fields.len() {
        let field_end = fields[k].at + fields[k].kind.floats();
        end = if field_end > end { field_end } else { end };
        k += 1;
    }
    end.div_ceil(4) * 4
}

/// States a uniform's layout, once, as a module. Each field is a constant,
/// the float offset the CPU writes the field at. `FIELDS` lists the same
/// fields for build.rs, which writes an accessor for each into every shader
/// that binds the uniform as `<uniform>_words` or reads it from a list
/// through `<uniform>_word`.
macro_rules! uniform {
    ($(#[$doc:meta])* $uniform:ident { $($(#[$field_doc:meta])* $name:ident: $kind:ident = $at:expr,)* }) => {
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
    /// The fields every draw's uniform holds, whatever draws it. `Q` and `R`
    /// are its position as a Q3 and a remainder (gpu/draw.rs `Position`).
    /// `DRIFTS` is 1 for a cloud's draw, which drifts with the wind, and 0
    /// otherwise (`DrawList::draw_instances`). A kind of draw that reads more
    /// has a uniform of its own below, with these fields at the same offsets.
    draw {
        Q: Vec3i = 0,
        R: Vec3 = 8,
        DRIFTS: F32 = 11,
    }
}
pub const DRAW_FLOATS: usize = floats(draw::FIELDS);

uniform! {
    /// The uniform of a decor draw, of the model tier and of the impostors
    /// (instance.wgsl): where the draw's lanes lie, its opacity, the
    /// impostor's atlas layer, and the first word of its index list.
    instance {
        Q: Vec3i = super::draw::Q,
        OFF: U32 = 3,
        STRIDE: U32 = 4,
        OPACITY: F32 = 5,
        LAYER: U32 = 6,
        IDX: U32 = 7,
        R: Vec3 = super::draw::R,
        DRIFTS: F32 = super::draw::DRIFTS,
    }
}

uniform! {
    /// A character's uniform (skinned_place.wgsl): its turn about Z, its
    /// opacity and its scale.
    skinned {
        Q: Vec3i = super::draw::Q,
        ROT: F32 = 3,
        OPACITY: F32 = 4,
        SCALE: F32 = 5,
        R: Vec3 = super::draw::R,
    }
}

uniform! {
    /// The location indicator's uniform (location.wgsl, location.slot.wgsl,
    /// gpu/pass/location.rs): the beacon's spin about Z, the zone's color,
    /// and whether the beacon takes its night look.
    location {
        Q: Vec3i = super::draw::Q,
        SPIN: F32 = 3,
        COLOR: Vec3 = 4,
        NIGHT: F32 = 7,
        R: Vec3 = super::draw::R,
    }
}

uniform! {
    /// The uniform of the hover ghost's boxes (box.wgsl): their opacity, the
    /// side of a box's cell in units, and its height step.
    ghost_box {
        Q: Vec3i = super::draw::Q,
        OPACITY: F32 = 4,
        UNIT: F32 = 5,
        Z_STEP: F32 = 6,
        R: Vec3 = super::draw::R,
    }
}

uniform! {
    /// The frame's uniform (frame.wgsl), which every world pass reads. It
    /// holds no light's direction or color: the passes read every light from
    /// the air's uniform (`atmos`).
    frame {
        VIEW_PROJ: Mat4 = 0,
        CAMERA_POS: Vec3 = 16,
        EDGE_STRENGTH: F32 = 19,
        VIEWPORT: Vec2 = 20,
        VIGNETTE: F32 = 23,
        /// The eye's place on the map as Mercator's longitude and ordinate,
        /// then the map's radius in units.
        MAP: Vec3 = 24,
        LOW_SPEC: F32 = 28,
        RIM_INTENSITY: F32 = 30,
        CORNER_AO: F32 = 31,
        /// The light box's projection from the eye's frame (gpu/shadow.rs
        /// `aim`). `SHADOW_STRENGTH` is how much of the light a shadow takes
        /// away, and it is 0 when no shadow map is drawn. `SHADOW_TEXEL` is
        /// how far a fragment moves along its normal before it reads the map:
        /// one texel at the map's reach. The light is the sun's, or the
        /// moon's when `SHADOW_MOON` is 1 (shadow.wgsl `shadow_light`).
        LIGHT_VIEW_PROJ: Mat4 = 32,
        SHADOW_STRENGTH: F32 = 48,
        SHADOW_TEXEL: F32 = 49,
        /// 1 while the camera gizmo is shown: the camera grid drawn over the canvas
        /// (gpu/pass/debug.rs) and its tint on the rays' cells (screen_shade.wgsl
        /// `cell_share`).
        GIZMO: F32 = 51,
        /// The canvas's size in device pixels, where `VIEWPORT` is the size
        /// the world is drawn at. What is drawn over the world after the
        /// finish is laid out in these pixels.
        SCREEN: Vec2 = 52,
        /// 1 when the shadow map is cast from the moon, and 0 when it is
        /// cast from the sun (`LIGHT_VIEW_PROJ`).
        SHADOW_MOON: F32 = 54,
        /// For each axis of the light box's clip space, across, up and deep,
        /// the step in the world that moves one clip unit along it. Each step
        /// is a row of `LIGHT_VIEW_PROJ` over its squared length, which holds
        /// because the box's projection is orthographic (shadow.wgsl
        /// `light_slope`).
        LIGHT_STEPS: Mat3 = 56,
        /// The eye's position as a Q3 (gpu/draw.rs `Position`). Every draw's
        /// position is taken relative to it (frame.wgsl `from_eye`), and
        /// `VIEW_PROJ` puts the eye at its origin.
        EYE: Vec3i = 68,
        /// How far off their middles this frame's pixels sample the world, in
        /// clip units with X right and Y up. It is pxl-engine `eye::JITTER`'s
        /// offset `j`, which is in drawn pixels with Y down, taken as
        /// (2 j.x / width, -2 j.y / height) of `VIEWPORT`. What is drawn into
        /// the world's color is moved against it (frame.wgsl `jittered`), and
        /// the sky's rays are aimed along it. The frame counts as changed
        /// when a word before this one changes, and a change in this word or
        /// in those after it does not count (gpu/renderer.rs `frame`).
        JITTER: Vec2 = 72,
        /// How far the wind has carried the cloud field, X and Y in units
        /// (pxl-content cloud.rs `drift`), as a `Position`: its Q3 as an
        /// i32's bits, then its remainder. A cloud's draw holds its place on
        /// the field's map and is drawn moved back by the drift (instance.wgsl
        /// `draw_translation`). These words lie after `JITTER`, so a frame in
        /// which only the drift moved does not count as changed. The clouds
        /// mark the tiles they left and the tiles they now cover instead
        /// (gpu/pass/mark.rs).
        DRIFT_Q: Vec2u = 74,
        DRIFT_R: Vec2 = 76,
    }
}
pub const FRAME_FLOATS: usize = floats(frame::FIELDS);

uniform! {
    /// The sky's uniform (sky_word.wgsl). `RAY` gives the ray through each
    /// point of the screen (gpu/sky.rs `put_ray`), `POLE` is the
    /// celestial pole, and `MAPPED` is 1 once the moon's face has arrived.
    sky {
        RAY: Mat3 = 0,
        POLE: Vec3 = 12,
        MAPPED: F32 = 15,
    }
}
pub const SKY_FLOATS: usize = floats(sky::FIELDS);

uniform! {
    /// The air's uniform (atmos_medium.wgsl, gpu/atmosphere.rs). Floats 9
    /// and 10 are unused.
    atmos {
        RAYLEIGH_SCATTERING: Vec3 = 0,
        BOTTOM_RADIUS: F32 = 3,
        OZONE_ABSORPTION: Vec3 = 4,
        TOP_RADIUS: F32 = 7,
        /// How far a level ray from the ground runs to the top of the air, in
        /// km. It is the leg of the right triangle whose hypotenuse is
        /// `TOP_RADIUS` and whose other leg is `BOTTOM_RADIUS`
        /// (atmos_medium.wgsl `transmittance_uv`).
        HORIZON_TO_TOP: F32 = 8,
        RAYLEIGH_SCALE_HEIGHT: F32 = 11,
        SUN_DIR: Vec3 = 12,
        MIE_SCATTERING: F32 = 15,
        CAMERA_ALT: F32 = 16,
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
        /// The moon's light as it reaches the air, relative to the sun's,
        /// which is taken as white.
        MOON_COLOR: Vec3 = 32,
        /// How far the eye's rays run, in units (pxl-engine `eye::FAR`).
        /// Past it the land is drawn from the Earth's picture alone.
        DRAWN: F32 = 35,
        /// The night sky's light. It lights the ground and is never drawn
        /// itself.
        NIGHT_SKY: Vec3 = 36,
        /// The altitude of the land under the eye, in km.
        LAND_ALT_KM: F32 = 39,
        /// A lit city's radiance where the Earth's picture shows city
        /// lights at full strength.
        CITY_LIGHT: Vec3 = 40,
        /// How far from the eye the Earth's tiles are drawn before the global
        /// picture takes over, in units (pxl-spec bake/tile/earth.rs
        /// `EARTH_DIST`).
        EARTH_DIST: F32 = 43,
        /// The window of Earth tiles the air reads. `EARTH_WINDOW` holds the
        /// south-west tile's column and row, the tiles across the window, and
        /// the tiles around the world. `EARTH_TILES` holds each tile's layer
        /// plus one in `EARTH_TILE_BITS` bits, row by row, and zero where the
        /// global picture stands in for the tile.
        EARTH_WINDOW: Vec4i = 44,
        EARTH_TILES: Vec4u = 48,
        /// How each light's sky view and its share of the air's volume were
        /// laid out (gpu/atmosphere.rs `prepare`), the sun's and then the
        /// moon's. A layout holds how far the horizon dips below the level,
        /// in radians, seen from the eye's radius the view was drawn at, then
        /// the near and far ends of the air's depth law there, in km
        /// (atmos_medium.wgsl `view_layout`). `SUN_REACHES` and
        /// `MOON_REACHES` are 1 when the light reaches the air by at least
        /// half a display level, and only then are the light's view and air
        /// drawn and read (`reaches`).
        SUN_LAYOUT: Vec3 = 52,
        SUN_REACHES: F32 = 55,
        MOON_LAYOUT: Vec3 = 56,
        MOON_REACHES: F32 = 59,
    }
}
pub const ATMOS_FLOATS: usize = floats(atmos::FIELDS);

uniform! {
    /// The cloud sheets' uniform (cloud.wgsl, gpu/cloud.rs).
    clouds {
        /// How much of the clouds the core shows (pxl-core
        /// domain/scene/environment/cloud.rs `clouds_shown`).
        SHOWN: F32 = 0,
        /// How far the eye draws, in km. No sheet is drawn past it.
        DRAW_DIST: F32 = 1,
        /// `CNT` is the lattice's units around the world. `DRIFT` is how
        /// many cells the wind has moved the lattice under the map. `BASE` is
        /// the lattice unit the eye is over, and `FRAC` is how far into that
        /// unit the eye is (gpu/cloud.rs `field`).
        CNT: U32 = 2,
        DRIFT: Vec2 = 4,
        BASE: Vec2u = 6,
        FRAC: Vec2 = 8,
    }
}
pub const CLOUDS_FLOATS: usize = floats(clouds::FIELDS);

uniform! {
    /// The stars' uniform (star.wgsl, gpu/star.rs).
    stars {
        VIEW_PROJ: Mat4 = 0,
        SKY: Mat4 = 16,
        /// The device pixels a CSS pixel covers. Stars are sized in CSS
        /// pixels, so a dense screen does not shrink them.
        DENSITY: F32 = 32,
        INTENSITY: F32 = 33,
    }
}
pub const STARS_FLOATS: usize = floats(stars::FIELDS);

uniform! {
    /// The uniform of the screen's ambient occlusion (ao_view.wgsl,
    /// gpu/ao.rs): the view's projection, its inverse, and the size of the
    /// half-resolution target.
    ao {
        PROJ: Mat4 = 0,
        INV_PROJ: Mat4 = 16,
        HALF: Vec2 = 32,
    }
}
pub const AO_FLOATS: usize = floats(ao::FIELDS);

uniform! {
    /// A marker set's uniform (marker.wgsl, gpu/pass/marker.rs): the view,
    /// the projection, the pixels a unit covers, and how many markers come
    /// before the set's first. Where each marker stands is in its instance
    /// (`MARKER_INSTANCE_WORDS`).
    marker {
        VIEW: Mat4 = 0,
        PROJ: Mat4 = 16,
        PX_PER_UNIT: F32 = 32,
        BEFORE: F32 = 33,
    }
}
pub const MARKER_FLOATS: usize = floats(marker::FIELDS);

/// The size of a marker's instance in words, and of a location zone's box in
/// floats, as the core lays them out (pxl-core domain/scene/marker/set.rs,
/// location.rs) and marker.wgsl and location_zone.wgsl read them. A marker's
/// first three words are the Q3 it stands at, and the rest are floats.
pub const MARKER_INSTANCE_WORDS: u32 = 11;
pub const LOCATION_ZONE_INSTANCE_FLOATS: u32 = 8;

/// The floats of a star's record (gpu/star.rs, star.wgsl): its direction
/// and flux, then its color and one float of padding.
pub const STAR_RECORD_FLOATS: usize = 8;

/// The words of the raster's view (gpu/ray/raster.rs, raster.wgsl
/// `Raster`). build.rs checks that the WGSL struct has this size.
pub const RASTER_FLOATS: usize = 28;

/// The factor the occlusion's passes divide the screen's resolution by
/// (gpu/ao.rs). It must be 2: the downsample keeps one depth of each 2×2
/// block, and the apply blends the 2×2 block about each pixel
/// (ao_downsample.wgsl, ao_apply.wgsl).
pub const AO_DOWNSCALE: u32 = 2;

/// The side in texels of N8AO's blue noise (gpu/ao.rs), which the pixels
/// read tiled.
pub const AO_NOISE_SIDE: u32 = 128;

/// The screen's tiles. A frame that is not drawn whole draws again only the
/// tiles it claims, as the light's map is cast tile by tile (pxl-engine
/// light.rs). Each tile is `TILE` drawn pixels a side. Its word is the
/// number of the frame it last changed in (`changed_at`), as a light tile's
/// word names a cast, and its age is the frames since then (`age`). The
/// words live on the GPU, as the light's do, because passes on the GPU mark
/// the tiles that changed, and words the CPU wrote each frame would undo
/// those marks. A tile stays claimed until its history has run through the
/// jitter's `JITTERS` frames (`DAMAGE_FRAMES`, gpu/resolve.rs).
pub mod screen {
    /// A tile's side in drawn pixels.
    pub const TILE: u32 = 32;
    /// The threads of a workgroup of the pass that lists the claimed tiles.
    pub const CAST_TILES_LANES: u32 = 64;
    /// The number of frames in which the jitter's sequence covers a pixel.
    pub const JITTERS: u32 = 8;

    /// The frames a screen tile stays claimed once it changes, the uniform's
    /// `damage_frames`. In the frame it changes, what earlier frames left is
    /// held within the colors this frame's pixels show. Then come
    /// `JITTERS - 1` still frames: the first one's color takes half of the
    /// result, and each after it takes its even share (screen_resolve.wgsl).
    /// Each of these frames samples at its own place in the jitter's
    /// sequence.
    pub const DAMAGE_FRAMES: u32 = JITTERS;

    /// The words of the uniform the passes over the tiles read
    /// (gpu/tile.rs `TileGrid::words`). The first four are u32s: the tiles
    /// across and down, `damage_frames`, and this frame's number. When the
    /// frame is drawn whole, `damage_frames` is u32::MAX, so every tile is
    /// claimed. The last four are f32 bits: the target's pixels across and
    /// down, how far a quad is grown past its tile, and a tile's side.
    pub const TILES_WORDS: usize = 8;

    /// The frames since a tile last changed, in the frame numbered `now`, from
    /// the tile's word `changed_at`. The difference wraps, so the frame number
    /// may wrap too. This and `in_damage` are the one rule a tile is claimed
    /// by: build.rs writes the same rule for the shaders as `tiles_age` and
    /// `tiles_in_damage` (tile_word.wgsl), and checks that the two agree.
    pub const fn age(now: u32, changed_at: u32) -> u32 {
        now.wrapping_sub(changed_at)
    }

    /// Whether a tile `age` frames old is claimed, where a tile stays claimed
    /// `damage_frames` frames once changed.
    pub const fn in_damage(age: u32, damage_frames: u32) -> bool {
        age < damage_frames
    }
}

/// The side in pixels of a square of the depth bound, which holds the least
/// and the most depth of each square for the propagation to read
/// (gpu/propagate.rs, screen_bound.wgsl, screen_propagate.wgsl).
pub const BOUND_STEP: u32 = 4;
/// The squares of the depth bound along a screen tile's side
/// (`screen::TILE`). Both passes over the bound take a tile as one workgroup
/// with one thread a square, so every square of a tile is set and read,
/// whatever the tile's side.
pub const BOUND_SQUARES: u32 = screen::TILE / BOUND_STEP;
const _: () = assert!(
    screen::TILE.is_multiple_of(BOUND_STEP),
    "a screen tile is whole squares of the depth's bound"
);

/// The side, in cells, of the square of the camera grid that one workgroup of
/// the pass finding where each cell's rays start covers, one thread a cell
/// (gpu/ray/cast.rs, cell.wgsl).
pub const CELL_SIDE: u32 = 8;

/// The sizes in texels of the air's lookup tables (gpu/atmosphere.rs
/// `SIZES`, atmos_medium.wgsl).
pub const TRANSMITTANCE_SIZE: (u32, u32) = (256, 64);
pub const MULTISCATTER_SIZE: (u32, u32) = (32, 32);
pub const SKY_VIEW_SIZE: (u32, u32) = (192, 108);
pub const IRRADIANCE_SIZE: (u32, u32) = (64, 16);

/// The side of the light's shadow map in texels (gpu/shadow.rs).
pub const SHADOW_SIZE: u32 = 2048;

/// The decor's draw groups, one a decor variant (gpu/pass/decor.rs). Each
/// group's impostor takes one layer of the impostor atlas and one fit in
/// impostor_place.wgsl's `fits`.
pub const DECOR_GROUPS: usize = 64;

/// The kinds of shape the atlas painter draws, by which atlas.wgsl `fs`
/// tells them apart (gpu/atlas.rs).
pub const ATLAS_FILL: u32 = 0;
pub const ATLAS_STROKE: u32 = 1;
pub const ATLAS_TRIANGLE: u32 = 2;
pub const ATLAS_TEXT: u32 = 3;
pub const ATLAS_HALO: u32 = 4;

/// The steps across a cell that a box's x and y are held in (mesh/ghost.rs,
/// box.wgsl). Its z is held in steps of its mesh's own.
pub const BOX_XY_STEPS: u32 = 256;

/// The vertices of a box: two triangles on each of its six faces
/// (pxl-engine raster.rs `corner`, box.wgsl).
pub const BOX_VERTICES: u32 = 36;

/// Every uniform stated in this file, each of which build.rs writes
/// accessors for.
pub const UNIFORMS: [(&str, &[Field]); 12] = [
    (draw::NAME, draw::FIELDS),
    (instance::NAME, instance::FIELDS),
    (skinned::NAME, skinned::FIELDS),
    (location::NAME, location::FIELDS),
    (ghost_box::NAME, ghost_box::FIELDS),
    (frame::NAME, frame::FIELDS),
    (sky::NAME, sky::FIELDS),
    (atmos::NAME, atmos::FIELDS),
    (clouds::NAME, clouds::FIELDS),
    (stars::NAME, stars::FIELDS),
    (ao::NAME, ao::FIELDS),
    (marker::NAME, marker::FIELDS),
];

/// The cloud sheets' tile in lattice units and in texels (gpu/cloud.rs).
/// 32 lattice units are about 250 km, further than the layer is drawn, and
/// the lattice wraps with the tile.
pub const CLOUD_TILE_UNITS: u32 = 32;
pub const CLOUD_TILE_TEXELS: u32 = 1024;

/// The bits each Earth tile's layer takes in the air's `EARTH_TILES` words
/// (gpu/atmosphere.rs).
pub const EARTH_TILE_BITS: u32 = 4;

/// The side in texels of the moon's near side as `make download-moon` cuts it.
/// The disc's radius is half of it.
pub const MOON_TEXELS: u32 = 512;
/// The moon's angular radius: its equatorial radius, 1738.1 km, over its
/// mean distance, 384,400 km.
pub const MOON_ANGULAR_RADIUS: f32 = 1738.1 / 384_400.0;
/// The moon's geometric albedo.
pub const MOON_ALBEDO: f32 = 0.12;
/// The moon map's mean reflectance over the disc, as `make download-moon` prints it
/// from the raw map; word.test.rs checks the shipped map against it. The map
/// is scaled by `MOON_ALBEDO` over this mean.
pub const MOON_MAP_MEAN: f32 = 0.2459;

/// The froxels along each side and along the depth of the air's volume for
/// each light (gpu/atmosphere.rs `froxel`). The volume is Hillaire 2020's
/// aerial perspective at 32³, the size Unreal's SkyAtmosphere uses, and it
/// is laid out about its light as the light's sky view is.
pub const AIR_FROXELS: u32 = 32;

/// The side of a square workgroup, with one thread a drawn pixel, a sample
/// or a tile of the light's box (sample_box.wgsl, light_slide.wgsl).
pub const SQUARE_SIDE: u32 = 8;
/// The threads of a workgroup over a row, one thread an entry
/// (light_mark_draws.wgsl, forget.wgsl).
pub const ROW_LANES: u32 = 64;

/// The words the sky views and the exposure measure into (gpu/atmosphere.rs
/// `plan`, atmos_skyview.wgsl `slopes`). Each light takes `LIGHT_SLOPES`
/// words, the sun's first, and the ground's two words follow at
/// `GROUND_SLOPES`.
pub const LIGHT_SLOPES: u32 = 5;
pub const GROUND_SLOPES: u32 = 2 * LIGHT_SLOPES;
pub const SLOPE_WORDS: u32 = GROUND_SLOPES + 2;

/// Half an 8-bit step of opacity. A cloud sheet whose opacity is below it
/// changes no pixel by a step, so it is not drawn.
pub const HALF_STEP: f32 = 0.5 / 255.0;

/// A layer number past the last layer. A face given it is drawn with its
/// swatch alone.
pub const NO_LAYER: u32 = 255;

/// A face's word holds the slots in the pack of its top, side and bottom
/// images, `SLOT_BITS` bits each, in the order `IMAGE_TOP`, `IMAGE_SIDE`,
/// `IMAGE_BOTTOM`. A slot is `NO_SLOT` until its image arrives. `NO_SLOT`
/// lies past any texture array WebGPU gives (maxTextureArrayLayers is 256).
pub const SLOT_BITS: u32 = 10;
pub const NO_SLOT: u32 = (1 << SLOT_BITS) - 1;
pub const IMAGE_TOP: u32 = 0;
pub const IMAGE_SIDE: u32 = 1;
pub const IMAGE_BOTTOM: u32 = 2;

#[cfg(test)]
#[path = "word.test.rs"]
mod test;
