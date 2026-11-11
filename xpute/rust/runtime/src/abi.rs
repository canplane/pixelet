// xpute-runtime/abi.rs

//! The runtime end of xpute's own protocols, generated as a program's is: a
//! writer for each export and a reader for each import (`sys`, `fs`).

use xpute_kit::status::error::MarshalError;
use xpute_kit::wire::xtp::PacketWriter;

#[path = "abi/fs.spec.rs"]
pub mod fs;
#[path = "abi/sys.spec.rs"]
pub mod sys;

/// What the runtime answers a command the core sends: a value to complete it
/// with now, or none where nothing is answered now.
pub type Outcome = Result<Option<i32>, MarshalError>;

/// What a runtime sends the core, as each export's generated writer gives it:
/// its number, the children its packet has, and its packet's writer. A
/// protocol whose commands are minors gives its minor, and the major is the
/// program's. Whether a frame asks for an answer (ACKREQ) is its sender's.
pub trait Export {
    const NUMBER: u32;
    const FIELDS: u32;
    fn write(&self, w: &mut PacketWriter);
}
