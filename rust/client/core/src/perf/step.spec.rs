// pxl-core/perf/step.spec.rs
//
// GENERATED from spec/client/core/perf/step.json (xpute-gen data.rs)
//
// The frame's vocabulary, the core's and the UI's: the steps a tick runs, as frame.rs
// registers them, and the effects a frame runs around them. The profiler records and
// reports them by these names (spec/client/core/perf/perf.json).

// Names as the spec spells them, so one record has one name on every side.
// A table is generated whole, whether or not this crate calls an entry.
#![allow(
    non_upper_case_globals,
    non_snake_case,
    non_camel_case_types,
    dead_code,
    reason = "generated from the spec"
)]

pub const STEP_NAMES: &[&str] = &[
    "controller",
    "input",
    "camera",
    "edit",
    "ghost",
    "environment",
    "companion",
    "demand",
    "scene",
];
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
#[repr(u8)]
pub enum StepName {
    controller = 0,
    input = 1,
    camera = 2,
    edit = 3,
    ghost = 4,
    environment = 5,
    companion = 6,
    demand = 7,
    scene = 8,
}

impl StepName {
    /// Every member, in its source's own order.
    pub const ALL: [StepName; 9] = [
        StepName::controller,
        StepName::input,
        StepName::camera,
        StepName::edit,
        StepName::ghost,
        StepName::environment,
        StepName::companion,
        StepName::demand,
        StepName::scene,
    ];

    /// The member `val` is, or none for a number no member has.
    pub const fn of(val: u8) -> Option<StepName> {
        match val {
            0 => Some(StepName::controller),
            1 => Some(StepName::input),
            2 => Some(StepName::camera),
            3 => Some(StepName::edit),
            4 => Some(StepName::ghost),
            5 => Some(StepName::environment),
            6 => Some(StepName::companion),
            7 => Some(StepName::demand),
            8 => Some(StepName::scene),
            _ => None,
        }
    }

    /// The member's name, as its source spells it.
    pub const fn name(self) -> &'static str {
        match self {
            StepName::controller => "controller",
            StepName::input => "input",
            StepName::camera => "camera",
            StepName::edit => "edit",
            StepName::ghost => "ghost",
            StepName::environment => "environment",
            StepName::companion => "companion",
            StepName::demand => "demand",
            StepName::scene => "scene",
        }
    }
}

pub const EFFECT_NAMES: &[&str] = &[
    "navigation",
    "bootstrap",
    "orient",
    "follow",
    "dst",
    "decor",
    "objects",
    "location",
    "settings",
];
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
#[repr(u8)]
pub enum EffectName {
    navigation = 0,
    bootstrap = 1,
    orient = 2,
    follow = 3,
    dst = 4,
    decor = 5,
    objects = 6,
    location = 7,
    settings = 8,
}

impl EffectName {
    /// Every member, in its source's own order.
    pub const ALL: [EffectName; 9] = [
        EffectName::navigation,
        EffectName::bootstrap,
        EffectName::orient,
        EffectName::follow,
        EffectName::dst,
        EffectName::decor,
        EffectName::objects,
        EffectName::location,
        EffectName::settings,
    ];

    /// The member `val` is, or none for a number no member has.
    pub const fn of(val: u8) -> Option<EffectName> {
        match val {
            0 => Some(EffectName::navigation),
            1 => Some(EffectName::bootstrap),
            2 => Some(EffectName::orient),
            3 => Some(EffectName::follow),
            4 => Some(EffectName::dst),
            5 => Some(EffectName::decor),
            6 => Some(EffectName::objects),
            7 => Some(EffectName::location),
            8 => Some(EffectName::settings),
            _ => None,
        }
    }

    /// The member's name, as its source spells it.
    pub const fn name(self) -> &'static str {
        match self {
            EffectName::navigation => "navigation",
            EffectName::bootstrap => "bootstrap",
            EffectName::orient => "orient",
            EffectName::follow => "follow",
            EffectName::dst => "dst",
            EffectName::decor => "decor",
            EffectName::objects => "objects",
            EffectName::location => "location",
            EffectName::settings => "settings",
        }
    }
}
