// pxl-core/sys/host.test.rs
//
// The host the core's tests run against: the other side of the link, laid as
// the guest's init lays it on a host. It implements the monotonic clock's
// `now` at the time a test sets (`set_clock`) and what a test mounts
// (`mount`), and answers a request later as a test says (`answer`). A call of
// an interface nothing mounts answers no-entry, and of the clock's other
// methods ENOSYS.

use std::cell::Cell;
use std::rc::Rc;

use xpute_guest::abi::clocks::monotonic_clock::{self, NOW};
use xpute_guest::clock::duration;
use xpute_guest::link::{init, recv};
use xpute_kit::status::errno::Errno;
use xpute_link::frame::HDR_SZ;
use xpute_link::{Handle, Layout, Method, Object, Packet, Reply, Side, Tag};

use crate::sys::heap::Heap;

/// A frame's cap: its words and a page, past every packet the tests lay.
const CAP: u32 = HDR_SZ + 4096;

thread_local! {
    static SIDE: Side = const { Side::new(|l| l.inbox, |_| recv()) };
    static LAID: Cell<bool> = const { Cell::new(false) };
    /// The clock's time, in ns.
    static NOW_NS: Cell<u64> = const { Cell::new(0) };
}

/// The guest's import, implemented by this side.
#[unsafe(no_mangle)]
extern "C" fn link_out(at: u64) -> i32 {
    SIDE.with(|s| s.recv(at))
}

/// The core's heap, the tests' process's global allocator, as pxl-runtime
/// declares it a device's.
#[global_allocator]
static HEAP: Heap = Heap;

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

/// Mounts `o` under the named handle `h`.
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
