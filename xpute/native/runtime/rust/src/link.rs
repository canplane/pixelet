// xpute-native-runtime/link.rs

//! The host's side of the link (link/wit/types.wit), the same machine as the
//! guest's (xpute-link link.rs `Side`) over the other box, which the runtime
//! holds for the host as a page's worker holds it (worker.ts `instantiate`):
//! it lays its frames
//! in the guest's inbox and crosses by entering the guest, which is linked
//! into the app as a module is on a page (link.ts `Guest`): its three
//! exports, as C symbols, and `link_out`, which the guest imports. The
//! guest's memory is the process's, which the guest set before it laid its
//! link. A guest that stops, a Rust guest's panic or a C++ guest's bug,
//! answers a trap (frame.json `TRAP`, or `TRAP_AT` from init), as a module traps: the runtime tells the platform and
//! enters the guest no more, and the UI stays. What differs from the guest's side is
//! only here: the layout is read from the guest's export, the answers due run
//! after each piece of the event loop's work (`run_due`), where a page's run
//! as microtasks, and while control is in the guest (the side's `crossing`:
//! an entry, or the guest's init and boot), a completion given by an object
//! it serves waits for the work at hand to end.

use std::cell::{Cell, RefCell};
use std::collections::VecDeque;
use std::rc::Rc;

use xpute_link::abi::types::Token;
use xpute_link::frame::HDR_SZ;
use xpute_link::{Answer, Args, Handle, Layout, Lower, Method, Object, Packet, Side, Tag, TRAP, TRAP_AT};
use xpute_kit::status::errno::Errno;

unsafe extern "C" {
    /// Lays the link and answers where its layout lies, or `TRAP_AT`.
    fn link_init() -> u32;
    /// The guest's first entry, which boots it: 0, or `TRAP`.
    fn link_boot() -> i32;
    /// Enters the guest with the frame laid in its inbox.
    fn link_in() -> i32;
}

/// The host's crossing: one guest entry with the frame laid in its inbox,
/// on the event loop's thread, never while an entry runs; a trapped link
/// lays and crosses nothing (xpute-link link.rs `Side::out`).
fn cross_in(_at: u32) -> i32 {
    // SAFETY: the guest's export, on its thread, one entry at a time.
    match unsafe { link_in() } {
        TRAP => {
            trap();
            Errno::EPIPE as i32
        }
        result => result,
    }
}

/// Marks the link trapped, and tells the platform the first time, as a
/// page's worker tells its trap. On the guest's thread.
pub fn trap() {
    if SIDE.with(Side::trap) {
        crate::log("[guest] trapped");
        if let Some(report) = REPORT.get() {
            report();
        }
    }
}

/// A completion given while control is in the guest: its request, result
/// and packet.
type Completion = (Tag, i32, Option<Vec<u8>>);

thread_local! {
    static SIDE: Side = const { Side::new(|l| l.inbox, cross_in) };
    /// The completions given while control is in the guest, with their
    /// packets written.
    static COMPLETIONS: RefCell<VecDeque<Completion>> = const { RefCell::new(VecDeque::new()) };
    /// The platform's report of a trap, given with the guest's first entry.
    static REPORT: Cell<Option<fn()>> = const { Cell::new(None) };
}

/// Enters the guest first, once the host's own objects are mounted: the
/// guest lays its link, the host reads its layout, then the guest boots and
/// may open them (link.ts `init`). `report` hears a trap, from then on.
/// False where the link trapped, whichever side failed.
pub fn init(report: fn()) -> bool {
    REPORT.set(Some(report));
    // SAFETY: the guest's export, called once, before any other.
    let at = SIDE.with(|s| s.crossed(|| unsafe { link_init() }));
    if at == TRAP_AT {
        trap();
        return false;
    }
    SIDE.with(|s| s.set_layout(Layout::at(at)));
    // SAFETY: the guest's export, called once, right after `link_init`.
    if SIDE.with(|s| s.crossed(|| unsafe { link_boot() })) == TRAP {
        trap();
    }
    !SIDE.with(Side::trapped)
}

pub fn mount(
    prefix: Vec<Token>,
    open: impl Fn(&[Token], Handle) -> Result<Rc<dyn Object>, Errno> + 'static,
) {
    SIDE.with(|s| s.mount(prefix, open));
}

/// Serves `o` under its interface's named handle `h`.
pub fn mount_named(h: Handle, o: Rc<dyn Object>) {
    SIDE.with(|s| s.mount_named(h, o));
}

/// Serves `o` under a handle issued for it, for a method to answer.
pub fn add(o: Rc<dyn Object>) -> Handle {
    SIDE.with(|s| s.add(o))
}

pub fn open(uri: &[Token]) -> Result<Handle, Errno> {
    SIDE.with(|s| s.open(uri))
}

pub fn call(h: Handle, m: Method, args: Args) -> (i32, Option<&'static [u8]>) {
    SIDE.with(|s| s.call(h, m, Packet::Args(args)))
}

pub fn send(h: Handle, m: Method, args: Args, answer: Option<Answer>) -> Tag {
    send_pkt(h, m, Packet::Args(args), answer)
}

/// `send` with a packet written already, as the UI's (remote.rs).
pub(crate) fn send_pkt(h: Handle, m: Method, pkt: Packet, answer: Option<Answer>) -> Tag {
    SIDE.with(|s| s.send(h, m, pkt, answer))
}

/// Cancels the request `t`, while its answer is pending (frame.json `CANCEL`).
pub fn cancel(t: Tag) {
    SIDE.with(|s| s.cancel(t));
}

/// Puts `stop` as how the request `t` the host serves stops if it is
/// canceled.
pub fn on_cancel(t: Tag, stop: Box<dyn FnOnce()>) {
    SIDE.with(|s| s.on_cancel(t, stop));
}

/// The bytes a frame's packet may take.
pub(crate) fn pkt_cap() -> u32 {
    SIDE.with(Side::layout).room - HDR_SZ
}

/// Answers the guest's request `t`; while control is in the guest, its packet
/// is written now and laid after the work at hand. A packet past the room is
/// answered EMSGSIZE with no data, as Side's own is.
pub fn task_return(t: Tag, result: i32, data: Args) {
    if !SIDE.with(Side::crossing) {
        return SIDE.with(|s| s.task_return(t, result, Packet::Args(data)));
    }
    let written = data.map(|(fixed_sz, write)| {
        let mut buf = vec![0; pkt_cap() as usize];
        let mut w = Lower::new(&mut buf, fixed_sz);
        write(&mut w);
        let len = w.finish().ok()?;
        buf.truncate(len);
        Some(buf)
    });
    let completion = match written {
        Some(None) => (t, Errno::EMSGSIZE as i32, None),
        pkt => (t, result, pkt.flatten()),
    };
    COMPLETIONS.with_borrow_mut(|l| l.push_back(completion));
}

/// Answers the guest's request `t` with a packet written already, as the
/// UI's answer to a method it serves (remote.rs `answer`), on the event
/// loop's thread between entries. A packet past the room is answered
/// EMSGSIZE with no data, as Side's own is.
pub(crate) fn task_return_pkt(t: Tag, result: i32, pkt: Option<Vec<u8>>) {
    let pkt = pkt.as_deref().map_or(Packet::Args(None), Packet::Bytes);
    SIDE.with(|s| s.task_return(t, result, pkt));
}

/// The guest's import: the frame it laid at `at`, taken by the host's side
/// of the link on this thread (link.ts `imports`). A panic of the host's
/// while it serves, which its hook has reported, unwinds no further: the link
/// is trapped, and the guest's call is answered ENOTRECOVERABLE. Once
/// trapped, a frame is not served: EPIPE, as `cross_in` answers.
#[unsafe(no_mangle)]
pub extern "C" fn link_out(at: u32) -> i32 {
    if SIDE.with(Side::trapped) {
        return Errno::EPIPE as i32;
    }
    std::panic::catch_unwind(|| SIDE.with(|s| s.recv(at))).unwrap_or_else(|_| {
        trap();
        Errno::ENOTRECOVERABLE as i32
    })
}

/// Lays the completions given while serving, then runs the answers due,
/// after the work at hand.
pub fn run_due() {
    while let Some((t, result, pkt)) = COMPLETIONS.with_borrow_mut(VecDeque::pop_front) {
        let pkt = pkt.as_deref().map_or(Packet::Args(None), Packet::Bytes);
        SIDE.with(|s| s.task_return(t, result, pkt));
    }
    SIDE.with(Side::run_due);
}
