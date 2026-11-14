// pxl-render/build/shared.rs

//! What the shaders share with the Rust, written from the Rust that defines it: the parts the
//! build writes, each its constants and then the small functions computed from pxl-engine,
//! pxl-spec, pxl-content and the crate. A value is written once. It goes into the part every
//! shader that reads it holds, and into constant, which every shader holds, where those
//! shaders hold no other part in common.

use crate::wgsl::Val::{self, *};
use crate::wgsl::{consts, predicate, table};
use crate::{atmosphere, color, debug_look, param, word};
use pxl_spec::asset::cube::Sentinel;
use word::screen;

/// Every part the build writes, by its name. A part with a file of its own under
/// src/frame/shader takes what is written here after that file.
pub fn parts() -> [(&'static str, String); 11] {
    [
        ("constant", constant()),
        ("atmos_medium", atmos_medium()),
        ("walk_word", walk_word()),
        ("world_word", world_word()),
        ("decor_word", decor_word()),
        ("ray", ray()),
        ("raster_box", raster_box()),
        ("light_word", light_word()),
        ("tile_word", tile_word()),
        ("cloud_hash", cloud_hash()),
        ("debug_look", debug_look()),
    ]
}

/// The values the host lays out and every shader may read (frame/shader/word.rs): each
/// uniform's size in vec4s and the sizes and kinds the host and the shaders share. Then the
/// values read by shaders that hold no other part in common, and every setting in
/// src/param.rs.
fn constant() -> String {
    use pxl_content::param::night;
    use pxl_engine::answer::side;
    use pxl_engine::{eye, face, light, param, WORKGROUP_CAP};
    use pxl_spec::share::EIGHTH_CNT;
    use pxl_spec::space::camera::grid;
    use pxl_spec::space::coord::q3::FRAC_W;
    use pxl_spec::space::coord::zoom::{HEIGHT_BITS, UNIT_ZOOM};
    use pxl_spec::space::layer::lattice::EFFECT_REACH;
    use word::*;
    let vec4s = |floats: usize| {
        assert!(floats.is_multiple_of(4), "a uniform is whole vec4s");
        U32(floats as u32 / 4)
    };
    let size = |(w, h): (u32, u32)| Vec2f([w as f32, h as f32]);
    let mut out = consts([
        ("DRAW_VEC4S", vec4s(DRAW_FLOATS)),
        // A Q3 step in units. It is a power of two, so scaling a record's position by it is
        // exact (frame.wgsl `from_eye`).
        ("Q3_STEP", F32(1.0 / (1u32 << FRAC_W) as f32)),
        ("FRAME_VEC4S", vec4s(FRAME_FLOATS)),
        ("SKY_VEC4S", vec4s(SKY_FLOATS)),
        ("ATMOS_VEC4S", vec4s(ATMOS_FLOATS)),
        ("CLOUDS_VEC4S", vec4s(CLOUDS_FLOATS)),
        ("STARS_VEC4S", vec4s(STARS_FLOATS)),
        ("AO_VEC4S", vec4s(AO_FLOATS)),
        ("MARKER_VEC4S", vec4s(MARKER_FLOATS)),
        ("STAR_REC_VEC4S", vec4s(STAR_REC_FLOATS)),
        (
            "LOCATION_ZONE_COLOR",
            Vec3f(color::linear_rgb(color::indicator::ACTION).map(|v| v as f32)),
        ),
        ("MARKER_INSTANCE_WORDS", U32(MARKER_INSTANCE_WORDS)),
        (
            "LOCATION_ZONE_INSTANCE_FLOATS",
            U32(LOCATION_ZONE_INSTANCE_FLOATS),
        ),
        ("AO_DOWNSCALE", U32(AO_DOWNSCALE)),
        ("AO_NOISE_SIDE", U32(AO_NOISE_SIDE)),
        ("DECOR_GROUP_CNT", U32(DECOR_GROUP_CNT as u32)),
        // The side of a square of the depth bound, and the squares on a side of a screen
        // tile; the bound's passes take a tile as one workgroup (screen_bound.wgsl,
        // screen_propagate.wgsl). Then the number of frames the jitter sequence takes to cover
        // a pixel, over which each still frame adds an even share (`screen::JITTER_CNT`,
        // screen_resolve.wgsl).
        ("BOUND_STEP", U32(BOUND_STEP)),
        ("BOUND_SQUARES", U32(BOUND_SQUARES)),
        ("TILE_JITTER_CNT", U32(screen::JITTER_CNT)),
        ("TRANSMITTANCE_SIZE", size(TRANSMITTANCE_SIZE)),
        ("MULTISCATTER_SIZE", size(MULTISCATTER_SIZE)),
        ("SKY_VIEW_SIZE", size(SKY_VIEW_SIZE)),
        ("IRRADIANCE_SIZE", size(IRRADIANCE_SIZE)),
        ("ATLAS_FILL", U32(ATLAS_FILL)),
        ("ATLAS_STROKE", U32(ATLAS_STROKE)),
        ("ATLAS_TRIANGLE", U32(ATLAS_TRIANGLE)),
        ("ATLAS_TEXT", U32(ATLAS_TEXT)),
        ("ATLAS_HALO", U32(ATLAS_HALO)),
        ("BOX_VERTEX_CNT", U32(BOX_VERTEX_CNT)),
        ("MOON_TEXELS", U32(MOON_TEXELS)),
        ("NO_LAYER", F32(NO_LAYER as f32)),
        ("HALF_STEP", F32(HALF_STEP)),
        ("MOON_ANGULAR_RADIUS", F32(MOON_ANGULAR_RADIUS)),
        ("MOON_ALBEDO", F32(MOON_ALBEDO)),
        ("MOON_MAP_MEAN", F32(MOON_MAP_MEAN)),
        ("SLOT_BITS", U32(SLOT_BITS)),
        ("NO_SLOT", U32(NO_SLOT)),
        ("IMAGE_TOP", U32(IMAGE_TOP)),
        ("IMAGE_SIDE", U32(IMAGE_SIDE)),
        ("IMAGE_BOTTOM", U32(IMAGE_BOTTOM)),
        ("EARTH_TILE_BITS", U32(EARTH_TILE_BITS)),
        ("WORKGROUP_CAP", U32(WORKGROUP_CAP)),
        ("AIR_FROXELS", U32(AIR_FROXELS)),
        ("SQUARE_SIDE", U32(SQUARE_SIDE)),
        ("ROW_LANE_CNT", U32(ROW_LANE_CNT)),
        ("CELL_SIDE", U32(CELL_SIDE)),
        ("LIGHT_SLOPES", U32(LIGHT_SLOPES)),
        ("GROUND_SLOPES", U32(GROUND_SLOPES)),
        ("SLOPE_WORDS", U32(SLOPE_WORDS)),
        ("CLOUD_TILE_UNITS", U32(CLOUD_TILE_UNITS)),
        ("CLOUD_TILE_TEXELS", U32(CLOUD_TILE_TEXELS)),
        // The bits of a Q3 Z, which spans the world's height about sea level.
        ("HEIGHT_BITS", U32(HEIGHT_BITS)),
        ("NIGHT_FALL", Vec2f(night::FALL)),
        // The camera grid's cell is the footprint a drawn cell covers per unit of distance,
        // seen square on. Every effect shares the one reach written beside it.
        ("GRID_CELL", F32(grid::CELL)),
        ("EFFECT_REACH", F32(EFFECT_REACH as f32)),
        // How far apart a receiver's taps lie in the shadow map. The map's read pad is sized
        // by it (pxl-engine param.rs `light::SPREAD`, light.rs `READ_PAD`).
        ("LIGHT_SPREAD", F32(param::light::SPREAD)),
        // The unit's zoom, and the bits of a Q3 step's fraction. A cell's place is taken from
        // the eye's position in Q3 steps, and a ray painted by its cell's zoom against the
        // unit's.
        ("UNIT_ZOOM", U32(UNIT_ZOOM)),
        ("FRAC_W", U32(FRAC_W)),
        // Where each of the eye's words lies (pxl-engine eye.rs). A walk binds the eye whole;
        // the screen pass binds it up to the jitter's two words, the last it reads.
        ("EYE_VEC4S", U32(eye::WORDS as u32 / 4)),
        ("EYE_READ_VEC4S", U32((eye::JITTER as u32 + 1) / 4 + 1)),
        ("EYE_ORIGIN", U32(eye::ORIGIN as u32)),
        ("EYE_STEPS", U32(eye::STEPS as u32)),
        ("EYE_FORWARD", U32(eye::FORWARD as u32)),
        ("EYE_FAR", U32(eye::FAR as u32)),
        ("EYE_RIGHT", U32(eye::RIGHT as u32)),
        ("EYE_UP", U32(eye::UP as u32)),
        ("EYE_TAN", U32(eye::TAN as u32)),
        ("EYE_RANGE", U32(eye::RANGE as u32)),
        ("EYE_CARVE", U32(eye::CARVE as u32)),
        ("EYE_HIDE", U32(eye::HIDE as u32)),
        ("EYE_LIGHT", U32(eye::LIGHT as u32)),
        ("EYE_JITTER", U32(eye::JITTER as u32)),
        ("EYE_GRID", U32(eye::GRID as u32)),
        // A light tile's side in texels (pxl-engine light.rs `TILE`).
        ("LIGHT_TILE", U32(light::TILE)),
        // A ray's faces word records what lies around the face it hit, one bit for each edge
        // and each corner, as pxl-engine face.rs lays it out.
        ("FACE_LINE_SHIFT", U32(face::LINE_SHIFT)),
        ("FACE_RISE_SHIFT", U32(face::RISE_SHIFT)),
        ("FACE_CORNER_SHIFT", U32(face::CORNER_SHIFT)),
    ]);
    // The side of the box drawn for a cell that fills only part of its cube, by the number of
    // eighths the cell keeps (pxl-engine answer.rs `side`). A fill of 0 eighths means all eight
    // are kept.
    out += &table(
        "fill_side",
        (1..EIGHTH_CNT).map(|k| (k, F32(side(k)))),
        F32(side(EIGHTH_CNT)),
    );
    out + &consts(params())
}

/// The air's numbers (look/atmosphere.rs) and the Earth's window's size, which no setting, time
/// or frame moves, for atmos_medium.wgsl, which every shader that reads the air holds. The air's
/// uniform holds what moves (frame/shader/word.rs `atmos`).
fn atmos_medium() -> String {
    use atmosphere::*;
    use pxl_spec::bake::tile::earth::{EARTH_DIST, EARTH_ZOOM};
    use pxl_spec::space::coord::world::around;
    use pxl_spec::space::layer::demand::GROUND_DEMAND_ACROSS;
    let f = |v: f64| F32(v as f32);
    let rgb = |v: [f64; 3]| Vec3f(v.map(|c| c as f32));
    consts([
        ("BOTTOM_RADIUS_KM", f(BOTTOM_RADIUS_KM)),
        ("TOP_RADIUS_KM", f(TOP_RADIUS_KM)),
        // How far a level ray from the ground runs to the top of the air: the leg of the right
        // triangle whose hypotenuse is the top's radius and whose other leg is the ground's
        // (atmos_medium.wgsl `transmittance_uv`).
        (
            "HORIZON_TO_TOP_KM",
            f(libm::sqrt(
                TOP_RADIUS_KM * TOP_RADIUS_KM - BOTTOM_RADIUS_KM * BOTTOM_RADIUS_KM,
            )),
        ),
        ("RAYLEIGH_SCATTERING", rgb(RAYLEIGH_SCATTERING)),
        ("RAYLEIGH_SCALE_HEIGHT_KM", f(RAYLEIGH_SCALE_HEIGHT_KM)),
        ("MIE_SCATTERING", f(MIE_SCATTERING)),
        ("MIE_EXTINCTION", f(MIE_EXTINCTION)),
        ("MIE_SCALE_HEIGHT_KM", f(MIE_SCALE_HEIGHT_KM)),
        ("MIE_G", f(MIE_G)),
        ("OZONE_ABSORPTION", rgb(OZONE_ABSORPTION)),
        ("OZONE_CENTER_KM", f(OZONE_CENTER_KM)),
        ("OZONE_HALF_WIDTH_KM", f(OZONE_HALF_WIDTH_KM)),
        ("GROUND_ALBEDO", f(GROUND_ALBEDO)),
        ("SUN_ANGULAR_RADIUS", f(SUN_ANGULAR_RADIUS)),
        ("EXPOSURE", f(EXPOSURE)),
        ("NIGHT_STOPS", f(NIGHT_STOPS)),
        ("SHOWN_FALL", f(SHOWN_FALL)),
        // The night sky's light, which lights the ground and is never drawn itself, and a lit
        // city's radiance where the Earth's picture shows city lights at full strength.
        ("NIGHT_SKY", rgb(NIGHT_COLOR.map(|c| c * NIGHT_SKY))),
        ("CITY_LIGHT", rgb(CITY_COLOR.map(|c| c * CITY_LIGHT))),
        // How far from the eye the Earth's tiles are drawn before the global picture takes
        // over, in units (pxl-spec bake/tile/earth.rs), and the window's tiles across and the
        // tiles around the world at the Earth's zoom.
        ("EARTH_DIST", f(EARTH_DIST)),
        ("EARTH_ACROSS", I32(GROUND_DEMAND_ACROSS as i32)),
        ("EARTH_AROUND", I32(around(EARTH_ZOOM) as i32)),
    ])
}

/// Every setting in src/param.rs, named by its path under `param`: `LIMB` in `sky::sun`
/// becomes `SKY_SUN_LIMB`. An `F64` setting is the host's alone and is not written.
fn params() -> Vec<(String, Val)> {
    use param::Value;
    let mut out = Vec::new();
    for &(path, name, val) in param::ALL.iter().flat_map(|m| m.iter()) {
        let topic = path.rsplit_once("param::").map_or("", |(_, t)| t);
        let named = format!("{}_{name}", topic.replace("::", "_").to_uppercase());
        out.push((
            named,
            match val {
                Value::F32(v) => F32(v),
                Value::I32(v) => I32(v),
                Value::U32(v) => U32(v),
                Value::Vec2(v) => Vec2f(v),
                Value::Vec3(v) => Vec3f(v),
                Value::F64(_) => continue,
            },
        ));
    }
    out
}

/// The constants the walks read (walk_word.wgsl), each written from the pxl-engine layout or
/// rule that defines it, and pxl-spec `cube::is_matter`.
fn walk_word() -> String {
    use pxl_engine::beam::SAMPLE_SIDE;
    use pxl_engine::clipmap::{self, Lookup};
    use pxl_engine::object::{NodeSlot, DAMAGE_CAP};
    use pxl_engine::{
        answer, beam, cast, count, face, light, node_cache, param, raster, ray, record,
    };
    use pxl_spec::asset::cube::is_matter;
    use pxl_spec::share::EIGHTH_CNT;
    use pxl_spec::space::camera::grid;
    use pxl_spec::space::coord::zoom::{FINEST_ZOOM, SHARD_ZOOM};
    use pxl_spec::space::layer::area::Shard;
    const {
        // `light_item` (light_list.wgsl) reads a workgroup's lanes as a `SAMPLE_SIDE` by
        // `SAMPLE_SIDE` square over one entry of the list. Each pass that calls it declares its
        // workgroup with a side of its own: light_cell.wgsl with `SAMPLE_SIDE`, the light's
        // sample box (sample.light.wgsl `sample_item`) with `SQUARE_SIDE`, and light.wgsl with
        // its tile. The cast sizes each pass's dispatch by that pass's side (ray/cast.rs
        // `light_args`), so each side must be SAMPLE_SIDE.
        assert!(
            light::TILE == SAMPLE_SIDE && word::SQUARE_SIDE == SAMPLE_SIDE,
            "light_item reads a workgroup as SAMPLE_SIDE x SAMPLE_SIDE lanes: a light tile and \
             SQUARE_SIDE are each that side"
        );
        // A queue's number is its run's tag, but for the light's holes (ask_run.wgsl
        // `walk_pass`).
        assert!(
            ray::QUEUE_EYE_CORNERS == node_cache::BEAM_PASS
                && ray::QUEUE_EYE_SAMPLES == node_cache::SAMPLES_PASS
                && ray::QUEUE_EYE_HOLES == node_cache::MISSED_PASS
                && ray::QUEUE_LIGHT_CORNERS == node_cache::LIGHT_BEAM_PASS
                && ray::QUEUE_LIGHT_SAMPLES == node_cache::LIGHT_SAMPLES_PASS
        );
    }
    consts([
        ("INV_SPLIT2", F32(grid::INV_SPLIT2)),
        ("NARROW", F32(cast::NARROW)),
        ("NARROW_SQUARED", F32(cast::NARROW_SQUARED)),
        // The light's cell rays start a margin short of where its beam ray stopped: a share of
        // the beam's spacing plus a number of Q3 steps (`light::margin`). A receiver reads
        // slots within a disk grown by its taps' reach. A tile is taken to hold such a slot
        // when the tile's middle lies within that disk grown by half the tile's diagonal
        // (`light::TILE_READ_PAD`, light_propagate.wgsl).
        ("LIGHT_MARGIN_PER_WIDTH", F32(2.0 * cast::SQRT_6)),
        ("LIGHT_MARGIN_STEPS", F32(light::margin(0.0))),
        ("LIGHT_TILE_READ_PAD", F32(light::TILE_READ_PAD)),
        ("LIGHT_FOOTPRINT_TILES", I32(light::FOOTPRINT_TILES)),
        ("LIGHT_READ_FIRST", I32(light::READ_TILES[0])),
        ("LIGHT_READ_LAST", I32(light::READ_TILES[1])),
        ("FRAC_1_SQRT_2", F32(core::f32::consts::FRAC_1_SQRT_2)),
        ("SAMPLE_SPACING", F32(beam::SAMPLE_SPACING)),
        ("SAMPLE_SPACING_INVERSE", F32(beam::SAMPLE_SPACING_INVERSE)),
        ("GRID_CELL_INVERSE", F32(grid::CELL_INVERSE)),
        ("REC_VIEW_GROWTH", F32(record::VIEW_GROWTH)),
        ("FINEST_ZOOM", U32(FINEST_ZOOM)),
        ("ZOOM_BITS", U32(pxl_engine::ZOOM_BITS)),
        ("SAMPLE", U32(param::beam::SAMPLE)),
        ("NEAR_SAMPLES", U32(param::beam::NEAR)),
        ("CELL_T", U32(beam::CELL_T)),
        ("CELL_CORNER", U32(beam::CELL_CORNER)),
        ("CELL_RESUMES", U32(beam::CELL_RESUMES)),
        ("CELL_WORDS", U32(beam::CELL_WORDS)),
        ("LINE_NODE", U32(beam::LINE_NODE)),
        ("LINE_STEPS", U32(beam::LINE_STEPS)),
        ("LINE_WORDS", U32(beam::LINE_WORDS)),
        ("NODE_ZOOM", U32(beam::NODE_ZOOM)),
        ("NODE_CORNER", U32(beam::NODE_CORNER)),
        ("NODE_ROOTS", U32(beam::NODE_ROOTS)),
        ("SAMPLE_NODE", U32(beam::SAMPLE_NODE)),
        ("SAMPLE_ASSET", U32(beam::SAMPLE_ASSET)),
        ("SAMPLE_PARKED", U32(beam::SAMPLE_PARKED)),
        ("SAMPLE_PARKED_T", U32(beam::SAMPLE_PARKED_T)),
        ("SAMPLE_PARKED_STATE", U32(beam::SAMPLE_PARKED_STATE)),
        ("SAMPLE_LACK_WORD", U32(beam::SAMPLE_LACK_WORD)),
        ("SAMPLE_AXIS", U32(beam::SAMPLE_AXIS)),
        ("SAMPLE_LACK", U32(beam::SAMPLE_LACK)),
        ("SAMPLE_STEPS_SHIFT", U32(beam::SAMPLE_STEPS_SHIFT)),
        ("NODE_WORDS", U32(beam::NODE_WORDS)),
        ("SAMPLE_WORDS", U32(beam::SAMPLE_WORDS)),
        ("SAMPLE_NONE", U32(beam::SAMPLE_NONE)),
        ("SAMPLE_SIDE", U32(SAMPLE_SIDE)),
        ("ENTRY_CNT", U32(node_cache::ENTRY_CNT)),
        ("ENTRIES_AT", U32(node_cache::ENTRIES_AT)),
        ("ENTRY_WORDS", U32(node_cache::ENTRY_WORDS)),
        ("BUCKET", U32(node_cache::BUCKET)),
        ("W_KIND", U32(node_cache::W_KIND)),
        ("KIND_MASK", U32(node_cache::KIND_MASK)),
        ("W_ASSET", U32(node_cache::W_ASSET)),
        ("W_NODE", U32(node_cache::W_NODE)),
        ("NONE", U32(node_cache::NONE)),
        ("CLAIMED", U32(node_cache::CLAIMED)),
        ("CLAIM_MASK", U32(node_cache::CLAIM_MASK)),
        ("CLAIM_FAR", U32(node_cache::CLAIM_FAR)),
        ("CELL_FAR", U32(node_cache::CELL_FAR)),
        ("TAG_BITS", U32(node_cache::TAG_BITS)),
        ("KEY_ZOOMS", U32(!((1u32 << node_cache::TAG_BITS) - 1))),
        ("KIND_SPLIT", U32(answer::Kind::Split as u32)),
        ("KIND_CELL", U32(answer::Kind::Cell as u32)),
        ("KIND_LACK", U32(answer::Kind::Lack as u32)),
        ("VAL_MASK", U32(answer::PLACED_ASSET - 1)),
        ("VAL_VOID", U32(Sentinel::Void as u32)),
        ("VAL_AIR", U32(Sentinel::Air as u32)),
        ("FILL_SHIFT", U32(answer::FILL_SHIFT)),
        ("EIGHTH_CNT", U32(EIGHTH_CNT)),
        ("BOX_BITS", U32(answer::BOX_BITS)),
        ("BOX_STEPS", U32(answer::BOX_STEPS)),
        ("STEPS_CAP", U32(cast::STEPS_CAP)),
        ("LIGHT_HDR_VEC4S", U32(light::HDR_VEC4S as u32)),
        ("LIGHT_HDR_HOLES", U32(light::HDR_HOLES as u32)),
        ("LIGHT_STARTS_AT", U32(light::STARTS_AT as u32)),
        ("LIGHT_ANCHOR", U32(light::ANCHOR as u32)),
        ("LIGHT_STEP_BITS", U32(light::STEP_BITS)),
        ("LIGHT_DEPTH_ONE", U32(light::DEPTH_ONE)),
        ("LIGHT_HDR_SIZE", U32(light::HDR_SIZE as u32)),
        ("MISSED_PASS", U32(node_cache::MISSED_PASS)),
        (
            "EYE_MISS_CNT",
            U32(node_cache::lane_at(count::EYE, count::HOLE_WALK)),
        ),
        ("HITS_ARGS", U32(node_cache::HITS_ARGS)),
        ("HITS_CNT", U32(node_cache::HITS_CNT)),
        (
            "LIGHT_ASKED_CNT",
            U32(node_cache::lane_at(count::LIGHT, count::ASK)),
        ),
        ("LIGHT_T_MAX", U32(light::W_T_MAX as u32)),
        ("LIGHT_ACROSS", U32(light::W_ACROSS as u32)),
        ("LIGHT_DOWN", U32(light::W_DOWN as u32)),
        ("LIGHT_DIR", U32(light::W_DIR as u32)),
        ("LIGHT_BIAS", U32(light::W_BIAS as u32)),
        ("LIGHT_WIDTH", U32(light::W_WIDTH as u32)),
        ("LIGHT_INVERSE", U32(light::W_INVERSE as u32)),
        ("LIGHT_T_MAX_INVERSE", U32(light::W_T_MAX_INVERSE as u32)),
        (
            "LIGHT_ACROSS2_INVERSE",
            U32(light::W_ACROSS2_INVERSE as u32),
        ),
        ("LIGHT_DOWN2_INVERSE", U32(light::W_DOWN2_INVERSE as u32)),
        ("LIGHT_BAND_SCALE", U32(light::W_BAND_SCALE as u32)),
        ("LIGHT_CORNER_X", U32(light::CORNER[0] as u32)),
        ("LIGHT_CORNER_Y", U32(light::CORNER[1] as u32)),
        ("LIGHT_HDR_CAST", U32(light::HDR_CAST as u32)),
        ("LIGHT_HDR_EPOCH", U32(light::HDR_EPOCH as u32)),
        ("LIGHT_HDR_CELLS", U32(light::HDR_CELLS as u32)),
        (
            "LIGHT_HDR_CELLS_ACROSS",
            U32(light::HDR_CELLS_ACROSS as u32),
        ),
        ("LIGHT_HDR_SAMPLES", U32(light::HDR_SAMPLES as u32)),
        (
            "LIGHT_HDR_SAMPLES_ACROSS",
            U32(light::HDR_SAMPLES_ACROSS as u32),
        ),
        ("LIGHT_HDR_READS", U32(light::HDR_READS as u32)),
        ("LIGHT_HDR_TILES", U32(light::HDR_TILES as u32)),
        ("LIGHT_HDR_HANDS", U32(light::HDR_HANDS as u32)),
        ("LIGHT_HDR_CARRY", U32(light::HDR_CARRY as u32)),
        ("LIGHT_HDR_SLIDE", U32(light::HDR_SLIDE as u32)),
        ("LIGHT_HDR_WORLD", U32(light::HDR_WORLD as u32)),
        ("LIGHT_HDR_SLIDE_CAST", U32(light::HDR_SLIDE_CAST as u32)),
        (
            "LIGHT_HDR_WORLD_RECEIVERS",
            U32(light::HDR_WORLD_RECEIVERS as u32),
        ),
        ("LIGHT_HDR_POOL", U32(light::HDR_POOL as u32)),
        ("LIGHT_HDR_HITS", U32(light::HDR_HITS as u32)),
        ("LIGHT_SLIDE_RAN", U32(light::SLIDE_RAN)),
        ("LIGHT_READS_FROM", U32(light::READS_FROM)),
        ("LIGHT_HDR_BEAM_ACROSS", U32(light::HDR_BEAM_ACROSS as u32)),
        (
            "LIGHT_HDR_CORNER_STATES",
            U32(light::HDR_CORNER_STATES as u32),
        ),
        (
            "LIGHT_HDR_PARKED_ITEMS",
            U32(light::HDR_PARKED_ITEMS as u32),
        ),
        ("LIGHT_HDR_LAST_RANGE", U32(light::HDR_LAST_RANGE as u32)),
        ("LIGHT_HDR_MARKS", U32(light::HDR_MARKS as u32)),
        ("LIGHT_HDR_LISTS", U32(light::HDR_LISTS as u32)),
        ("LIGHT_LIST_ACROSS", U32(light::LIST_ACROSS)),
        ("LIGHT_LIST_LANE_CNT", U32(light::LIST_LANE_CNT)),
        ("LIGHT_BEAM", U32(light::BEAM)),
        ("LIGHT_SAMPLE", U32(param::light::SAMPLE)),
        ("LIGHT_LINE_WORDS", U32(light::LINE_WORDS)),
        ("LIGHT_LINE_ORIGIN", U32(light::LINE_ORIGIN)),
        ("LIGHT_BEAM_WINDOW", U32(light::BEAM_WINDOW)),
        ("LIGHT_LIST_CORNERS", U32(light::LIST_CORNERS)),
        ("LIGHT_LIST_SOURCES", U32(light::LIST_SOURCES)),
        ("LIGHT_LIST_DAMAGE", U32(light::LIST_DAMAGE)),
        ("LIGHT_LIST_RECEIVERS", U32(light::LIST_RECEIVERS)),
        ("LIGHT_LIST_READS", U32(light::LIST_READS)),
        ("LIGHT_HDR_COUNTED", U32(light::HDR_COUNTED as u32)),
        ("LIGHT_BEAM_PASS", U32(node_cache::LIGHT_BEAM_PASS)),
        (
            "LIGHT_BEAM_CNT",
            U32(node_cache::lane_at(count::LIGHT, count::BEAM)),
        ),
        ("CNT_RAY_CNT", U32(count::RAY_CNT)),
        ("CNT_VOXEL", U32(count::VOXEL)),
        ("CNT_SKY", U32(count::SKY)),
        ("CNT_HOLE", U32(count::HOLE)),
        ("SAMPLES_PASS", U32(node_cache::SAMPLES_PASS)),
        ("LIGHT_SAMPLES_PASS", U32(node_cache::LIGHT_SAMPLES_PASS)),
        (
            "EYE_SAMPLE_CNT",
            U32(node_cache::lane_at(count::EYE, count::SAMPLE)),
        ),
        (
            "EYE_ASKED_CNT",
            U32(node_cache::lane_at(count::EYE, count::ASK)),
        ),
        (
            "LIGHT_SAMPLE_CNT",
            U32(node_cache::lane_at(count::LIGHT, count::SAMPLE)),
        ),
        ("GRID_KEEP", U32(beam::GRID_KEEP as u32)),
        ("GRID_RADIUS", U32(beam::GRID_RADIUS as u32)),
        ("GRID_RADIUS_TAN", U32(beam::GRID_RADIUS_TAN as u32)),
        ("GRID_PAST", U32(beam::GRID_PAST as u32)),
        ("GRID_SPAN", U32(beam::GRID_SPAN as u32)),
        ("GRID_CORNER_LATTICE", U32(beam::GRID_CORNER_LATTICE as u32)),
        ("GRID_SAMPLE_LATTICE", U32(beam::GRID_SAMPLE_LATTICE as u32)),
        ("GRID_CELLS_AT", U32(beam::GRID_CELLS_AT as u32)),
        ("GRID_SAMPLES_AT", U32(beam::GRID_SAMPLES_AT as u32)),
        ("GRID_HITS_AT", U32(beam::GRID_HITS_AT as u32)),
        ("GRID_ASKS_AT", U32(beam::GRID_ASKS_AT as u32)),
        ("GRID_HIT_ASKS_AT", U32(beam::GRID_HIT_ASKS_AT as u32)),
        ("GRID_WALKED_AT", U32(beam::GRID_WALKED_AT as u32)),
        (
            "GRID_CORNER_STATES_AT",
            U32(beam::GRID_CORNER_STATES_AT as u32),
        ),
        ("GRID_HANDS_AT", U32(beam::GRID_HANDS_AT as u32)),
        ("GRID_SLOPE", U32(beam::GRID_SLOPE as u32)),
        ("STARTS_ACROSS", U32(light::STARTS_ACROSS as u32)),
        ("STARTS_DOWN", U32(light::STARTS_DOWN as u32)),
        ("STARTS_ANCHORS", U32(light::STARTS_ANCHORS as u32)),
        ("STARTS_FRACS", U32(light::STARTS_FRACS as u32)),
        (
            "LIGHT_MISS_CNT",
            U32(node_cache::lane_at(count::LIGHT, count::HOLE_WALK)),
        ),
        ("FRAME", U32(node_cache::FRAME)),
        ("PASS_CNT", U32(node_cache::PASS_CNT)),
        ("CNT_ON", U32(node_cache::CNT_ON)),
        ("CNT_EYE", U32(count::EYE)),
        ("CNT_WALK_CNT", U32(count::WALK_CNT)),
        ("Y_START", U32(cast::Y_START)),
        ("Z_START", U32(cast::Z_START)),
        ("SHARD_ZOOM", U32(SHARD_ZOOM)),
        ("SHARD_UNKNOWN", U32(Shard::UNKNOWN.word())),
        ("SHARD_INSIDE", U32(Shard::INSIDE.word())),
        ("LOOKUP_NONE", U32(Lookup::NONE.word())),
        ("SLOT_NOTHING", U32(NodeSlot::NOTHING.word())),
        ("LINE_INHERITANCE", U32(beam::LINE_INHERITANCE)),
        ("LINE_STEP", U32(beam::LINE_STEP)),
        ("W_INHERITANCE", U32(node_cache::W_INHERITANCE)),
        ("MISS_ACROSS", U32(ray::MISS_ACROSS)),
        ("MISS_LANE_CNT", U32(ray::MISS_LANE_CNT)),
        ("SQUARE_WORDS", U32(ray::SQUARE_WORDS)),
        ("POOL_CAP", U32(ray::POOL_CAP)),
        ("PARK_HOLE", U32(ray::PARK_HOLE)),
        ("PARK_WORDS", U32(ray::PARK_WORDS)),
        ("HITS_A_FRAME", U32(ray::HITS_A_FRAME)),
        ("HIT_LANE_CNT", U32(beam::HIT_LANE_CNT)),
        ("HIT_WORDS", U32(beam::HIT_WORDS)),
        ("HIT_NODE", U32(beam::HIT_NODE)),
        ("HIT_FACE", U32(beam::HIT_FACE)),
        ("HIT_PX", U32(beam::HIT_PX)),
        ("HIT_WAIT", U32(beam::HIT_WAIT)),
        ("HIT_LACK", U32(beam::HIT_LACK)),
        ("QUEUE_LANE_CNT", U32(ray::QUEUE_LANE_CNT)),
        ("QUEUE_GROUP_CNT", U32(ray::QUEUE_GROUP_CNT)),
        ("QUEUE_STEP_CNT", U32(ray::QUEUE_STEP_CNT)),
        ("QUEUE_NEXT", U32(ray::QUEUE_NEXT)),
        ("QUEUE_NEXT_PARKED", U32(ray::QUEUE_NEXT_PARKED)),
        ("QUEUE_ASKS", U32(ray::QUEUE_ASKS)),
        ("QUEUE_ASKS_PARKED", U32(ray::QUEUE_ASKS_PARKED)),
        ("QUEUE_HITS", U32(ray::QUEUE_HITS)),
        ("QUEUE_PARKED", U32(ray::QUEUE_PARKED)),
        (
            "EYE_CORNERS_RECS_AT",
            U32(ray::queue_recs_at(ray::QUEUE_EYE_CORNERS)),
        ),
        (
            "EYE_SAMPLES_RECS_AT",
            U32(ray::queue_recs_at(ray::QUEUE_EYE_SAMPLES)),
        ),
        (
            "EYE_HOLES_RECS_AT",
            U32(ray::queue_recs_at(ray::QUEUE_EYE_HOLES)),
        ),
        (
            "LIGHT_CORNERS_RECS_AT",
            U32(ray::queue_recs_at(ray::QUEUE_LIGHT_CORNERS)),
        ),
        (
            "LIGHT_SAMPLES_RECS_AT",
            U32(ray::queue_recs_at(ray::QUEUE_LIGHT_SAMPLES)),
        ),
        (
            "LIGHT_HOLES_RECS_AT",
            U32(ray::queue_recs_at(ray::QUEUE_LIGHT_HOLES)),
        ),
        ("QUEUE_REC_WORDS", U32(ray::QUEUE_REC_WORDS)),
        ("QUEUE_WORDS", U32(ray::QUEUE_WORDS)),
        ("QUEUE_HDR_STRIDE", U32(ray::QUEUE_HDR_STRIDE)),
        ("QUEUE_LIGHT_CORNERS", U32(ray::QUEUE_LIGHT_CORNERS)),
        ("QUEUE_LIGHT_HOLES", U32(ray::QUEUE_LIGHT_HOLES)),
        ("QUEUE_RECS_CAP", U32(ray::QUEUE_RECS_CAP)),
        ("STATE_WORDS", U32(ray::STATE_WORDS)),
        ("PARKED", U32(ray::PARKED)),
        ("LAST_NEXT", U32(node_cache::LAST_NEXT)),
        ("LAST_CNT", U32(node_cache::LAST_CNT)),
        ("ASKED", U32(node_cache::ASKED)),
        ("ASKS_A_FRAME", U32(ray::ASKS_A_FRAME)),
        ("HIT_ASSET", U32(beam::HIT_ASSET)),
        ("HIT_LINE", U32(beam::HIT_LINE)),
        ("FACE_BOX", U32(face::BOX)),
        ("RAY_FACE_SHIFT", U32(ray::FACE_SHIFT)),
        ("RAY_ZOOM_SHIFT", U32(ray::ZOOM_SHIFT)),
        ("RAY_FILL_SHIFT", U32(ray::FILL_SHIFT)),
        ("CAST_SIDE", U32(ray::CAST_SIDE)),
        ("SCREEN_TILE", U32(screen::TILE)),
        ("RASTER_ASKED_AT", U32(raster::ASKED_AT)),
        ("RASTER_HIT_ASKED_AT", U32(raster::HIT_ASKED_AT)),
        ("BEAM_PASS", U32(node_cache::BEAM_PASS)),
        (
            "EYE_BEAM_CNT",
            U32(node_cache::lane_at(count::EYE, count::BEAM)),
        ),
        ("RASTER_ARGS_INSTANCE_CNT", U32(raster::ARGS_INSTANCE_CNT)),
        ("RASTER_ARGS_ASKS", U32(raster::ARGS_ASKS)),
        ("RASTER_ARGS_CNT", U32(raster::ARGS_CNT)),
        ("RASTER_FACES_LANE_CNT", U32(raster::FACES_LANE_CNT)),
        ("LIGHT_HDR_LAST_CORNER", U32(light::HDR_LAST_CORNER as u32)),
        ("LIGHT_HDR_FOOTPRINT", U32(light::HDR_FOOTPRINT as u32)),
        (
            "LIGHT_RECEIVER_BOXES_AT",
            U32(light::RECEIVER_BOXES_AT as u32),
        ),
        (
            "LIGHT_RECEIVER_BOX_CAP",
            U32(light::RECEIVER_BOX_CAP as u32),
        ),
        ("LIGHT_COLUMNS_AT", U32(light::COLUMNS_AT as u32)),
        ("LIGHT_COLUMN_CAP", U32(light::COLUMN_CAP as u32)),
        ("LIGHT_REACH_NDC", U32(light::REACH_NDC as u32)),
        ("LIGHT_ARGS_CELLS", U32(light::ARGS_CELLS)),
        ("LIGHT_ARGS_SAMPLE_HIT", U32(light::ARGS_SAMPLE_HIT)),
        ("LIGHT_ARGS_SLOT", U32(light::ARGS_SLOT)),
        ("LIGHT_RECEIVER_TILES_CNT", U32(light::RECEIVER_TILES_CNT)),
        ("LIGHT_ARGS_LAY", U32(light::ARGS_LAY)),
        ("LIGHT_ARGS_PASSES", U32(light::ARGS_PASSES)),
        ("LIGHT_LIST_CNT", U32(light::LIST_CNT)),
        ("ASK_NODE", U32(beam::ASK_NODE)),
        ("ASK_FACE", U32(beam::ASK_FACE)),
        ("ASK_LINE", U32(beam::ASK_LINE)),
        ("ASK_TO", U32(beam::ASK_TO)),
        ("ASK_WORDS", U32(beam::ASK_WORDS)),
        ("ASK_NONE", U32(beam::ASK_NONE)),
        ("FACE_WORD_BITS", U32(face::BITS)),
        ("FACE_EDGE_CNT", U32(face::EDGE_CNT)),
        ("RASTER_ARGS_FACES", U32(raster::ARGS_FACES)),
        ("NODE_CACHE_FORGETS", U32(node_cache::FORGETS)),
        ("FORGETS_ALL", U32(node_cache::FORGETS_ALL)),
        ("NODE_CACHE_CHANGED_AT", U32(node_cache::CHANGED_AT)),
        (
            "NODE_CACHE_CHANGED_BRICKS_AT",
            U32(node_cache::CHANGED_BRICKS_AT),
        ),
        ("NODE_CACHE_RECTS_AT", U32(node_cache::RECTS_AT)),
        ("NODE_CACHE_WRITTEN_AT", U32(node_cache::WRITTEN_AT)),
        ("NODE_CACHE_WRITTEN_CAP", U32(node_cache::WRITTEN_CAP)),
        ("NODE_CACHE_RECTS", U32(DAMAGE_CAP)),
        ("KEYS_AT", U32(node_cache::KEYS_AT)),
        ("STATES_AT", U32(node_cache::STATES_AT)),
        ("RECS_AT", U32(node_cache::RECS_AT)),
        ("R_WORDS", U32(node_cache::R_WORDS)),
        ("R_DRAWN", U32(node_cache::R_DRAWN)),
        ("R_FACES", U32(node_cache::R_FACES)),
        ("W_PLACED", U32(node_cache::W_PLACED)),
        ("EYE_CLIP_LEVEL_CNT", U32(clipmap::EYE_CLIP_LEVEL_CNT)),
        ("NODE_CACHE_DRAWN", U32(node_cache::DRAWN)),
        ("REC_EYE_DRAW_CAP", U32(record::EYE_DRAW_CAP)),
        ("REC_LIGHT_DRAW_CAP", U32(record::LIGHT_DRAW_CAP)),
        ("REC_DRAW_LANE_CNT", U32(record::DRAW_LANE_CNT)),
        ("REC_FACES", U32(record::FACES)),
        ("REC_ASKED", U32(record::ASKED)),
        ("REC_ASKED_SHIFT", U32(record::ASKED_SHIFT)),
        ("REC_WHOLE", U32(record::WHOLE)),
        ("REC_TO", U32(record::TO_REC)),
    ]) + &predicate("is_matter", answer::PLACED_ASSET, is_matter)
}

/// The constants and tables the world's query reads (world.wgsl), each written from the
/// pxl-engine or pxl-content layout or rule that defines it, and a value's kind, pxl-spec
/// `value::is_ground` and pxl-engine `object::voxel`.
fn world_word() -> String {
    use pxl_content::cave::{
        BLOCK, BLOCKS_TAG, CELL_BLOCK_CNT, CROSS_TAG, NO_SIDE, PIPE, ROOT, ROOT_BLOCKS, SPAWN_TAG,
        SPLIT_TAG, WALK_TAG,
    };
    use pxl_content::param::{cave, strata};
    use pxl_engine::clipmap as clip;
    use pxl_engine::object::{self, Cut};
    use pxl_engine::{answer, ground, seed};
    use pxl_spec::asset::cube::Ground;
    use pxl_spec::brick;
    use pxl_spec::ground::GroundId;
    use pxl_spec::share::{FILL_BITS, FULL};
    use pxl_spec::space::coord::world::SEA_LEVEL;
    use pxl_spec::space::coord::zoom::{LEAF_CELL_ZOOM, MIN_ZOOM};
    use pxl_spec::space::layer::area;
    use pxl_spec::value;
    use pxl_spec::way::{AIR_OF, NOTHING, SPAN_CAP};
    use xpute_kit::math::rng;
    let [a, b, c, d, _] = cave::ROOT_TUNNELS;
    let none = Cut::NONE;
    let mut out = consts([
        ("LEAF_CELL_ZOOM", U32(LEAF_CELL_ZOOM)),
        ("GROUND_ID_BITS", U32(GroundId::BITS)),
        ("GROUND_ID_MAX", U32(u32::from(GroundId::MAX))),
        ("MIN_ZOOM", U32(MIN_ZOOM)),
        ("VAL_UNKNOWN", U32(Sentinel::Unknown as u32)),
        ("PLACED_ASSET", U32(answer::PLACED_ASSET)),
        ("FULL", U32(FULL)),
        ("FULL_BITS", U32(FULL.trailing_zeros())),
        ("FILL_BITS", U32(FILL_BITS)),
        ("BRANCH", U32(object::BRANCH)),
        ("OBJECT_ROOT", U32(object::ROOT)),
        ("CHILDREN", U32(object::CHILDREN)),
        ("SLOT_FOLDED", U32(object::FOLDED)),
        ("SLOT_ZOOM_MASK", U32(object::ZOOM_MASK)),
        ("SLOT_SHIFT", U32(object::SLOT_SHIFT)),
        ("SHARD_KNOWN", U32(area::SHARD_KNOWN)),
        ("SHARD_SERVED_SHIFT", U32(area::SHARD_SERVED_SHIFT)),
        ("SHARD_OWN", U32(area::OWN)),
        ("BRICK", U32(brick::BRICK)),
        ("BRICK_SHIFT", U32(brick::BRICK_SHIFT)),
        ("BRICK_CELL_CNT", U32(brick::BRICK_CELL_CNT)),
        ("BRICK_WORDS", U32(clip::BRICK_WORDS)),
        ("BRICK_MIP_LAST", U32(clip::BRICK_MIP_LAST)),
        ("ACROSS", U32(clip::ACROSS)),
        ("CLIP_ENTRY_CNT", U32(clip::ENTRY_CNT)),
        ("FIRST_CELL_ZOOM", U32(clip::FIRST_CELL_ZOOM)),
        ("ORIGIN_AT", U32(clip::ORIGIN_AT)),
        ("JOINED_AT", U32(clip::JOINED_AT)),
        ("JOINED_ENTRY_CNT", U32(clip::JOINED_ENTRY_CNT)),
        ("JOINED_MIP_CNT", U32(clip::JOINED_MIP_CNT)),
        ("COLUMNS_AT", U32(clip::COLUMNS_AT)),
        ("BOUNDS_AT", U32(clip::BOUNDS_AT)),
        ("BOUND_ENTRY_WORDS", U32(clip::BOUND_ENTRY_WORDS)),
        ("OWN_AT", U32(clip::OWN_AT)),
        ("OWN_CELL_CNT", U32(clip::OWN_CELL_CNT)),
        ("ROOT_AT", U32(clip::ROOT_AT)),
        ("RUN_BITS_AT", U32(clip::RUN_BITS_AT)),
        ("RUN_RANK_AT", U32(clip::RUN_RANK_AT)),
        ("RUN_OFFS_AT", U32(clip::RUN_OFFS_AT)),
        ("SPANS_ACROSS", U32(clip::SPANS_ACROSS)),
        ("SPANS_CNT", U32(clip::SPANS_CNT)),
        ("WAY_NOTHING", U32(NOTHING)),
        ("WAY_AIR_OF", U32(AIR_OF)),
        ("WAY_SPAN_CAP", U32(SPAN_CAP as u32)),
        ("LOOKUP_SLOT_BITS", U32(clip::LOOKUP_SLOT_BITS)),
        ("LOOKUP_FROM_SHIFT", U32(clip::LOOKUP_FROM_SHIFT)),
        ("LOOKUP_CELL_ZOOM_SHIFT", U32(clip::LOOKUP_CELL_ZOOM_SHIFT)),
        ("LOOKUP_AT", U32((1 << clip::LOOKUP_CELL_ZOOM_SHIFT) - 1)),
        ("STRATA_SAND", U32(Ground::Sand as u32)),
        ("STRATA_BEACH", U32(Ground::Beach as u32)),
        ("STRATA_WATER", U32(Ground::Water as u32)),
        ("STRATA_SOIL", U32(Ground::Soil as u32)),
        ("STRATA_SANDSTONE", U32(Ground::Sandstone as u32)),
        ("STRATA_STONE", U32(Ground::Stone as u32)),
        ("CAVE_ROOT", U32(ROOT)),
        ("CAVE_PIPE", U32(PIPE)),
        ("CAVE_BLOCK", U32(BLOCK)),
        ("CELL_BLOCK_CNT", U32(CELL_BLOCK_CNT)),
        ("ROOT_BLOCKS", U32(ROOT_BLOCKS)),
        ("CAVE_NUDGE", U32(cave::NUDGE)),
        ("CAVE_NO_SIDE", U32(NO_SIDE)),
        ("CAVE_SPAWN_TAG", U32(SPAWN_TAG)),
        ("CAVE_WALK_TAG", U32(WALK_TAG)),
        ("CAVE_CROSS_TAG", U32(CROSS_TAG)),
        ("CAVE_SPLIT_TAG", U32(SPLIT_TAG)),
        ("CAVE_BLOCKS_TAG", U32(BLOCKS_TAG)),
        ("MASTER_SEED", U32(seed::MASTER_SEED)),
        ("SEED_SALT_32", U32(rng::SEED_SALT_32)),
        ("MIX32_FIRST", U32(rng::MIX32_FIRST)),
        ("MIX32_LAST", U32(rng::MIX32_LAST)),
        ("TUNNELS_LAST", U32(cave::ROOT_TUNNELS[4])),
        ("SEA_LEVEL", I32(SEA_LEVEL)),
        ("SEA_CELL", I32(ground::SEA_CELL)),
        ("FACE_NONE", I32(i16::MIN as i32)),
        ("EMPTY_LO", I32(ground::EMPTY[0])),
        ("EMPTY_HI", I32(ground::EMPTY[1])),
        ("UNBOUNDED_LO", I32(ground::UNBOUNDED[0])),
        ("UNBOUNDED_HI", I32(ground::UNBOUNDED[1])),
        ("STRATA_FILLER_DEPTH", I32(strata::FILLER_DEPTH)),
        ("STRATA_SANDSTONE_DEPTH", I32(strata::SANDSTONE_DEPTH)),
        ("STRATA_BEDROCK_DEPTH", I32(strata::BEDROCK_DEPTH)),
        ("BOX_NONE", Vec2u(answer::BOX_NONE)),
        ("BOX_WHOLE", Vec2u(answer::BOX_WHOLE)),
        ("TUNNELS_FIRST", Vec4u([a, b, c, d])),
        (
            "NO_CUT",
            Vec4i([none.at[0], none.at[1], none.at[2], none.zoom as i32]),
        ),
    ]);
    out += &table(
        "tunnel_blocks",
        (0..4).map(|k| (k, U32(cave::BLOCKS_A_TUNNEL[k as usize]))),
        U32(cave::BLOCKS_A_TUNNEL[4]),
    );
    out += &table(
        "bound_mip_at",
        (0..=clip::BRICK_MIP_LAST).map(|j| (j, U32(clip::bound_mip_at(j)))),
        U32(clip::bound_mip_at(clip::BRICK_MIP_LAST + 1)),
    );
    out + &predicate("is_ground", answer::PLACED_ASSET, value::is_ground)
        + &predicate("voxel", answer::PLACED_ASSET, object::voxel)
}

/// The constants and tables decor's passes read beyond the world's words, each written from
/// the pxl-content layout or rule that defines it, and pxl-spec `value::is_transport`.
fn decor_word() -> String {
    use core::mem::{offset_of, size_of};
    use pxl_content::generation::{self as g, drawing};
    use pxl_content::param::scatter::ALIGN_NEIGHBOR_RADIUS;
    use pxl_content::scatter;
    use pxl_engine::{answer, cast, world};
    use pxl_spec::asset::decor::{Orientation, Row, DECORS, ROWS};
    use pxl_spec::asset::id::{Kind, IDX_BITS};
    use pxl_spec::share::EIGHTH_CNT;
    use pxl_spec::space::coord::zoom::UNIT_ZOOM;
    use pxl_spec::value;
    const {
        assert!(
            g::TALLEST > 0 && g::WIDEST > 0,
            "decor's tallest and widest are a cell or more"
        );
        assert!(
            UNIT_ZOOM - world::PLACED_FIRST == 3,
            "decor_pose.wgsl `overlaps_placed` descends three levels to the unit cells"
        );
    }
    let row = |at: usize| (at / size_of::<u32>()) as u32;
    let mut out = consts([
        ("LANE_CNT", U32(g::LANE_CNT)),
        ("REC_WORDS", U32(g::REC_WORDS)),
        ("HDR_WORDS", U32(g::HDR_WORDS)),
        ("CNT_AT", U32(g::CNT_AT)),
        ("ENTRY_RECS_AT", U32(g::ENTRY_RECS_AT)),
        ("BRICK_REC_CAP", U32(g::BRICK_REC_CAP)),
        ("UNGENERATED", U32(g::UNGENERATED)),
        ("TURN_SHIFT", U32(g::TURN_SHIFT)),
        ("VARIANT_SHIFT", U32(g::VARIANT_SHIFT)),
        ("TURN_BITS", U32(g::TURN_BITS)),
        ("CELL_ZOOM_SHIFT", U32(g::CELL_ZOOM_SHIFT)),
        ("THIN_ARGS", U32(g::THIN_ARGS)),
        ("THIN_CNT", U32(g::THIN_CNT)),
        ("GENERATE_CNT", U32(g::GENERATE_CNT)),
        ("LIST_RECTS", U32(g::LIST_RECTS)),
        ("RECTS_AT", U32(g::RECTS_AT)),
        ("RECT_WORDS", U32(g::RECT_WORDS)),
        ("RECT_CAP", U32(g::RECT_CAP)),
        ("LIST_EVERY", U32(g::LIST_EVERY)),
        ("LIST_PARTS", U32(g::LIST_PARTS)),
        ("GENERATE_ARGS", U32(g::GENERATE_ARGS)),
        ("ENTRY_REC_CAP", U32(g::ENTRY_REC_CAP)),
        ("PLACED_HDR", U32(g::PLACED_HDR)),
        ("PLACED_HDR_WORDS", U32(g::PLACED_HDR_WORDS)),
        ("RUN", U32(g::RUN)),
        ("RUN_CNT", U32(g::RUN_CNT)),
        ("EYE_ENTRY_CNT", U32(g::EYE_ENTRY_CNT)),
        ("ROW_CHANCE", U32(row(offset_of!(Row, chance)))),
        ("DRAWING_NONE", U32(drawing::NONE)),
        ("DRAWING_REGION_SHIFT", U32(drawing::REGION_SHIFT)),
        ("DRAWING_COLUMN_CNT", U32(drawing::COLUMN_CNT)),
        ("DRAWING_MODEL_ANGLE", U32(drawing::MODEL_ANGLE)),
        ("DRAWING_IMPOSTOR_ANGLE", U32(drawing::IMPOSTOR_ANGLE)),
        ("DRAWING_ANCHOR", U32(drawing::ANCHOR)),
        ("DRAWING_HIDE", U32(drawing::HIDE)),
        ("DRAWING_EXTENTS", U32(drawing::EXTENTS)),
        ("DRAWING_MODEL", U32(drawing::MODEL)),
        ("DRAWING_IMPOSTOR", U32(drawing::IMPOSTOR)),
        ("DRAWING_CULLED", U32(drawing::CULLED)),
        ("LIST_AT", U32(g::LIST_AT)),
        ("LIST_WORDS", U32(g::LIST_WORDS)),
        ("PAIR_DECOR_SHIFT", U32(g::PAIR_DECOR_SHIFT)),
        ("PLACED_TAG", U32(g::PLACED_TAG)),
        ("PLACED_FIRST", U32(world::PLACED_FIRST)),
        ("SCATTER_CNT_TAG", U32(scatter::CNT_TAG)),
        ("SCATTER_SPLIT_TAG", U32(scatter::SPLIT_TAG)),
        ("DECORS", U32(ROWS.len() as u32)),
        ("IDX_BITS", U32(IDX_BITS)),
        ("DECOR_KIND", U32(Kind::Decor as u32)),
        ("ROW_WORDS", U32(row(size_of::<Row>()))),
        ("ROW_SEED", U32(row(offset_of!(Row, seed)))),
        ("ROW_ORIENT", U32(row(offset_of!(Row, orient)))),
        ("ROW_ON", U32(row(offset_of!(Row, on)))),
        ("ROW_VARIANT_CNT", U32(row(offset_of!(Row, variant_cnt)))),
        ("ROW_SIZES", U32(row(offset_of!(Row, sizes)))),
        ("ALONG_TRANSPORT", U32(Orientation::AlongTransport as u32)),
        ("REGION_CNT", U32(drawing::REGION_CNT)),
        ("ARGS_WORDS", U32(drawing::ARGS_WORDS)),
        ("IMPOSTOR_CORNER_CNT", U32(drawing::IMPOSTOR_CORNER_CNT)),
        ("UNIFORM_WORDS", U32(drawing::UNIFORM_WORDS)),
        ("UNIFORM_IDX", U32(word::instance::IDX as u32)),
        ("TALLEST", U32(g::TALLEST as u32)),
        ("WIDEST", U32(g::WIDEST as u32)),
        ("READ_REACH", I32(g::READ_REACH)),
        ("ALIGN_NEIGHBOR_RADIUS", I32(ALIGN_NEIGHBOR_RADIUS)),
        ("CLAIM_EXTENT", Vec3i(g::CLAIM_EXTENT)),
        ("NEAREST_U", F32(drawing::NEAREST_U)),
    ]);
    let last = ROWS.len() - 1;
    let by_decor = |name: &str, of: &dyn Fn(usize) -> Val| {
        table(name, (0..last).map(|k| (k as u32, of(k))), of(last))
    };
    out += &by_decor("decor_first_rec", &|k| U32(g::first_rec(&ROWS, k as u32)));
    out += &by_decor("decor_first_group", &|k| U32(DECORS[k].first_group()));
    out += &by_decor("decor_root", &|k| U32(scatter::ROOTS[k]));
    out += &by_decor("decor_widest", &|k| I32(ROWS[k].widest() as i32));
    // Each turn's spin is written as the bits of the float the Rust computes, so
    // decor_compact.wgsl writes the same spin the CPU does.
    let turns = (1u32 << g::TURN_BITS) - 1;
    let spin = |t: u32| U32((t as f32 * core::f32::consts::FRAC_PI_2).to_bits());
    out += &table("turn_spin", (0..turns).map(|t| (t, spin(t))), spin(turns));
    // How far below its box's top a cell keeping `k` eighths leaves its own top, as a share of
    // the cell's side (`cast::top`, `generation::footing`), read as the Rust's float.
    let sink = |k: u32| {
        let asset = (k % EIGHTH_CNT) << answer::FILL_SHIFT;
        Bits(1.0 - cast::fill_bound(([0.0; 3], [1.0; 3]), asset).1[2])
    };
    out += &table(
        "sink_share",
        (1..EIGHTH_CNT).map(|k| (k, sink(k))),
        sink(EIGHTH_CNT),
    );
    out + &predicate("is_transport", answer::PLACED_ASSET, value::is_transport)
}

/// What the screen pass reads off a ray's words (pxl-engine ray.rs), so every shader reads a
/// field exactly as the cast writes it: where its faces word lies, the ground's leaf cell zoom
/// its cell's zoom is painted against, and its first word's fields. A coarse cell's ray
/// carries the cell's four asset ids in its third word, packed as pxl-spec ground.rs
/// `asset_ids` packs them.
fn ray() -> String {
    use pxl_engine::ray::{
        FACE_BITS, FACE_SHIFT, FACE_WORD, FILL_SHIFT, VAL_BITS, ZOOM_BITS, ZOOM_SHIFT,
    };
    use pxl_spec::ground::GroundId;
    use pxl_spec::share::FILL_BITS;
    use pxl_spec::space::layer::lattice;
    let mut out = consts([
        ("RAY_FACE_WORD", U32(FACE_WORD)),
        ("GROUND_CELL_ZOOM", U32(lattice::GROUND.leaf_cell_zoom())),
    ]);
    for (name, shift, bits) in [
        ("val", 0, VAL_BITS),
        ("face", FACE_SHIFT, FACE_BITS),
        ("zoom", ZOOM_SHIFT, ZOOM_BITS),
        ("fill", FILL_SHIFT, FILL_BITS),
    ] {
        out += &format!(
            "fn ray_{name}(w: u32) -> u32 {{ return (w >> {shift}u) & {}u; }}\n",
            (1u32 << bits) - 1
        );
    }
    let (b, m) = (GroundId::BITS, u32::from(GroundId::MAX));
    out + &format!(
        "fn ray_asset_ids(w: u32) -> vec4u {{ return vec4u(w & {m}u, (w >> {b}u) & {m}u, (w >> {}u) & {m}u, w >> {}u); }}\n",
        2 * b,
        3 * b
    )
}

/// The raster's words (pxl-engine raster.rs) and the corners of a box's vertices, first as
/// raster.rs `corner` winds them and then wound the other way, packed three bits a corner and
/// ten corners a word, which `raster_corner` reads back. The Galaxy S23's Vulkan driver refuses
/// a render pipeline whose vertex stage indexes a constant array of vectors at a place computed
/// as it runs, and takes the same table packed into u32 words. A hit's box that holds the eye
/// is drawn at a depth just short of 1, the depth of a lack. Every float written here is
/// finite, as Tint refuses a literal past f32's range.
fn raster_box() -> String {
    use pxl_engine::raster::{corner, EYED, ID_MASK, LACK, NEAR, STALE, STALE_DEPTH};
    use pxl_spec::space::hierarchy::tree::child_idx;
    let n = word::BOX_VERTEX_CNT;
    let per_word = 32 / 3;
    let bits: Vec<u32> = (0..2 * n)
        .map(|k| corner(k % n, k >= n))
        .map(|c| {
            assert!(
                c.iter().all(|&v| v == 0 || v == 1),
                "a corner's a 0 or a 1 a lane"
            );
            child_idx(c.map(|v| v as i32)) as u32
        })
        .collect();
    let corners = bits
        .chunks(per_word)
        .map(|w| U32(w.iter().enumerate().fold(0, |a, (j, &b)| a | b << (3 * j))))
        .collect();
    consts([
        ("RASTER_CORNERS", Array(corners)),
        ("RASTER_ID_MASK", U32(ID_MASK)),
        ("RASTER_EYED", U32(EYED)),
        ("RASTER_STALE", U32(STALE)),
        ("RASTER_STALE_DEPTH", F32(STALE_DEPTH)),
        ("RASTER_LACK", U32(LACK)),
        ("RASTER_EYED_DEPTH", F32(1.0 - 1.0 / (1u32 << 20) as f32)),
        ("RASTER_NEAR", F32(NEAR)),
    ]) + &format!(
        "fn raster_corner(k: u32) -> vec3f {{\n  let b = (RASTER_CORNERS[k / {per_word}u] >> (3u * (k % {per_word}u))) & 7u;\n  return vec3f(f32(b & 1u), f32((b >> 1u) & 1u), f32(b >> 2u));\n}}\n"
    )
}

/// The light's lay and the reading of a light slot's word as a depth, a share of the light
/// box's depth, as pxl-engine light.rs `word` lays it out. Every shader reads a depth through
/// `light_depth`.
fn light_word() -> String {
    use pxl_engine::light::{DEPTH_ONE, LAY_VERTEX_CNT};
    consts([("LAY_VERTEX_CNT", U32(LAY_VERTEX_CNT))])
        + &format!("fn light_depth(w: u32) -> f32 {{ return f32(w) / {DEPTH_ONE}.0; }}\n")
}

/// The threads of the pass that lists the damaged tiles, beside the tiles' rule in
/// tile_word.wgsl, which the Rust's rule (word.rs `screen::age`, `screen::in_damage`) must be:
/// the age is a wrapping u32 difference, and damage ends at `damage_frame_cnt`.
fn tile_word() -> String {
    assert_eq!(screen::age(0, u32::MAX), 1);
    assert_eq!(screen::age(5, 5), 0);
    assert!(screen::in_damage(0, 1) && !screen::in_damage(1, 1));
    assert!(screen::in_damage(u32::MAX - 1, u32::MAX) && !screen::in_damage(u32::MAX, u32::MAX));
    consts([("CAST_TILES_LANE_CNT", U32(screen::CAST_TILES_LANE_CNT))])
}

/// The cloud lattice's cells, and the sheets' noise hash from pxl-content cloud.rs's
/// multipliers, rounds and output bits.
fn cloud_hash() -> String {
    use pxl_content::cloud::{HASH_IN, HASH_LAST, HASH_ROUNDS, HASH_SHARE_BITS};
    use pxl_content::param::cloud::LATTICE_SHAMT;
    let [hx, hy, hs] = HASH_IN;
    let rounds: String = HASH_ROUNDS
        .iter()
        .map(|[shift, mul]| format!("h ^= h >> {shift}u; h *= {mul}u; "))
        .collect();
    consts([("CLOUD_LATTICE_CELLS", U32(1 << LATTICE_SHAMT))])
        + &format!(
            "fn lattice_hash(i: vec2u, salt: u32) -> f32 {{ var h = (i.x * {hx}u) ^ (i.y * {hy}u) ^ (salt * {hs}u); {rounds}h ^= h >> {HASH_LAST}u; return f32(h >> {}u) / {}.0; }}\n",
            32 - HASH_SHARE_BITS,
            1u32 << HASH_SHARE_BITS
        )
}

/// The debug views' looks (debug_look.json), each decoded to linear light, as the world's
/// color and the canvas's blend it before it is encoded once (finish.wgsl), so each shows as
/// its sRGB; the ramp read straight between its stops.
fn debug_look() -> String {
    use debug_look::{grid, share, steps};
    use pxl_spec::color::rgb;
    use pxl_spec::space::camera::grid::SPANS;
    // A world's cell spans (1, `SPANS`] of the view's cells where the rays stop on it
    // (pxl-engine grid.rs `cell_zoom_at`), so its share of a cell squared lies within
    // (1, `SPANS`²]: the tints' bounds are those.
    const {
        assert!(
            share::finer::MOST == 1.0 && share::within::MOST == SPANS * SPANS,
            "debug_look.json's share bounds are the split's"
        );
    }
    let light = |c: &str| color::linear_rgb(rgb(c)).map(|v| v as f32);
    let [r, g, b] = light(grid::RGB);
    let last = steps::RAMP.len() - 1;
    consts([
        ("GRID_COLOR", Vec4f([r, g, b, grid::ALPHA])),
        ("SHARE_FINER", Vec3f(light(share::finer::RGB))),
        ("SHARE_FINER_MOST", F32(share::finer::MOST)),
        ("SHARE_WITHIN", Vec3f(light(share::within::RGB))),
        ("SHARE_WITHIN_MOST", F32(share::within::MOST)),
        ("SHARE_COARSER", Vec3f(light(share::coarser::RGB))),
        ("STEPS_MOST", F32(steps::MOST)),
        ("STEPS_NONE", Vec3f(light(steps::NONE))),
        (
            "STEPS_RAMP",
            Array(steps::RAMP.iter().map(|&c| Vec3f(light(c))).collect()),
        ),
    ]) + &format!(
        "fn steps_ramp(x: f32) -> vec3f {{ let f = clamp(x, 0.0, 1.0) * {last}.0; let i = min(u32(f), {}u); return mix(STEPS_RAMP[i], STEPS_RAMP[i + 1u], f - f32(i)); }}\n",
        last - 1
    )
}

/// An accessor for each field of every uniform `text` reads (frame/shader/word.rs `UNIFORMS`),
/// which reads the field off the vec4s where the host wrote it. A uniform is never declared as
/// a struct, as one crashed the GPU process of Safari's WebGPU. Each field is checked to lie
/// within its vec4s and apart from the others.
pub fn accessors(text: &str) -> String {
    use word::{floats, Kind, UNIFORMS};
    let mut out = String::new();
    for (uniform, fields) in UNIFORMS {
        let words = format!("{uniform}_words");
        // A shader reads a uniform from its own `_words` binding, or, where the uniform lies in
        // a list, through its `_word` function. A shader that does neither gets no accessors.
        let row_of: fn(&str, &str, usize) -> String =
            if text.contains(&format!("var<uniform> {words}:")) {
                |words, _, row| format!("{words}[{row}]")
            } else if text.contains(&format!("fn {uniform}_word(")) {
                |_, uniform, row| format!("{uniform}_word({row}u)")
            } else {
                continue;
            };
        let at = |row: usize| row_of(&words, uniform, row);
        let mut taken = vec![false; floats(fields)];
        for f in fields {
            let (row, col, n) = (f.at / 4, f.at % 4, f.kind.floats());
            for t in &mut taken[f.at..f.at + n] {
                assert!(!*t, "{uniform}'s {} lies over another field", f.name);
                *t = true;
            }
            let lanes = format!("{}.{}", at(row), &"xyzw"[col..(col + n).min(4)]);
            let column = |k: usize| format!("{}.xyz", at(row + k));
            let (ty, read) = match f.kind {
                Kind::F32 => ("f32", lanes),
                Kind::U32 => ("u32", format!("bitcast<u32>({lanes})")),
                Kind::Vec2 => ("vec2f", lanes),
                Kind::Vec2u => ("vec2u", format!("bitcast<vec2u>({lanes})")),
                Kind::Vec2i => ("vec2i", format!("bitcast<vec2i>({lanes})")),
                Kind::Vec3 => ("vec3f", lanes),
                Kind::Vec3i => ("vec3i", format!("bitcast<vec3i>({lanes})")),
                Kind::Vec4i => ("vec4i", format!("bitcast<vec4i>({})", at(row))),
                Kind::Vec4u => ("vec4u", format!("bitcast<vec4u>({})", at(row))),
                Kind::Mat3 => (
                    "mat3x3f",
                    format!("mat3x3f({}, {}, {})", column(0), column(1), column(2)),
                ),
                Kind::Mat4 => (
                    "mat4x4f",
                    format!(
                        "mat4x4f({}, {}, {}, {})",
                        at(row),
                        at(row + 1),
                        at(row + 2),
                        at(row + 3)
                    ),
                ),
            };
            let whole = matches!(f.kind, Kind::Vec4i | Kind::Vec4u | Kind::Mat3 | Kind::Mat4);
            assert!(
                if whole { col == 0 } else { col + n <= 4 },
                "{uniform}'s {} crosses a vec4",
                f.name
            );
            out += &format!(
                "fn {uniform}_{}() -> {ty} {{ return {read}; }}\n",
                f.name.to_lowercase()
            );
        }
    }
    out
}
