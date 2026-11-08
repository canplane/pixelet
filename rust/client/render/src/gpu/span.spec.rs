// pxl-render/gpu/span.spec.rs
//
// GENERATED from spec/client/render/gpu/span.json
//
// What the renderer reports of a frame (pxl-render gpu/renderer.rs): the GPU spans it is timed in,
// in the order the renderer encodes them, and the kind of frame it made; the profiler records and reports
// them by these names.

// Names as the spec spells them, so one record has one name on every side.
// A table is generated whole, whether or not this crate calls an entry.
#![allow(
    non_upper_case_globals,
    non_snake_case,
    clippy::absolute_paths,
    non_camel_case_types,
    dead_code,
    reason = "generated from the spec"
)]

pub const GPU_SPAN_NAMES: usize = 38;
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
#[repr(u8)]
pub enum GpuSpanName {
    air = 0,
    exposure = 1,
    ao = 2,
    world = 3,
    world_rest = 4,
    resolve = 5,
    finish = 6,
    marker = 7,
    debug = 8,
    character = 9,
    cast = 10,
    beam_hit = 11,
    beam = 12,
    grid_cell = 13,
    sample_hit = 14,
    sample = 15,
    raster_box = 16,
    sraster = 17,
    px = 18,
    hole = 19,
    light_world_receivers = 20,
    light_draws = 21,
    light_slide = 22,
    light_propagate = 23,
    light_damage = 24,
    light_beam_hit = 25,
    light_beam = 26,
    light_sample = 27,
    light_raster = 28,
    light_slot = 29,
    light_hit = 30,
    light_walk = 31,
    generate = 32,
    shadow = 33,
    face = 34,
    damage = 35,
    shadow_damage = 36,
    propagate = 37,
}

impl GpuSpanName {
    /// Every member, in the JSON's own order.
    pub const ALL: [GpuSpanName; 38] = [
        GpuSpanName::air,
        GpuSpanName::exposure,
        GpuSpanName::ao,
        GpuSpanName::world,
        GpuSpanName::world_rest,
        GpuSpanName::resolve,
        GpuSpanName::finish,
        GpuSpanName::marker,
        GpuSpanName::debug,
        GpuSpanName::character,
        GpuSpanName::cast,
        GpuSpanName::beam_hit,
        GpuSpanName::beam,
        GpuSpanName::grid_cell,
        GpuSpanName::sample_hit,
        GpuSpanName::sample,
        GpuSpanName::raster_box,
        GpuSpanName::sraster,
        GpuSpanName::px,
        GpuSpanName::hole,
        GpuSpanName::light_world_receivers,
        GpuSpanName::light_draws,
        GpuSpanName::light_slide,
        GpuSpanName::light_propagate,
        GpuSpanName::light_damage,
        GpuSpanName::light_beam_hit,
        GpuSpanName::light_beam,
        GpuSpanName::light_sample,
        GpuSpanName::light_raster,
        GpuSpanName::light_slot,
        GpuSpanName::light_hit,
        GpuSpanName::light_walk,
        GpuSpanName::generate,
        GpuSpanName::shadow,
        GpuSpanName::face,
        GpuSpanName::damage,
        GpuSpanName::shadow_damage,
        GpuSpanName::propagate,
    ];

    /// The member a number names, or none for a number the set does not
    /// give out.
    pub fn of(val: u8) -> Option<GpuSpanName> {
        GpuSpanName::ALL
            .into_iter()
            .find(|member| *member as u8 == val)
    }
}

impl GpuSpanName {
    /// The members whose `per_px` is true, in the JSON's order.
    pub const PER_PX: [GpuSpanName; 10] = [
        GpuSpanName::ao,
        GpuSpanName::world,
        GpuSpanName::world_rest,
        GpuSpanName::resolve,
        GpuSpanName::sraster,
        GpuSpanName::px,
        GpuSpanName::hole,
        GpuSpanName::light_world_receivers,
        GpuSpanName::damage,
        GpuSpanName::propagate,
    ];
}

pub const LIGHT_CHANGED: usize = 5;
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
#[repr(u8)]
pub enum LightChanged {
    content = 0,
    slide = 1,
    receivers = 2,
    footprint = 3,
    world = 4,
}

impl LightChanged {
    /// Every member, in the JSON's own order.
    pub const ALL: [LightChanged; 5] = [
        LightChanged::content,
        LightChanged::slide,
        LightChanged::receivers,
        LightChanged::footprint,
        LightChanged::world,
    ];

    /// The member a number names, or none for a number the set does not
    /// give out.
    pub fn of(val: u8) -> Option<LightChanged> {
        LightChanged::ALL
            .into_iter()
            .find(|member| *member as u8 == val)
    }
}

pub const FRAME_KINDS: usize = 10;
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
#[repr(u8)]
pub enum FrameKind {
    none = 0,
    unready = 1,
    stream_busy = 2,
    dropped = 3,
    plain = 4,
    still = 5,
    cast = 6,
    unchanged = 7,
    px = 8,
    masked = 9,
}

impl FrameKind {
    /// Every member, in the JSON's own order.
    pub const ALL: [FrameKind; 10] = [
        FrameKind::none,
        FrameKind::unready,
        FrameKind::stream_busy,
        FrameKind::dropped,
        FrameKind::plain,
        FrameKind::still,
        FrameKind::cast,
        FrameKind::unchanged,
        FrameKind::px,
        FrameKind::masked,
    ];

    /// The member a number names, or none for a number the set does not
    /// give out.
    pub fn of(val: u8) -> Option<FrameKind> {
        FrameKind::ALL
            .into_iter()
            .find(|member| *member as u8 == val)
    }
}
