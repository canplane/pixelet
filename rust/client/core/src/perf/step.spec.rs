// pxl-core/perf/step.spec.rs
//
// GENERATED from spec/client/core/perf/step.json (xpute-gen data.rs)
//
// The frame's vocabulary, the core's and the UI's: the steps a tick runs, as frame.rs
// registers them, and the effects a frame runs around them. The profiler records and
// reports them by these names (spec/client/core/perf/perf.json).

// A table is generated whole, whether or not this crate calls an entry.
#![allow(dead_code, reason = "generated from the spec")]

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
    Controller = 0,
    Input = 1,
    Camera = 2,
    Edit = 3,
    Ghost = 4,
    Environment = 5,
    Companion = 6,
    Demand = 7,
    Scene = 8,
}

impl StepName {
    /// Every member, in its source's own order.
    pub const ALL: [StepName; 9] = [
        StepName::Controller,
        StepName::Input,
        StepName::Camera,
        StepName::Edit,
        StepName::Ghost,
        StepName::Environment,
        StepName::Companion,
        StepName::Demand,
        StepName::Scene,
    ];

    /// The member `val` is, or none for a number no member has.
    pub const fn of(val: u8) -> Option<StepName> {
        match val {
            0 => Some(StepName::Controller),
            1 => Some(StepName::Input),
            2 => Some(StepName::Camera),
            3 => Some(StepName::Edit),
            4 => Some(StepName::Ghost),
            5 => Some(StepName::Environment),
            6 => Some(StepName::Companion),
            7 => Some(StepName::Demand),
            8 => Some(StepName::Scene),
            _ => None,
        }
    }

    /// The member's name, as its source spells it.
    pub const fn name(self) -> &'static str {
        match self {
            StepName::Controller => "controller",
            StepName::Input => "input",
            StepName::Camera => "camera",
            StepName::Edit => "edit",
            StepName::Ghost => "ghost",
            StepName::Environment => "environment",
            StepName::Companion => "companion",
            StepName::Demand => "demand",
            StepName::Scene => "scene",
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
    "object",
    "location",
    "setting",
];
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
#[repr(u8)]
pub enum EffectName {
    Navigation = 0,
    Bootstrap = 1,
    Orient = 2,
    Follow = 3,
    Dst = 4,
    Decor = 5,
    Object = 6,
    Location = 7,
    Setting = 8,
}

impl EffectName {
    /// Every member, in its source's own order.
    pub const ALL: [EffectName; 9] = [
        EffectName::Navigation,
        EffectName::Bootstrap,
        EffectName::Orient,
        EffectName::Follow,
        EffectName::Dst,
        EffectName::Decor,
        EffectName::Object,
        EffectName::Location,
        EffectName::Setting,
    ];

    /// The member `val` is, or none for a number no member has.
    pub const fn of(val: u8) -> Option<EffectName> {
        match val {
            0 => Some(EffectName::Navigation),
            1 => Some(EffectName::Bootstrap),
            2 => Some(EffectName::Orient),
            3 => Some(EffectName::Follow),
            4 => Some(EffectName::Dst),
            5 => Some(EffectName::Decor),
            6 => Some(EffectName::Object),
            7 => Some(EffectName::Location),
            8 => Some(EffectName::Setting),
            _ => None,
        }
    }

    /// The member's name, as its source spells it.
    pub const fn name(self) -> &'static str {
        match self {
            EffectName::Navigation => "navigation",
            EffectName::Bootstrap => "bootstrap",
            EffectName::Orient => "orient",
            EffectName::Follow => "follow",
            EffectName::Dst => "dst",
            EffectName::Decor => "decor",
            EffectName::Object => "object",
            EffectName::Location => "location",
            EffectName::Setting => "setting",
        }
    }
}
