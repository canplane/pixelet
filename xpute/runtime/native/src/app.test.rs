// xpute-runtime-native/app.test.rs

//! What the native runtime's process tests share. Each is a binary of its
//! own, since a process starts one runtime and links one guest. The test is
//! the program, its guest's memory the process's. And the test is the app:
//! it starts the runtime and keeps what the runtime tells it.

use std::path::PathBuf;
use std::sync::Mutex;
use std::time::{Duration, Instant};

use xpute_link::abi::types::Token;
use xpute_link::frame::{Frame, HDR_SZ};
use xpute_link::{Handle, Lower, store_uri};
use xpute_runtime_native::listener;
use xpute_runtime_native::native::{self, Start};

/// What the runtime tells the app of the guest's end, in the order told.
#[derive(Debug, PartialEq)]
pub enum Exit {
    /// A stop: its errno, and its file.
    Stop(i32, String),
    /// A trap: what stopped the guest.
    Trap(String),
}

static EXITS: Mutex<Vec<Exit>> = Mutex::new(Vec::new());

/// The runtime's UTF-8, `len` bytes at `at`.
fn text(at: *const u8, len: usize) -> String {
    // SAFETY: the runtime's, `len` bytes.
    String::from_utf8_lossy(unsafe { core::slice::from_raw_parts(at, len) }).into_owned()
}

extern "C" fn stopped(errnum: i32, file: *const u8, len: usize, _: u32, _: u64, _: u64) {
    EXITS
        .lock()
        .unwrap()
        .push(Exit::Stop(errnum, text(file, len)));
}

extern "C" fn trapped(err: *const u8, len: usize) {
    EXITS.lock().unwrap().push(Exit::Trap(text(err, len)));
}

pub fn exits() -> Vec<Exit> {
    core::mem::take(&mut *EXITS.lock().unwrap())
}

/// Starts a UI on the runtime, as the app's module does (`xpute_start`):
/// what it implements, its named interfaces and none opened by a prefix. It
/// answers the UI's handle.
pub fn start(start: Start, named: &[Handle]) -> u32 {
    listener::stopped_on(Some(stopped));
    listener::trap_on(Some(trapped));
    let mut prefixes = [0; 8];
    let mut w = Lower::new(&mut prefixes, 8);
    w.list(0, 0, 12, 4);
    w.finish().unwrap();
    native::start(start, named.to_vec(), &prefixes, PathBuf::new())
}

/// Waits until `cond` holds, which it must within seconds.
pub fn until(what: &str, mut cond: impl FnMut() -> bool) {
    let deadline = Instant::now() + Duration::from_secs(10);
    while !cond() {
        assert!(Instant::now() < deadline, "never: {what}");
        std::thread::sleep(Duration::from_millis(1));
    }
}

/// The frames of the batch the UI of the handle `ui` takes now, each without
/// its packet (remote.rs).
pub fn take(ui: u32) -> Vec<Frame> {
    let batch = native::take(ui);
    // SAFETY: what `take` handed over, read whole and then freed once.
    let bytes = unsafe { core::slice::from_raw_parts(batch.0, batch.1) }.to_vec();
    // SAFETY: as above.
    unsafe { native::taken_free(batch.0, batch.1) };
    let mut out = Vec::new();
    let mut at = 0;
    while at < bytes.len() {
        let word = |k: usize| {
            let w = at + 4 * k;
            u32::from_le_bytes(bytes[w..w + 4].try_into().unwrap())
        };
        let f = Frame::of(core::array::from_fn(word));
        at += (HDR_SZ + f.pkt_len).next_multiple_of(8) as usize;
        out.push(f);
    }
    out
}

/// The UI of the handle `ui` sends `m` of `h` with the packet `pkt`, its
/// request `tag`.
pub fn post(ui: u32, tag: u32, h: Handle, m: u32, pkt: Option<Vec<u8>>) {
    xpute_runtime_native::post(ui, tag, h, m, pkt);
}

/// `uri` as a lowered `list<token>`, the args of the root's open.
pub fn lowered(uri: &[Token]) -> Vec<u8> {
    let mut buf = vec![0; 256];
    let mut w = Lower::new(&mut buf, 8);
    store_uri(&mut w, 0, uri);
    let len = w.finish().unwrap();
    buf.truncate(len);
    buf
}

/// Waits until the UI of the handle `ui` hears an answer to its request `tag`,
/// gathering every frame into `frames`, and answers its result.
pub fn answer_to(ui: u32, tag: u32, frames: &mut Vec<Frame>) -> i64 {
    let mut result = None;
    until(&format!("the answer to {tag}"), || {
        let more = take(ui);
        frames.extend(more);
        result = frames
            .iter()
            .find(|m| m.res && m.tag == tag)
            .map(|m| i64::from(m.result));
        result.is_some()
    });
    result.unwrap()
}
