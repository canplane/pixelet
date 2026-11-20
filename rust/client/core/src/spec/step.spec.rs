// pxl-core/spec/step.spec.rs
//
// GENERATED from spec/client/core/perf/step.json5 (xpute-gen data.rs)
//
// The frame's vocabulary, the core's and the UI's: the steps a tick runs, as frame.rs
// registers them, and the phase each runs in, the effects a frame runs before them, the parts of the stage it runs after them and the requests' pump, and the reasons a frame is asked for. The profiler records and
// reports them by these names (spec/client/core/perf/perf.json5).

use crate::spec::phase::Phase;

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
#[repr(u8)]
pub enum StepName {
    Input = 0,
    Camera = 1,
    Edit = 2,
    Ghost = 3,
    Location = 4,
    Environment = 5,
    Companion = 6,
    Demand = 7,
    Scene = 8,
}

impl StepName {
    /// Every member, in its source's own order.
    pub const ALL: [StepName; 9] = [
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
            0 => Some(StepName::Input),
            1 => Some(StepName::Camera),
            2 => Some(StepName::Edit),
            3 => Some(StepName::Ghost),
            4 => Some(StepName::Location),
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

/// The phase each step runs in, in STEP_NAME's order (pxl-core frame.rs `tick`).
pub const STEP_PHASE: [Phase; 9] = [
    Phase::Interaction,
    Phase::Interaction,
    Phase::Interaction,
    Phase::Interaction,
    Phase::Interaction,
    Phase::Interaction,
    Phase::Interaction,
    Phase::Visible,
    Phase::Visible,
];

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
#[repr(u8)]
pub enum EffectName {
    Navigation = 0,
    Decor = 1,
    Object = 2,
    Setting = 3,
}

impl EffectName {
    /// Every member, in its source's own order.
    pub const ALL: [EffectName; 4] = [
        EffectName::Navigation,
        EffectName::Decor,
        EffectName::Object,
        EffectName::Setting,
    ];

    /// The member `val` is, or none for a number no member has.
    pub const fn of(val: u8) -> Option<EffectName> {
        match val {
            0 => Some(EffectName::Navigation),
            1 => Some(EffectName::Decor),
            2 => Some(EffectName::Object),
            3 => Some(EffectName::Setting),
            _ => None,
        }
    }

    /// The member's name, as its source spells it.
    pub const fn name(self) -> &'static str {
        match self {
            EffectName::Navigation => "navigation",
            EffectName::Decor => "decor",
            EffectName::Object => "object",
            EffectName::Setting => "setting",
        }
    }
}

/// The parts of a frame's stage, in the order they start (pxl-core domain/scene/stage.rs `stage_render`): `assemble` what the renderer's frame is made from, the eye's cast among it; `upload` the ground's bricks and the imagery's tiles handed to the renderer, which asks for them as it encodes; `encode` the renderer's frame, less those uploads; `judge` what the frame leaves, the images it took given back and the drawn resolution judged.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
#[repr(u8)]
pub enum StagePartName {
    Assemble = 0,
    Upload = 1,
    Encode = 2,
    Judge = 3,
}

impl StagePartName {
    /// Every member, in its source's own order.
    pub const ALL: [StagePartName; 4] = [
        StagePartName::Assemble,
        StagePartName::Upload,
        StagePartName::Encode,
        StagePartName::Judge,
    ];

    /// The member `val` is, or none for a number no member has.
    pub const fn of(val: u8) -> Option<StagePartName> {
        match val {
            0 => Some(StagePartName::Assemble),
            1 => Some(StagePartName::Upload),
            2 => Some(StagePartName::Encode),
            3 => Some(StagePartName::Judge),
            _ => None,
        }
    }

    /// The member's name, as its source spells it.
    pub const fn name(self) -> &'static str {
        match self {
            StagePartName::Assemble => "assemble",
            StagePartName::Upload => "upload",
            StagePartName::Encode => "encode",
            StagePartName::Judge => "judge",
        }
    }
}

/// Why a frame is asked for (pxl-core sys/sched.rs `invalidate`): `method` a method from outside applied (the UI's, a surface's among them), `view` the camera moved or the ratio the world is drawn at changed, `scene` what is shown changed, `completion` something outside the frame finished and a frame takes it (a read, a parse, a write, the GPU's), `pending` a frame stopped short of its work or put it off and a later one goes on, `animation` an animation's step or a frame of its motion.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
#[repr(u8)]
pub enum InvalidationReason {
    Method = 0,
    View = 1,
    Scene = 2,
    Completion = 3,
    Pending = 4,
    Animation = 5,
}

impl InvalidationReason {
    /// Every member, in its source's own order.
    pub const ALL: [InvalidationReason; 6] = [
        InvalidationReason::Method,
        InvalidationReason::View,
        InvalidationReason::Scene,
        InvalidationReason::Completion,
        InvalidationReason::Pending,
        InvalidationReason::Animation,
    ];

    /// The member `val` is, or none for a number no member has.
    pub const fn of(val: u8) -> Option<InvalidationReason> {
        match val {
            0 => Some(InvalidationReason::Method),
            1 => Some(InvalidationReason::View),
            2 => Some(InvalidationReason::Scene),
            3 => Some(InvalidationReason::Completion),
            4 => Some(InvalidationReason::Pending),
            5 => Some(InvalidationReason::Animation),
            _ => None,
        }
    }

    /// The member's name, as its source spells it.
    pub const fn name(self) -> &'static str {
        match self {
            InvalidationReason::Method => "method",
            InvalidationReason::View => "view",
            InvalidationReason::Scene => "scene",
            InvalidationReason::Completion => "completion",
            InvalidationReason::Pending => "pending",
            InvalidationReason::Animation => "animation",
        }
    }
}
