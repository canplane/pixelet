// pxl-core/sys/io/demand.spec.rs
//
// GENERATED from spec/client/core/sys/io/demand.json (xpute-gen data.rs)
//
// A request's vocabulary, the core's and the UI's: what stream it is in, how
// its run came to an end, and the phases a run reports its time in. The profiler
// records and reports them by these names (spec/client/core/perf/perf.json).

// Names as the spec spells them, so one record has one name on every side.
// A table is generated whole, whether or not this crate calls an entry.
#![allow(
    non_upper_case_globals,
    non_snake_case,
    non_camel_case_types,
    dead_code,
    reason = "generated from the spec"
)]

pub const STREAM_NAMES: &[&str] = &["terrain", "place", "division", "asset", "earth", "object"];
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
#[repr(u8)]
pub enum StreamName {
    terrain = 0,
    place = 1,
    division = 2,
    asset = 3,
    earth = 4,
    object = 5,
}

impl StreamName {
    /// Every member, in its source's own order.
    pub const ALL: [StreamName; 6] = [
        StreamName::terrain,
        StreamName::place,
        StreamName::division,
        StreamName::asset,
        StreamName::earth,
        StreamName::object,
    ];

    /// The member `val` is, or none for a number no member has.
    pub const fn of(val: u8) -> Option<StreamName> {
        match val {
            0 => Some(StreamName::terrain),
            1 => Some(StreamName::place),
            2 => Some(StreamName::division),
            3 => Some(StreamName::asset),
            4 => Some(StreamName::earth),
            5 => Some(StreamName::object),
            _ => None,
        }
    }

    /// The member's name, as its source spells it.
    pub const fn name(self) -> &'static str {
        match self {
            StreamName::terrain => "terrain",
            StreamName::place => "place",
            StreamName::division => "division",
            StreamName::asset => "asset",
            StreamName::earth => "earth",
            StreamName::object => "object",
        }
    }
}

pub const OUTCOME_NAMES: &[&str] = &["ok", "aborted", "err"];
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
#[repr(u8)]
pub enum OutcomeName {
    ok = 0,
    aborted = 1,
    err = 2,
}

impl OutcomeName {
    /// Every member, in its source's own order.
    pub const ALL: [OutcomeName; 3] = [OutcomeName::ok, OutcomeName::aborted, OutcomeName::err];

    /// The member `val` is, or none for a number no member has.
    pub const fn of(val: u8) -> Option<OutcomeName> {
        match val {
            0 => Some(OutcomeName::ok),
            1 => Some(OutcomeName::aborted),
            2 => Some(OutcomeName::err),
            _ => None,
        }
    }

    /// The member's name, as its source spells it.
    pub const fn name(self) -> &'static str {
        match self {
            OutcomeName::ok => "ok",
            OutcomeName::aborted => "aborted",
            OutcomeName::err => "err",
        }
    }
}

pub const TASK_PHASE_NAMES: &[&str] = &["net_ms", "bytes", "decode_ms"];
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
#[repr(u8)]
pub enum TaskPhaseName {
    net_ms = 0,
    bytes = 1,
    decode_ms = 2,
}

impl TaskPhaseName {
    /// Every member, in its source's own order.
    pub const ALL: [TaskPhaseName; 3] = [
        TaskPhaseName::net_ms,
        TaskPhaseName::bytes,
        TaskPhaseName::decode_ms,
    ];

    /// The member `val` is, or none for a number no member has.
    pub const fn of(val: u8) -> Option<TaskPhaseName> {
        match val {
            0 => Some(TaskPhaseName::net_ms),
            1 => Some(TaskPhaseName::bytes),
            2 => Some(TaskPhaseName::decode_ms),
            _ => None,
        }
    }

    /// The member's name, as its source spells it.
    pub const fn name(self) -> &'static str {
        match self {
            TaskPhaseName::net_ms => "net_ms",
            TaskPhaseName::bytes => "bytes",
            TaskPhaseName::decode_ms => "decode_ms",
        }
    }
}

/// A run's timings past this many are dropped.
pub const TASK_PHASES_MAX: u32 = 6;
