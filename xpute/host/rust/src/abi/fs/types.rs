// xpute-host/abi/fs/types.rs
//
// GENERATED from wit/fs (xpute-link-gen wit.rs)

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
#[repr(u32)]
pub enum ErrorCode {
    /// No file is there.
    NoEntry = 0,
    /// Its backend failed to read or write it.
    Io = 1,
    /// Its backend keeps nothing written there.
    ReadOnly = 2,
    /// Its backend refused access to it.
    Access = 3,
    /// Its backend holds it in a form it cannot read as bytes.
    BadMessage = 4,
    /// The network did not reach its server.
    RemoteUnreachable = 5,
    /// Its server holds no such object.
    NoObject = 6,
}

impl ErrorCode {
    /// Every member, in its source's own order.
    pub const ALL: [ErrorCode; 7] = [
        ErrorCode::NoEntry,
        ErrorCode::Io,
        ErrorCode::ReadOnly,
        ErrorCode::Access,
        ErrorCode::BadMessage,
        ErrorCode::RemoteUnreachable,
        ErrorCode::NoObject,
    ];

    /// The member `val` is, or none for a number no member has.
    pub const fn of(val: u32) -> Option<ErrorCode> {
        match val {
            0 => Some(ErrorCode::NoEntry),
            1 => Some(ErrorCode::Io),
            2 => Some(ErrorCode::ReadOnly),
            3 => Some(ErrorCode::Access),
            4 => Some(ErrorCode::BadMessage),
            5 => Some(ErrorCode::RemoteUnreachable),
            6 => Some(ErrorCode::NoObject),
            _ => None,
        }
    }

    /// The member's name, as its source spells it.
    pub const fn name(self) -> &'static str {
        match self {
            ErrorCode::NoEntry => "NoEntry",
            ErrorCode::Io => "Io",
            ErrorCode::ReadOnly => "ReadOnly",
            ErrorCode::Access => "Access",
            ErrorCode::BadMessage => "BadMessage",
            ErrorCode::RemoteUnreachable => "RemoteUnreachable",
            ErrorCode::NoObject => "NoObject",
        }
    }
}
