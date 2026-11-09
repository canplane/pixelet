// pxl-core/door/touch.rs

//! A device's touches on the world, taken on the platform's main thread and
//! posted to the runtime's thread with no JavaScript between. It touches no
//! state of the core's: it only writes the frame a page's UI would have sent.

use crate::door::cmd::{Command, InputPointer, PointerPhase, PointerType};
use xpute_kit::wire::xtp::PacketWriter;
use xpute_runtime::native;

/// More than an INPUT_POINTER's packet.
const PACKET_BYTES: usize = 128;

fn touch(phase: u32, id: i32, x: f64, y: f64) {
    let Some(phase) = PointerPhase::of(phase) else {
        return;
    };
    let mut buf = [0u8; PACKET_BYTES];
    let mut w = PacketWriter::new(&mut buf, InputPointer::FIELDS);
    InputPointer {
        phase,
        pointer_type: PointerType::Touch,
        id,
        button: 0,
        x,
        y,
    }
    .write(&mut w);
    let Some(len) = w.fit_len() else {
        return;
    };
    native::input();
    native::post(0, Command::INPUT_POINTER as u32, 0, 0, &buf[..len as usize]);
}

pub(crate) fn install() {
    native::touch_by(touch);
}
