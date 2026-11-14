// xpute-native-runtime/app.test.rs

//! What the native runtime's process tests share. Each is a binary of its
//! own, since a process starts one runtime and links one guest. A native
//! guest's offsets name its one region, so the process's heap is a region
//! too, set as the guest's memory (xpute-link `mem::set`) at the first
//! allocation, as a program's guest sets its own; it is never freed, a test
//! being short. And the test is the app: it starts the runtime and keeps what
//! the runtime tells it.

use std::alloc::{GlobalAlloc, Layout, System};
use std::path::PathBuf;
use std::sync::atomic::{AtomicUsize, Ordering};
use std::sync::{Mutex, Once};
use std::time::{Duration, Instant};

use xpute_kit::wire::xtp::cursor::TreeReader;
use xpute_link::abi::types::Token;
use xpute_link::mem::{self, Region};
use xpute_link::{store_uri, Handle, Lower};
use xpute_native_runtime::native::{self, Start};

const HEAP_SZ: usize = 1 << 28;

struct Heap;

static BASE: AtomicUsize = AtomicUsize::new(0);
static END: AtomicUsize = AtomicUsize::new(0);

// SAFETY: each block lies in the region, apart from every other, and none is
// handed out twice.
unsafe impl GlobalAlloc for Heap {
    unsafe fn alloc(&self, l: Layout) -> *mut u8 {
        static SET: Once = Once::new();
        SET.call_once(|| {
            // SAFETY: a layout of nonzero size.
            let base = unsafe { System.alloc_zeroed(Layout::from_size_align(HEAP_SZ, 4096).unwrap()) };
            mem::set(Region { base, len: HEAP_SZ as u32 });
            BASE.store(base.expose_provenance(), Ordering::Relaxed);
        });
        let base = BASE.load(Ordering::Relaxed);
        let mut end = END.load(Ordering::Relaxed);
        loop {
            let at = (base + end).next_multiple_of(l.align()) - base;
            if at + l.size() > HEAP_SZ {
                return core::ptr::null_mut();
            }
            match END.compare_exchange_weak(end, at + l.size(), Ordering::Relaxed, Ordering::Relaxed) {
                Ok(_) => return core::ptr::with_exposed_provenance_mut(base + at),
                Err(now) => end = now,
            }
        }
    }

    unsafe fn dealloc(&self, _: *mut u8, _: Layout) {}
}

#[global_allocator]
static HEAP: Heap = Heap;

/// What the runtime tells the app of the guest's end, in the order told.
#[derive(Debug, PartialEq)]
pub enum End {
    /// A stop: its errno, and its file.
    Stop(i32, String),
    /// A trap: what stopped the guest.
    Trap(String),
}

static ENDS: Mutex<Vec<End>> = Mutex::new(Vec::new());

/// The runtime's UTF-8, `len` bytes at `at`.
fn text(at: *const u8, len: usize) -> String {
    // SAFETY: the runtime's, `len` bytes.
    String::from_utf8_lossy(unsafe { core::slice::from_raw_parts(at, len) }).into_owned()
}

extern "C" fn stopped(errnum: i32, file: *const u8, len: usize, _: u32, _: u64, _: u64) {
    ENDS.lock().unwrap().push(End::Stop(errnum, text(file, len)));
}

extern "C" fn trapped(err: *const u8, len: usize) {
    ENDS.lock().unwrap().push(End::Trap(text(err, len)));
}

pub fn ends() -> Vec<End> {
    core::mem::take(&mut *ENDS.lock().unwrap())
}

/// Starts a UI on the runtime, as the app's module does (`xpute_start`):
/// what it serves, its named interfaces and none opened by a prefix. It
/// answers the UI's generation.
pub fn start(start: Start, named: &[Handle]) -> u32 {
    native::stopped_on(Some(stopped));
    native::trap_on(Some(trapped));
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

/// A message the runtime sent a UI, its leading numbers: its kind, then what
/// that kind carries (remote.rs).
pub type Message = Vec<i64>;

/// The messages of the batch the UI of `generation` takes now.
pub fn take(generation: u32) -> Vec<Message> {
    let batch = xpute_native_runtime::native::take(generation);
    // SAFETY: what `take` handed over, freed once.
    let bytes = unsafe { core::slice::from_raw_parts(batch.0, batch.1) }.to_vec();
    unsafe { xpute_native_runtime::native::taken_free(batch.0, batch.1) };
    let mut out = Vec::new();
    let mut at = 0;
    while at < bytes.len() {
        let len = u32::from_le_bytes(bytes[at..at + 4].try_into().unwrap()) as usize;
        let f = TreeReader::new(&bytes[at + 8..at + 8 + len]).unwrap().read_branch().unwrap();
        out.push((0..).map_while(|k| f.at(k).ok()?.get_number().ok()).map(|n| n as i64).collect());
        at += (8 + len).next_multiple_of(8);
    }
    out
}

/// The UI of `generation` sends `m` of `h` with the packet `pkt`, its
/// request `tag`.
pub fn post(generation: u32, tag: u32, h: Handle, m: u32, pkt: Option<Vec<u8>>) {
    xpute_native_runtime::post(generation, tag, h, m, pkt);
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

/// Waits until the UI of `generation` hears an answer to its request `tag`,
/// gathering every message into `messages`, and answers its result.
pub fn answer_to(generation: u32, tag: u32, messages: &mut Vec<Message>) -> i64 {
    let mut result = None;
    until(&format!("the answer to {tag}"), || {
        let more = take(generation);
        messages.extend(more);
        result = messages.iter().find(|m| m[0] == 0 && m[1] == i64::from(tag)).map(|m| m[2]);
        result.is_some()
    });
    result.unwrap()
}
