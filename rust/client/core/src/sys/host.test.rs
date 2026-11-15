// pxl-core/sys/host.test.rs
//
// The host the core's tests run against: the other side of the link, laid
// as the guest's init lays it on a host. It serves the monotonic clock's `now`
// at the time a test sets (`set_clock`) and what a test mounts (`mount`), and
// answers a request later as a test says (`answer`). A call of an interface
// nothing mounts answers no-entry, and of the clock's other methods ENOSYS.

use std::cell::Cell;
use std::rc::Rc;

use xpute_guest::abi::clocks::monotonic_clock::{self, NOW};
use xpute_guest::clock::duration;
use xpute_guest::link::{init, recv};
use xpute_kit::status::errno::Errno;
use xpute_link::frame::HDR_SZ;
use xpute_link::{Handle, Layout, Method, Object, Packet, Reply, Side, Tag};

/// A frame's cap: its words and a page, past every packet the tests lay.
const CAP: u32 = HDR_SZ + 4096;

thread_local! {
    static SIDE: Side = const { Side::new(|l| l.inbox, |_| recv()) };
    static LAID: Cell<bool> = const { Cell::new(false) };
    /// The clock's time, in ns.
    static NOW_NS: Cell<u64> = const { Cell::new(0) };
}

/// The guest's import, served by this side.
#[unsafe(no_mangle)]
extern "C" fn link_out(at: u32) -> i32 {
    SIDE.with(|s| s.recv(at))
}

/// The guest's memory, as this side's runtime answers it, on the runtime's
/// contract (xpute-native-runtime native.rs `mem`): the manifest's size and
/// `.fixed` the core's share, as pxl-runtime gives a device's, reserved by
/// the first call, inside the global allocator, zeroed and on a page, a
/// refusal stopping the process. The system is asked a page more at its own
/// alignment, where its zeroed memory is mapped as it is touched.
///
/// # Safety
/// `len` and `share` are each null or a writable word.
#[unsafe(no_mangle)]
unsafe extern "C" fn xpute_mem(len: *mut u32, share: *mut u32) -> *mut u8 {
    use crate::spec::manifest::mem;
    use std::alloc::{GlobalAlloc, Layout, System};
    const PAGE_SZ: usize = 4096;
    static BASE: std::sync::OnceLock<usize> = std::sync::OnceLock::new();
    let base = *BASE.get_or_init(|| {
        let Ok(layout) = Layout::from_size_align(mem::SZ as usize + PAGE_SZ, 16) else {
            std::process::abort();
        };
        // SAFETY: a layout of nonzero size.
        let at = unsafe { System.alloc_zeroed(layout) };
        if at.is_null() {
            std::process::abort();
        }
        at.expose_provenance().next_multiple_of(PAGE_SZ)
    });
    // SAFETY: the caller's words.
    unsafe {
        if !len.is_null() {
            *len = mem::SZ;
        }
        if !share.is_null() {
            *share = mem::FIXED_SZ;
        }
    }
    core::ptr::with_exposed_provenance_mut(base)
}

struct Clock;

impl Object for Clock {
    fn call(&self, m: Method, _: Tag, _: Option<&[u8]>) -> Reply {
        match m {
            NOW => {
                let ns = NOW_NS.get();
                Reply {
                    result: 0,
                    data: Some((8, Box::new(move |w| w.u64(0, ns)))),
                }
            }
            _ => Reply::from(Errno::ENOSYS as i32),
        }
    }
}

/// Lays the link, once on this thread.
fn lay() {
    if LAID.replace(true) {
        return;
    }
    let at = init(CAP, || {}, || false);
    SIDE.with(|s| {
        s.set_layout(Layout::at(at));
        s.mount_named(monotonic_clock::HANDLE, Rc::new(Clock));
    });
}

/// Sets the clock to `ms`.
pub(crate) fn set_clock(ms: f64) {
    lay();
    NOW_NS.set(duration(ms));
}

/// Serves `o` under the named handle `h`.
pub(crate) fn mount(h: Handle, o: Rc<dyn Object>) {
    lay();
    SIDE.with(|s| s.mount_named(h, o));
}

/// Answers the guest's request `t`, which its method answered EINPROGRESS,
/// with `result` and the return data `data`, in an entry of the guest's,
/// which runs what the answer made due.
pub(crate) fn answer(t: Tag, result: i32, data: &[u8]) {
    SIDE.with(|s| s.task_return(t, result, Packet::Bytes(data)));
}
