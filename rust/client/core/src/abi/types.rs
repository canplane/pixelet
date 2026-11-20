// pxl-core/abi/types.rs
//
// GENERATED from spec/client/core/wit (xpute-link-gen wit.rs)

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
#[repr(u32)]
pub enum ErrorCode {
    /// The settings are not read yet, and a change would be overwritten.
    WouldBlock = 0,
    /// No surface is open there.
    BadDescriptor = 1,
    /// The surface draws something else.
    Busy = 2,
    /// The text is longer than its kind holds, a marker's or a chat line's.
    TooLong = 3,
    /// The profiler's setting is off.
    NotPermitted = 4,
    /// The profiler's window could not be had.
    InsufficientMemory = 5,
}

impl ErrorCode {
    /// Every member, in its source's own order.
    pub const ALL: [ErrorCode; 6] = [
        ErrorCode::WouldBlock,
        ErrorCode::BadDescriptor,
        ErrorCode::Busy,
        ErrorCode::TooLong,
        ErrorCode::NotPermitted,
        ErrorCode::InsufficientMemory,
    ];

    /// The member `val` is, or none for a number no member has.
    pub const fn of(val: u32) -> Option<ErrorCode> {
        match val {
            0 => Some(ErrorCode::WouldBlock),
            1 => Some(ErrorCode::BadDescriptor),
            2 => Some(ErrorCode::Busy),
            3 => Some(ErrorCode::TooLong),
            4 => Some(ErrorCode::NotPermitted),
            5 => Some(ErrorCode::InsufficientMemory),
            _ => None,
        }
    }

    /// The member's name, as its source spells it.
    pub const fn name(self) -> &'static str {
        match self {
            ErrorCode::WouldBlock => "WouldBlock",
            ErrorCode::BadDescriptor => "BadDescriptor",
            ErrorCode::Busy => "Busy",
            ErrorCode::TooLong => "TooLong",
            ErrorCode::NotPermitted => "NotPermitted",
            ErrorCode::InsufficientMemory => "InsufficientMemory",
        }
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
#[repr(u32)]
pub enum TrackingMode {
    None = 0,
    Follow = 1,
    FollowWithHeading = 2,
}

impl TrackingMode {
    /// Every member, in its source's own order.
    pub const ALL: [TrackingMode; 3] = [
        TrackingMode::None,
        TrackingMode::Follow,
        TrackingMode::FollowWithHeading,
    ];

    /// The member `val` is, or none for a number no member has.
    pub const fn of(val: u32) -> Option<TrackingMode> {
        match val {
            0 => Some(TrackingMode::None),
            1 => Some(TrackingMode::Follow),
            2 => Some(TrackingMode::FollowWithHeading),
            _ => None,
        }
    }

    /// The member's name, as its source spells it.
    pub const fn name(self) -> &'static str {
        match self {
            TrackingMode::None => "None",
            TrackingMode::Follow => "Follow",
            TrackingMode::FollowWithHeading => "FollowWithHeading",
        }
    }
}
