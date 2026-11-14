// pxl-core/abi/types.rs
//
// GENERATED from spec/client/core/wit (xpute-link-gen wit.rs)

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
#[repr(u32)]
pub enum Sensor {
    Geolocation = 0,
    Orient = 1,
}

impl Sensor {
    /// Every member, in its source's own order.
    pub const ALL: [Sensor; 2] = [Sensor::Geolocation, Sensor::Orient];

    /// The member `val` is, or none for a number no member has.
    pub const fn of(val: u32) -> Option<Sensor> {
        match val {
            0 => Some(Sensor::Geolocation),
            1 => Some(Sensor::Orient),
            _ => None,
        }
    }

    /// The member's name, as its source spells it.
    pub const fn name(self) -> &'static str {
        match self {
            Sensor::Geolocation => "Geolocation",
            Sensor::Orient => "Orient",
        }
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
#[repr(u32)]
pub enum ErrorCode {
    /// The settings are not read yet, and a change would be overwritten.
    WouldBlock = 0,
    /// No surface is open there.
    BadDescriptor = 1,
    /// The surface draws something else.
    Busy = 2,
    /// The text is longer than a marker holds.
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

#[derive(Clone, Debug, PartialEq)]
pub struct Shard {
    pub name: String,
    pub versions: Vec<Option<String>>,
}

impl Shard {
    pub const SZ: usize = 16;
    pub const ALIGN: usize = 4;

    pub fn store(&self, w: &mut _rt::Lower, at: usize) {
        w.str(at, &self.name);
        if let Some(b) = w.list(at + 8, self.versions.len(), 12, 4) {
            for (i, x) in self.versions.iter().enumerate() {
                if let Some(v) = x {
                    w.u8(b + i * 12, 1);
                    w.str(b + i * 12 + 4, v);
                }
            }
        }
    }

    pub fn load(r: _rt::Lift<'_>, at: usize) -> Result<Shard, _rt::Errno> {
        Ok(Shard {
            name: r.str(at).map(str::to_owned)?,
            versions: r.list(at + 8, 12, 4).and_then(|(b, n)| {
                (0..n)
                    .map(|i| {
                        r.case(b + i * 12, 2).and_then(|d| {
                            if d == 0 {
                                Ok(None)
                            } else {
                                r.str(b + i * 12 + 4).map(str::to_owned).map(Some)
                            }
                        })
                    })
                    .collect::<Result<Vec<_>, _rt::Errno>>()
            })?,
        })
    }
}

#[derive(Clone, Debug, PartialEq)]
pub struct Declaration {
    pub min_zoom: u32,
    pub formats: Vec<u32>,
    pub versions: Vec<String>,
    pub shards: Vec<Shard>,
}

impl Declaration {
    pub const SZ: usize = 28;
    pub const ALIGN: usize = 4;

    pub fn store(&self, w: &mut _rt::Lower, at: usize) {
        w.u32(at, self.min_zoom);
        w.arr(at + 4, &self.formats);
        if let Some(b) = w.list(at + 12, self.versions.len(), 8, 4) {
            for (i, x) in self.versions.iter().enumerate() {
                w.str(b + i * 8, x);
            }
        }
        if let Some(b) = w.list(at + 20, self.shards.len(), Shard::SZ, Shard::ALIGN) {
            for (i, x) in self.shards.iter().enumerate() {
                (x).store(w, b + i * Shard::SZ);
            }
        }
    }

    pub fn load(r: _rt::Lift<'_>, at: usize) -> Result<Declaration, _rt::Errno> {
        Ok(Declaration {
            min_zoom: r.u32(at)?,
            formats: r.arr::<u32>(at + 4)?,
            versions: r.list(at + 12, 8, 4).and_then(|(b, n)| {
                (0..n)
                    .map(|i| r.str(b + i * 8).map(str::to_owned))
                    .collect::<Result<Vec<_>, _rt::Errno>>()
            })?,
            shards: r
                .list(at + 20, Shard::SZ, Shard::ALIGN)
                .and_then(|(b, n)| {
                    (0..n)
                        .map(|i| Shard::load(r, b + i * Shard::SZ))
                        .collect::<Result<Vec<_>, _rt::Errno>>()
                })?,
        })
    }
}

mod _rt {
    pub(super) use xpute_kit::status::errno::Errno;
    pub(super) use xpute_link::{Lift, Lower};
}
