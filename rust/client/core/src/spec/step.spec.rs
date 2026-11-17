// pxl-core/spec/step.spec.rs
//
// GENERATED from spec/client/core/perf/step.json5 (xpute-gen data.rs)
//
// The frame's vocabulary, the core's and the UI's: the steps a tick runs, as frame.rs
// registers them, and the effects a frame runs around them. The profiler records and
// reports them by these names (spec/client/core/perf/perf.json5).

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
#[repr(u8)]
pub enum StepName {
    Controller = 0,
    Input = 1,
    Camera = 2,
    Edit = 3,
    Ghost = 4,
    Location = 5,
    Environment = 6,
    Companion = 7,
    Demand = 8,
    Scene = 9,
}

impl StepName {
    /// Every member, in its source's own order.
    pub const ALL: [StepName; 10] = [
        StepName::Controller,
        StepName::Input,
        StepName::Camera,
        StepName::Edit,
        StepName::Ghost,
        StepName::Location,
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
            5 => Some(StepName::Location),
            6 => Some(StepName::Environment),
            7 => Some(StepName::Companion),
            8 => Some(StepName::Demand),
            9 => Some(StepName::Scene),
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
            StepName::Location => "location",
            StepName::Environment => "environment",
            StepName::Companion => "companion",
            StepName::Demand => "demand",
            StepName::Scene => "scene",
        }
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
#[repr(u8)]
pub enum EffectName {
    Navigation = 0,
    Bootstrap = 1,
    Follow = 2,
    Dst = 3,
    Decor = 4,
    Object = 5,
    Setting = 6,
}

impl EffectName {
    /// Every member, in its source's own order.
    pub const ALL: [EffectName; 7] = [
        EffectName::Navigation,
        EffectName::Bootstrap,
        EffectName::Follow,
        EffectName::Dst,
        EffectName::Decor,
        EffectName::Object,
        EffectName::Setting,
    ];

    /// The member `val` is, or none for a number no member has.
    pub const fn of(val: u8) -> Option<EffectName> {
        match val {
            0 => Some(EffectName::Navigation),
            1 => Some(EffectName::Bootstrap),
            2 => Some(EffectName::Follow),
            3 => Some(EffectName::Dst),
            4 => Some(EffectName::Decor),
            5 => Some(EffectName::Object),
            6 => Some(EffectName::Setting),
            _ => None,
        }
    }

    /// The member's name, as its source spells it.
    pub const fn name(self) -> &'static str {
        match self {
            EffectName::Navigation => "navigation",
            EffectName::Bootstrap => "bootstrap",
            EffectName::Follow => "follow",
            EffectName::Dst => "dst",
            EffectName::Decor => "decor",
            EffectName::Object => "object",
            EffectName::Setting => "setting",
        }
    }
}
