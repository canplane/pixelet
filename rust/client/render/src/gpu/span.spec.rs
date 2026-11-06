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

pub const GPU_SPAN_NAMES: usize = 39;
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
    preview = 9,
    fill = 10,
    cast = 11,
    beam_hit = 12,
    beam = 13,
    cell = 14,
    sample_hit = 15,
    sample = 16,
    sample_box = 17,
    sraster = 18,
    px = 19,
    miss = 20,
    light_receivers = 21,
    light_draws = 22,
    light_slide = 23,
    light_propagate = 24,
    light_claim = 25,
    light_beam_hit = 26,
    light_beam = 27,
    light_sample = 28,
    light_raster = 29,
    light_slot = 30,
    light_hit = 31,
    light_walk = 32,
    generate = 33,
    shadow = 34,
    face = 35,
    mark = 36,
    light_mark = 37,
    propagate = 38,
}

impl GpuSpanName {
    /// Every member, in the JSON's own order.
    pub const ALL: [GpuSpanName; 39] = [
        GpuSpanName::air,
        GpuSpanName::exposure,
        GpuSpanName::ao,
        GpuSpanName::world,
        GpuSpanName::world_rest,
        GpuSpanName::resolve,
        GpuSpanName::finish,
        GpuSpanName::marker,
        GpuSpanName::debug,
        GpuSpanName::preview,
        GpuSpanName::fill,
        GpuSpanName::cast,
        GpuSpanName::beam_hit,
        GpuSpanName::beam,
        GpuSpanName::cell,
        GpuSpanName::sample_hit,
        GpuSpanName::sample,
        GpuSpanName::sample_box,
        GpuSpanName::sraster,
        GpuSpanName::px,
        GpuSpanName::miss,
        GpuSpanName::light_receivers,
        GpuSpanName::light_draws,
        GpuSpanName::light_slide,
        GpuSpanName::light_propagate,
        GpuSpanName::light_claim,
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
        GpuSpanName::mark,
        GpuSpanName::light_mark,
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
        GpuSpanName::miss,
        GpuSpanName::light_receivers,
        GpuSpanName::mark,
        GpuSpanName::propagate,
    ];
    /// The members whose `per_cell` is true, in the JSON's order.
    pub const PER_CELL: [GpuSpanName; 3] =
        [GpuSpanName::beam_hit, GpuSpanName::beam, GpuSpanName::cell];
}

pub const LIGHT_MOVED: usize = 6;
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
#[repr(u8)]
pub enum LightMoved {
    content = 0,
    slide = 1,
    receivers = 2,
    footprint = 3,
    world = 4,
    owed = 5,
}

impl LightMoved {
    /// Every member, in the JSON's own order.
    pub const ALL: [LightMoved; 6] = [
        LightMoved::content,
        LightMoved::slide,
        LightMoved::receivers,
        LightMoved::footprint,
        LightMoved::world,
        LightMoved::owed,
    ];

    /// The member a number names, or none for a number the set does not
    /// give out.
    pub fn of(val: u8) -> Option<LightMoved> {
        LightMoved::ALL
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
    held = 2,
    dropped = 3,
    plain = 4,
    standing = 5,
    cast = 6,
    stood = 7,
    px = 8,
    gated = 9,
}

impl FrameKind {
    /// Every member, in the JSON's own order.
    pub const ALL: [FrameKind; 10] = [
        FrameKind::none,
        FrameKind::unready,
        FrameKind::held,
        FrameKind::dropped,
        FrameKind::plain,
        FrameKind::standing,
        FrameKind::cast,
        FrameKind::stood,
        FrameKind::px,
        FrameKind::gated,
    ];

    /// The member a number names, or none for a number the set does not
    /// give out.
    pub fn of(val: u8) -> Option<FrameKind> {
        FrameKind::ALL
            .into_iter()
            .find(|member| *member as u8 == val)
    }
}
