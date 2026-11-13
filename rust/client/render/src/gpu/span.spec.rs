// pxl-render/gpu/span.spec.rs
//
// GENERATED from spec/client/render/gpu/span.json (xpute-gen data.rs)
//
// What the renderer reports of a frame (pxl-render renderer.rs): the GPU spans it is timed in,
// in the order the renderer encodes them, and the kind of frame it made; the profiler records and reports
// them by these names.

// Names as the spec spells them, so one record has one name on every side.
// A table is generated whole, whether or not this crate calls an entry.
#![allow(
    non_upper_case_globals,
    non_snake_case,
    non_camel_case_types,
    dead_code,
    reason = "generated from the spec"
)]

#[derive(Clone, Copy, Debug)]
pub struct GpuSpanNameRecord {
    pub name: &'static str,
    pub per_px: bool,
}

pub const GPU_SPAN_NAMES: [GpuSpanNameRecord; 38] = [
    GpuSpanNameRecord {
        name: "air",
        per_px: false,
    },
    GpuSpanNameRecord {
        name: "exposure",
        per_px: false,
    },
    GpuSpanNameRecord {
        name: "ao",
        per_px: true,
    },
    GpuSpanNameRecord {
        name: "world",
        per_px: true,
    },
    GpuSpanNameRecord {
        name: "world_rest",
        per_px: true,
    },
    GpuSpanNameRecord {
        name: "resolve",
        per_px: true,
    },
    GpuSpanNameRecord {
        name: "finish",
        per_px: false,
    },
    GpuSpanNameRecord {
        name: "marker",
        per_px: false,
    },
    GpuSpanNameRecord {
        name: "debug",
        per_px: false,
    },
    GpuSpanNameRecord {
        name: "character",
        per_px: false,
    },
    GpuSpanNameRecord {
        name: "cast",
        per_px: false,
    },
    GpuSpanNameRecord {
        name: "beam_hit",
        per_px: false,
    },
    GpuSpanNameRecord {
        name: "beam",
        per_px: false,
    },
    GpuSpanNameRecord {
        name: "grid_cell",
        per_px: false,
    },
    GpuSpanNameRecord {
        name: "sample_hit",
        per_px: false,
    },
    GpuSpanNameRecord {
        name: "sample",
        per_px: false,
    },
    GpuSpanNameRecord {
        name: "raster_box",
        per_px: false,
    },
    GpuSpanNameRecord {
        name: "sraster",
        per_px: true,
    },
    GpuSpanNameRecord {
        name: "px",
        per_px: true,
    },
    GpuSpanNameRecord {
        name: "hole",
        per_px: true,
    },
    GpuSpanNameRecord {
        name: "light_world_receivers",
        per_px: true,
    },
    GpuSpanNameRecord {
        name: "light_draws",
        per_px: false,
    },
    GpuSpanNameRecord {
        name: "light_slide",
        per_px: false,
    },
    GpuSpanNameRecord {
        name: "light_propagate",
        per_px: false,
    },
    GpuSpanNameRecord {
        name: "light_damage",
        per_px: false,
    },
    GpuSpanNameRecord {
        name: "light_beam_hit",
        per_px: false,
    },
    GpuSpanNameRecord {
        name: "light_beam",
        per_px: false,
    },
    GpuSpanNameRecord {
        name: "light_sample",
        per_px: false,
    },
    GpuSpanNameRecord {
        name: "light_raster",
        per_px: false,
    },
    GpuSpanNameRecord {
        name: "light_slot",
        per_px: false,
    },
    GpuSpanNameRecord {
        name: "light_hit",
        per_px: false,
    },
    GpuSpanNameRecord {
        name: "light_walk",
        per_px: false,
    },
    GpuSpanNameRecord {
        name: "generate",
        per_px: false,
    },
    GpuSpanNameRecord {
        name: "shadow",
        per_px: false,
    },
    GpuSpanNameRecord {
        name: "face",
        per_px: false,
    },
    GpuSpanNameRecord {
        name: "damage",
        per_px: true,
    },
    GpuSpanNameRecord {
        name: "shadow_damage",
        per_px: false,
    },
    GpuSpanNameRecord {
        name: "propagate",
        per_px: true,
    },
];

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
    /// Every member, in its source's own order.
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

    /// The member `val` is, or none for a number no member has.
    pub const fn of(val: u8) -> Option<GpuSpanName> {
        match val {
            0 => Some(GpuSpanName::air),
            1 => Some(GpuSpanName::exposure),
            2 => Some(GpuSpanName::ao),
            3 => Some(GpuSpanName::world),
            4 => Some(GpuSpanName::world_rest),
            5 => Some(GpuSpanName::resolve),
            6 => Some(GpuSpanName::finish),
            7 => Some(GpuSpanName::marker),
            8 => Some(GpuSpanName::debug),
            9 => Some(GpuSpanName::character),
            10 => Some(GpuSpanName::cast),
            11 => Some(GpuSpanName::beam_hit),
            12 => Some(GpuSpanName::beam),
            13 => Some(GpuSpanName::grid_cell),
            14 => Some(GpuSpanName::sample_hit),
            15 => Some(GpuSpanName::sample),
            16 => Some(GpuSpanName::raster_box),
            17 => Some(GpuSpanName::sraster),
            18 => Some(GpuSpanName::px),
            19 => Some(GpuSpanName::hole),
            20 => Some(GpuSpanName::light_world_receivers),
            21 => Some(GpuSpanName::light_draws),
            22 => Some(GpuSpanName::light_slide),
            23 => Some(GpuSpanName::light_propagate),
            24 => Some(GpuSpanName::light_damage),
            25 => Some(GpuSpanName::light_beam_hit),
            26 => Some(GpuSpanName::light_beam),
            27 => Some(GpuSpanName::light_sample),
            28 => Some(GpuSpanName::light_raster),
            29 => Some(GpuSpanName::light_slot),
            30 => Some(GpuSpanName::light_hit),
            31 => Some(GpuSpanName::light_walk),
            32 => Some(GpuSpanName::generate),
            33 => Some(GpuSpanName::shadow),
            34 => Some(GpuSpanName::face),
            35 => Some(GpuSpanName::damage),
            36 => Some(GpuSpanName::shadow_damage),
            37 => Some(GpuSpanName::propagate),
            _ => None,
        }
    }

    /// The member's name, as its source spells it.
    pub const fn name(self) -> &'static str {
        match self {
            GpuSpanName::air => "air",
            GpuSpanName::exposure => "exposure",
            GpuSpanName::ao => "ao",
            GpuSpanName::world => "world",
            GpuSpanName::world_rest => "world_rest",
            GpuSpanName::resolve => "resolve",
            GpuSpanName::finish => "finish",
            GpuSpanName::marker => "marker",
            GpuSpanName::debug => "debug",
            GpuSpanName::character => "character",
            GpuSpanName::cast => "cast",
            GpuSpanName::beam_hit => "beam_hit",
            GpuSpanName::beam => "beam",
            GpuSpanName::grid_cell => "grid_cell",
            GpuSpanName::sample_hit => "sample_hit",
            GpuSpanName::sample => "sample",
            GpuSpanName::raster_box => "raster_box",
            GpuSpanName::sraster => "sraster",
            GpuSpanName::px => "px",
            GpuSpanName::hole => "hole",
            GpuSpanName::light_world_receivers => "light_world_receivers",
            GpuSpanName::light_draws => "light_draws",
            GpuSpanName::light_slide => "light_slide",
            GpuSpanName::light_propagate => "light_propagate",
            GpuSpanName::light_damage => "light_damage",
            GpuSpanName::light_beam_hit => "light_beam_hit",
            GpuSpanName::light_beam => "light_beam",
            GpuSpanName::light_sample => "light_sample",
            GpuSpanName::light_raster => "light_raster",
            GpuSpanName::light_slot => "light_slot",
            GpuSpanName::light_hit => "light_hit",
            GpuSpanName::light_walk => "light_walk",
            GpuSpanName::generate => "generate",
            GpuSpanName::shadow => "shadow",
            GpuSpanName::face => "face",
            GpuSpanName::damage => "damage",
            GpuSpanName::shadow_damage => "shadow_damage",
            GpuSpanName::propagate => "propagate",
        }
    }
}

pub const LIGHT_CHANGED: &[&str] = &["content", "slide", "receivers", "footprint", "world"];
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
    /// Every member, in its source's own order.
    pub const ALL: [LightChanged; 5] = [
        LightChanged::content,
        LightChanged::slide,
        LightChanged::receivers,
        LightChanged::footprint,
        LightChanged::world,
    ];

    /// The member `val` is, or none for a number no member has.
    pub const fn of(val: u8) -> Option<LightChanged> {
        match val {
            0 => Some(LightChanged::content),
            1 => Some(LightChanged::slide),
            2 => Some(LightChanged::receivers),
            3 => Some(LightChanged::footprint),
            4 => Some(LightChanged::world),
            _ => None,
        }
    }

    /// The member's name, as its source spells it.
    pub const fn name(self) -> &'static str {
        match self {
            LightChanged::content => "content",
            LightChanged::slide => "slide",
            LightChanged::receivers => "receivers",
            LightChanged::footprint => "footprint",
            LightChanged::world => "world",
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
    none = 0,
    unready = 1,
    dropped = 2,
    plain = 3,
    still = 4,
    cast = 5,
    unchanged = 6,
    px = 7,
    masked = 8,
}

impl FrameKind {
    /// Every member, in its source's own order.
    pub const ALL: [FrameKind; 9] = [
        FrameKind::none,
        FrameKind::unready,
        FrameKind::dropped,
        FrameKind::plain,
        FrameKind::still,
        FrameKind::cast,
        FrameKind::unchanged,
        FrameKind::px,
        FrameKind::masked,
    ];

    /// The member `val` is, or none for a number no member has.
    pub const fn of(val: u8) -> Option<FrameKind> {
        match val {
            0 => Some(FrameKind::none),
            1 => Some(FrameKind::unready),
            2 => Some(FrameKind::dropped),
            3 => Some(FrameKind::plain),
            4 => Some(FrameKind::still),
            5 => Some(FrameKind::cast),
            6 => Some(FrameKind::unchanged),
            7 => Some(FrameKind::px),
            8 => Some(FrameKind::masked),
            _ => None,
        }
    }

    /// The member's name, as its source spells it.
    pub const fn name(self) -> &'static str {
        match self {
            FrameKind::none => "none",
            FrameKind::unready => "unready",
            FrameKind::dropped => "dropped",
            FrameKind::plain => "plain",
            FrameKind::still => "still",
            FrameKind::cast => "cast",
            FrameKind::unchanged => "unchanged",
            FrameKind::px => "px",
            FrameKind::masked => "masked",
        }
    }
}
