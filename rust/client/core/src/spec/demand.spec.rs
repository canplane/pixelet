// pxl-core/spec/demand.spec.rs
//
// GENERATED from spec/client/core/sys/io/demand.json5 (xpute-gen data.rs)
//
// A request's vocabulary, the core's and the UI's: what stream it is in, how
// its run came to an end, and the phases a run reports its time in. The profiler
// records and reports them by these names (spec/client/core/perf/perf.json5).

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
#[repr(u8)]
pub enum StreamName {
    Terrain = 0,
    Place = 1,
    Division = 2,
    Asset = 3,
    Imagery = 4,
    Object = 5,
}

impl StreamName {
    /// Every member, in its source's own order.
    pub const ALL: [StreamName; 6] = [
        StreamName::Terrain,
        StreamName::Place,
        StreamName::Division,
        StreamName::Asset,
        StreamName::Imagery,
        StreamName::Object,
    ];

    /// The member `val` is, or none for a number no member has.
    pub const fn of(val: u8) -> Option<StreamName> {
        match val {
            0 => Some(StreamName::Terrain),
            1 => Some(StreamName::Place),
            2 => Some(StreamName::Division),
            3 => Some(StreamName::Asset),
            4 => Some(StreamName::Imagery),
            5 => Some(StreamName::Object),
            _ => None,
        }
    }

    /// The member's name, as its source spells it.
    pub const fn name(self) -> &'static str {
        match self {
            StreamName::Terrain => "terrain",
            StreamName::Place => "place",
            StreamName::Division => "division",
            StreamName::Asset => "asset",
            StreamName::Imagery => "imagery",
            StreamName::Object => "object",
        }
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
#[repr(u8)]
pub enum OutcomeName {
    Ok = 0,
    Aborted = 1,
    Err = 2,
}

impl OutcomeName {
    /// Every member, in its source's own order.
    pub const ALL: [OutcomeName; 3] = [OutcomeName::Ok, OutcomeName::Aborted, OutcomeName::Err];

    /// The member `val` is, or none for a number no member has.
    pub const fn of(val: u8) -> Option<OutcomeName> {
        match val {
            0 => Some(OutcomeName::Ok),
            1 => Some(OutcomeName::Aborted),
            2 => Some(OutcomeName::Err),
            _ => None,
        }
    }

    /// The member's name, as its source spells it.
    pub const fn name(self) -> &'static str {
        match self {
            OutcomeName::Ok => "ok",
            OutcomeName::Aborted => "aborted",
            OutcomeName::Err => "err",
        }
    }
}

/// What a run reports of itself, each at most once.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
#[repr(u8)]
pub enum TaskPhaseName {
    NetMs = 0,
    Bytes = 1,
    DecodeMs = 2,
}

impl TaskPhaseName {
    /// Every member, in its source's own order.
    pub const ALL: [TaskPhaseName; 3] = [
        TaskPhaseName::NetMs,
        TaskPhaseName::Bytes,
        TaskPhaseName::DecodeMs,
    ];

    /// The member `val` is, or none for a number no member has.
    pub const fn of(val: u8) -> Option<TaskPhaseName> {
        match val {
            0 => Some(TaskPhaseName::NetMs),
            1 => Some(TaskPhaseName::Bytes),
            2 => Some(TaskPhaseName::DecodeMs),
            _ => None,
        }
    }

    /// The member's name, as its source spells it.
    pub const fn name(self) -> &'static str {
        match self {
            TaskPhaseName::NetMs => "net_ms",
            TaskPhaseName::Bytes => "bytes",
            TaskPhaseName::DecodeMs => "decode_ms",
        }
    }
}
