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
//! answers a trap (frame.json5 `TRAP`, or `TRAP_AT` from init), as a module traps: the runtime tells the platform and
//! enters the guest no more, and the UI stays. What differs from the guest's side is
//! only here: the layout is read from the guest's export, the answers due run
//! after each piece of the event loop's work (`run_due`), where a page's run
//! as microtasks, and while control is in the guest (the side's `crossing`:
//! an entry, or the guest's init and boot), a completion given by an object
//! it serves waits for the work at hand to end.

use std::cell::RefCell;
use std::collections::VecDeque;
use std::rc::Rc;
use std::sync::OnceLock;
use std::thread::ThreadId;

use xpute_kit::status::errno::Errno;
use xpute_link::abi::types::Token;
use xpute_link::frame::HDR_SZ;
use xpute_link::{
    Answer, Args, Handle, Layout, Lower, Method, Object, Packet, Side, Tag, TRAP, TRAP_AT,
};

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
            trap(GUEST_TRAPPED);
            Errno::EPIPE as i32
        }
        result => result,
    }
}

/// Whether the link is trapped (`trap`). On the guest's thread.
pub(crate) fn trapped() -> bool {
    SIDE.with(Side::trapped)
}

/// The guest's thread, the one that mounts what the host serves and enters
/// the guest, which the runtime names as it starts it (native.rs `run`).
static GUEST_THREAD: OnceLock<ThreadId> = OnceLock::new();

/// Names this thread the guest's.
pub(crate) fn record_guest_thread() {
    GUEST_THREAD.get_or_init(|| std::thread::current().id());
}

/// Whether this is the guest's thread, where the link's state is.
pub(crate) fn on_guest_thread() -> bool {
    GUEST_THREAD.get() == Some(&std::thread::current().id())
}

/// What a trap out of the guest's export says: the guest told its failure
/// by its stop already, if it could.
const GUEST_TRAPPED: &str = "the guest trapped";

/// What a trap from a panic of the host's says, which its hook follows with
/// where it stopped (native.rs `hook`).
pub(crate) const HOST_STOPPED: &str = "the host stopped";

/// Marks the link trapped, and tells the platform the first time, with
/// `err`, what stopped it, as a page's worker tells its trap with its error
/// (listener.rs `tell_trap`). On the guest's thread.
pub(crate) fn trap(err: &str) {
    if SIDE.with(Side::trap) {
        crate::log(&format!("[link] trapped: {err}"));
        crate::listener::tell_trap(err);
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
}

/// Enters the guest first, once the host's own objects are mounted: the
/// guest lays its link, the host reads its layout, then the guest boots and
/// may open them (link.ts `init`). Where an interface of `imports` (the
/// bindings' `IMPORTS`) is served by nothing mounted, under its named handle
/// or by an opener of its resources (xpute-link `Side::serves`), the link
/// traps, naming each, and the guest is never entered. False where the link
/// trapped, whichever side failed.
pub(crate) fn init(imports: &[(&str, Handle)]) -> bool {
    let unserved: Vec<&str> = imports
        .iter()
        .filter(|&&(_, h)| !SIDE.with(|s| s.serves(h)))
        .map(|&(name, _)| name)
        .collect();
    if !unserved.is_empty() {
        trap(&format!("no server mounts {}", unserved.join(", ")));
        return false;
    }
    // SAFETY: the guest's export, called once, before any other.
    let at = SIDE.with(|s| s.crossed(|| unsafe { link_init() }));
    if at == TRAP_AT {
        trap(GUEST_TRAPPED);
        return false;
    }
    SIDE.with(|s| s.set_layout(Layout::at(at)));
    // SAFETY: the guest's export, called once, right after `link_init`.
    if SIDE.with(|s| s.crossed(|| unsafe { link_boot() })) == TRAP {
        trap(GUEST_TRAPPED);
    }
    !trapped()
}

pub fn mount(
    h: Handle,
    prefix: Vec<Token>,
    open: impl Fn(&[Token], Handle) -> Result<Rc<dyn Object>, Errno> + 'static,
) {
    SIDE.with(|s| s.mount(h, prefix, open));
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

/// Lets the guest's object `h` go (frame.json5 `DROP`).
pub fn drop(h: Handle) {
    SIDE.with(|s| s.drop(h));
}

pub fn send(h: Handle, m: Method, args: Args, answer: Option<Answer>) -> Tag {
    send_pkt(h, m, Packet::Args(args), answer)
}

/// `send` with a packet written already, as the UI's (remote.rs).
pub(crate) fn send_pkt(h: Handle, m: Method, pkt: Packet, answer: Option<Answer>) -> Tag {
    SIDE.with(|s| s.send(h, m, pkt, answer))
}

/// Cancels the request `t`, while its answer is pending (frame.json5 `CANCEL`).
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
    SIDE.with(Side::layout).cap - HDR_SZ
}

/// Answers the guest's request `t`; while control is in the guest, its packet
/// is written now and laid after the work at hand. A packet past the cap is
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
/// loop's thread between entries. A packet past the cap is answered
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
    if trapped() {
        return Errno::EPIPE as i32;
    }
    std::panic::catch_unwind(|| SIDE.with(|s| s.recv(at))).unwrap_or_else(|_| {
        trap(HOST_STOPPED);
        Errno::ENOTRECOVERABLE as i32
    })
}

/// Lays the completions given while serving, then runs the answers due,
/// after the work at hand.
pub(crate) fn run_due() {
    while let Some((t, result, pkt)) = COMPLETIONS.with_borrow_mut(VecDeque::pop_front) {
        let pkt = pkt.as_deref().map_or(Packet::Args(None), Packet::Bytes);
        SIDE.with(|s| s.task_return(t, result, pkt));
    }
    SIDE.with(Side::run_due);
}
