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

pub const GPU_SPAN_NAMES: usize = 26;
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
#[repr(u8)]
pub enum GpuSpanName {
    air = 0,
    prepass = 1,
    ao = 2,
    world = 3,
    resolve = 4,
    finish = 5,
    marker = 6,
    debug = 7,
    case = 8,
    fill = 9,
    cast = 10,
    beam = 11,
    cell = 12,
    sample = 13,
    sraster = 14,
    px = 15,
    miss = 16,
    light_beam = 17,
    light_sample = 18,
    light_raster = 19,
    light_slot = 20,
    light_walk = 21,
    generate = 22,
    shadow = 23,
    face = 24,
    mark = 25,
}

impl GpuSpanName {
    /// Every member, in the JSON's own order.
    pub const ALL: [GpuSpanName; 26] = [
        GpuSpanName::air,
        GpuSpanName::prepass,
        GpuSpanName::ao,
        GpuSpanName::world,
        GpuSpanName::resolve,
        GpuSpanName::finish,
        GpuSpanName::marker,
        GpuSpanName::debug,
        GpuSpanName::case,
        GpuSpanName::fill,
        GpuSpanName::cast,
        GpuSpanName::beam,
        GpuSpanName::cell,
        GpuSpanName::sample,
        GpuSpanName::sraster,
        GpuSpanName::px,
        GpuSpanName::miss,
        GpuSpanName::light_beam,
        GpuSpanName::light_sample,
        GpuSpanName::light_raster,
        GpuSpanName::light_slot,
        GpuSpanName::light_walk,
        GpuSpanName::generate,
        GpuSpanName::shadow,
        GpuSpanName::face,
        GpuSpanName::mark,
    ];

    /// The member a number names, or none for a number the set does not
    /// give out.
    pub fn of(val: u8) -> Option<GpuSpanName> {
        GpuSpanName::ALL
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
