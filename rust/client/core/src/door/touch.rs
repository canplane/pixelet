// pxl-core/door/touch.rs

//! A device's touches on the world, taken on the platform's main thread and
//! posted to the runtime's thread with no JavaScript between. It touches no
//! state of the core's: it only writes the frame a page's UI would have sent.

use crate::door::cmd::{Command, InputPointer, PointerPhase, PointerType};
use xpute_kit::wire::xtp::PacketWriter;
#[cfg(target_os = "android")]
use xpute_runtime::jni::sys::{jclass, jdouble, jint, JNIEnv};
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

/// A touch on the world view, from the platform's main thread: 0 down, 1 move,
/// 2 up, 3 out and 4 cancel, at x, y in the view's own units.
#[no_mangle]
pub extern "C" fn pxl_touch(phase: u32, id: i32, x: f64, y: f64) {
    touch(phase, id, x, y);
}

#[cfg(target_os = "android")]
#[no_mangle]
extern "system" fn Java_me_plei_pixelet_platform_PxlNative_touch(
    _env: *mut JNIEnv,
    _class: jclass,
    phase: jint,
    id: jint,
    x: jdouble,
    y: jdouble,
) {
    touch(phase as u32, id, x, y);
}
