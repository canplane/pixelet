// pxl-render/gpu/span.spec.rs
//
// GENERATED from spec/client/render/gpu/span.json (xpute-gen data.rs)
//
// What the renderer reports of a frame (pxl-render renderer.rs): the GPU spans it is timed in,
// in the order the renderer encodes them, and the kind of frame it made; the profiler records and reports
// them by these names.

// A table is generated whole, whether or not this crate calls an entry.
#![allow(dead_code, reason = "generated from the spec")]

pub const GPU_SPAN_NAMES: &[&str] = &[
    "air",
    "exposure",
    "ao",
    "world",
    "world_rest",
    "resolve",
    "finish",
    "marker",
    "debug",
    "character",
    "cast",
    "beam_hit",
    "beam",
    "grid_cell",
    "sample_hit",
    "sample",
    "raster_box",
    "sraster",
    "px",
    "hole",
    "light_world_receivers",
    "light_draws",
    "light_slide",
    "light_propagate",
    "light_damage",
    "light_beam_hit",
    "light_beam",
    "light_sample",
    "light_raster",
    "light_slot",
    "light_hit",
    "light_walk",
    "generate",
    "shadow",
    "face",
    "damage",
    "shadow_damage",
    "propagate",
];
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
#[repr(u8)]
pub enum GpuSpanName {
    Air = 0,
    Exposure = 1,
    Ao = 2,
    World = 3,
    WorldRest = 4,
    Resolve = 5,
    Finish = 6,
    Marker = 7,
    Debug = 8,
    Character = 9,
    Cast = 10,
    BeamHit = 11,
    Beam = 12,
    GridCell = 13,
    SampleHit = 14,
    Sample = 15,
    RasterBox = 16,
    Sraster = 17,
    Px = 18,
    Hole = 19,
    LightWorldReceivers = 20,
    LightDraws = 21,
    LightSlide = 22,
    LightPropagate = 23,
    LightDamage = 24,
    LightBeamHit = 25,
    LightBeam = 26,
    LightSample = 27,
    LightRaster = 28,
    LightSlot = 29,
    LightHit = 30,
    LightWalk = 31,
    Generate = 32,
    Shadow = 33,
    Face = 34,
    Damage = 35,
    ShadowDamage = 36,
    Propagate = 37,
}

impl GpuSpanName {
    /// Every member, in its source's own order.
    pub const ALL: [GpuSpanName; 38] = [
        GpuSpanName::Air,
        GpuSpanName::Exposure,
        GpuSpanName::Ao,
        GpuSpanName::World,
        GpuSpanName::WorldRest,
        GpuSpanName::Resolve,
        GpuSpanName::Finish,
        GpuSpanName::Marker,
        GpuSpanName::Debug,
        GpuSpanName::Character,
        GpuSpanName::Cast,
        GpuSpanName::BeamHit,
        GpuSpanName::Beam,
        GpuSpanName::GridCell,
        GpuSpanName::SampleHit,
        GpuSpanName::Sample,
        GpuSpanName::RasterBox,
        GpuSpanName::Sraster,
        GpuSpanName::Px,
        GpuSpanName::Hole,
        GpuSpanName::LightWorldReceivers,
        GpuSpanName::LightDraws,
        GpuSpanName::LightSlide,
        GpuSpanName::LightPropagate,
        GpuSpanName::LightDamage,
        GpuSpanName::LightBeamHit,
        GpuSpanName::LightBeam,
        GpuSpanName::LightSample,
        GpuSpanName::LightRaster,
        GpuSpanName::LightSlot,
        GpuSpanName::LightHit,
        GpuSpanName::LightWalk,
        GpuSpanName::Generate,
        GpuSpanName::Shadow,
        GpuSpanName::Face,
        GpuSpanName::Damage,
        GpuSpanName::ShadowDamage,
        GpuSpanName::Propagate,
    ];

    /// The member `val` is, or none for a number no member has.
    pub const fn of(val: u8) -> Option<GpuSpanName> {
        match val {
            0 => Some(GpuSpanName::Air),
            1 => Some(GpuSpanName::Exposure),
            2 => Some(GpuSpanName::Ao),
            3 => Some(GpuSpanName::World),
            4 => Some(GpuSpanName::WorldRest),
            5 => Some(GpuSpanName::Resolve),
            6 => Some(GpuSpanName::Finish),
            7 => Some(GpuSpanName::Marker),
            8 => Some(GpuSpanName::Debug),
            9 => Some(GpuSpanName::Character),
            10 => Some(GpuSpanName::Cast),
            11 => Some(GpuSpanName::BeamHit),
            12 => Some(GpuSpanName::Beam),
            13 => Some(GpuSpanName::GridCell),
            14 => Some(GpuSpanName::SampleHit),
            15 => Some(GpuSpanName::Sample),
            16 => Some(GpuSpanName::RasterBox),
            17 => Some(GpuSpanName::Sraster),
            18 => Some(GpuSpanName::Px),
            19 => Some(GpuSpanName::Hole),
            20 => Some(GpuSpanName::LightWorldReceivers),
            21 => Some(GpuSpanName::LightDraws),
            22 => Some(GpuSpanName::LightSlide),
            23 => Some(GpuSpanName::LightPropagate),
            24 => Some(GpuSpanName::LightDamage),
            25 => Some(GpuSpanName::LightBeamHit),
            26 => Some(GpuSpanName::LightBeam),
            27 => Some(GpuSpanName::LightSample),
            28 => Some(GpuSpanName::LightRaster),
            29 => Some(GpuSpanName::LightSlot),
            30 => Some(GpuSpanName::LightHit),
            31 => Some(GpuSpanName::LightWalk),
            32 => Some(GpuSpanName::Generate),
            33 => Some(GpuSpanName::Shadow),
            34 => Some(GpuSpanName::Face),
            35 => Some(GpuSpanName::Damage),
            36 => Some(GpuSpanName::ShadowDamage),
            37 => Some(GpuSpanName::Propagate),
            _ => None,
        }
    }

    /// The member's name, as its source spells it.
    pub const fn name(self) -> &'static str {
        match self {
            GpuSpanName::Air => "air",
            GpuSpanName::Exposure => "exposure",
            GpuSpanName::Ao => "ao",
            GpuSpanName::World => "world",
            GpuSpanName::WorldRest => "world_rest",
            GpuSpanName::Resolve => "resolve",
            GpuSpanName::Finish => "finish",
            GpuSpanName::Marker => "marker",
            GpuSpanName::Debug => "debug",
            GpuSpanName::Character => "character",
            GpuSpanName::Cast => "cast",
            GpuSpanName::BeamHit => "beam_hit",
            GpuSpanName::Beam => "beam",
            GpuSpanName::GridCell => "grid_cell",
            GpuSpanName::SampleHit => "sample_hit",
            GpuSpanName::Sample => "sample",
            GpuSpanName::RasterBox => "raster_box",
            GpuSpanName::Sraster => "sraster",
            GpuSpanName::Px => "px",
            GpuSpanName::Hole => "hole",
            GpuSpanName::LightWorldReceivers => "light_world_receivers",
            GpuSpanName::LightDraws => "light_draws",
            GpuSpanName::LightSlide => "light_slide",
            GpuSpanName::LightPropagate => "light_propagate",
            GpuSpanName::LightDamage => "light_damage",
            GpuSpanName::LightBeamHit => "light_beam_hit",
            GpuSpanName::LightBeam => "light_beam",
            GpuSpanName::LightSample => "light_sample",
            GpuSpanName::LightRaster => "light_raster",
            GpuSpanName::LightSlot => "light_slot",
            GpuSpanName::LightHit => "light_hit",
            GpuSpanName::LightWalk => "light_walk",
            GpuSpanName::Generate => "generate",
            GpuSpanName::Shadow => "shadow",
            GpuSpanName::Face => "face",
            GpuSpanName::Damage => "damage",
            GpuSpanName::ShadowDamage => "shadow_damage",
            GpuSpanName::Propagate => "propagate",
        }
    }
}

pub const GPU_SPAN_PER_PX: [GpuSpanName; 10] = [
    GpuSpanName::Ao,
    GpuSpanName::World,
    GpuSpanName::WorldRest,
    GpuSpanName::Resolve,
    GpuSpanName::Sraster,
    GpuSpanName::Px,
    GpuSpanName::Hole,
    GpuSpanName::LightWorldReceivers,
    GpuSpanName::Damage,
    GpuSpanName::Propagate,
];

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
#[repr(u8)]
pub enum LightChanged {
    Content = 0,
    Slide = 1,
    Receivers = 2,
    Footprint = 3,
    World = 4,
}

impl LightChanged {
    /// Every member, in its source's own order.
    pub const ALL: [LightChanged; 5] = [
        LightChanged::Content,
        LightChanged::Slide,
        LightChanged::Receivers,
        LightChanged::Footprint,
        LightChanged::World,
    ];

    /// The member `val` is, or none for a number no member has.
    pub const fn of(val: u8) -> Option<LightChanged> {
        match val {
            0 => Some(LightChanged::Content),
            1 => Some(LightChanged::Slide),
            2 => Some(LightChanged::Receivers),
            3 => Some(LightChanged::Footprint),
            4 => Some(LightChanged::World),
            _ => None,
        }
    }

    /// The member's name, as its source spells it.
    pub const fn name(self) -> &'static str {
        match self {
            LightChanged::Content => "content",
            LightChanged::Slide => "slide",
            LightChanged::Receivers => "receivers",
            LightChanged::Footprint => "footprint",
            LightChanged::World => "world",
        }
    }
}

pub const FRAME_KINDS: &[&str] = &[
    "none",
    "unready",
    "dropped",
    "plain",
    "still",
    "cast",
    "unchanged",
    "px",
    "masked",
];
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
#[repr(u8)]
pub enum FrameKind {
    None = 0,
    Unready = 1,
    Dropped = 2,
    Plain = 3,
    Still = 4,
    Cast = 5,
    Unchanged = 6,
    Px = 7,
    Masked = 8,
}

impl FrameKind {
    /// Every member, in its source's own order.
    pub const ALL: [FrameKind; 9] = [
        FrameKind::None,
        FrameKind::Unready,
        FrameKind::Dropped,
        FrameKind::Plain,
        FrameKind::Still,
        FrameKind::Cast,
        FrameKind::Unchanged,
        FrameKind::Px,
        FrameKind::Masked,
    ];

    /// The member `val` is, or none for a number no member has.
    pub const fn of(val: u8) -> Option<FrameKind> {
        match val {
            0 => Some(FrameKind::None),
            1 => Some(FrameKind::Unready),
            2 => Some(FrameKind::Dropped),
            3 => Some(FrameKind::Plain),
            4 => Some(FrameKind::Still),
            5 => Some(FrameKind::Cast),
            6 => Some(FrameKind::Unchanged),
            7 => Some(FrameKind::Px),
            8 => Some(FrameKind::Masked),
            _ => None,
        }
    }

    /// The member's name, as its source spells it.
    pub const fn name(self) -> &'static str {
        match self {
            FrameKind::None => "none",
            FrameKind::Unready => "unready",
            FrameKind::Dropped => "dropped",
            FrameKind::Plain => "plain",
            FrameKind::Still => "still",
            FrameKind::Cast => "cast",
            FrameKind::Unchanged => "unchanged",
            FrameKind::Px => "px",
            FrameKind::Masked => "masked",
        }
    }
}
