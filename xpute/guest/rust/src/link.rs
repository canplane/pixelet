// xpute-guest/link.rs

//! The guest's end of the link (xpute-link link.rs): its one side, which lays
//! in its outbox and crosses by `link_out`, what its three exports run
//! (lib.rs `link!`), and the entry they run each call in. The machine's names are read here as the
//! guest's callers have read them.

use std::cell::Cell;
use std::rc::Rc;

use xpute_kit::status::errno::Errno;
use xpute_link::abi::types::Token;
use xpute_link::frame::HDR_SZ;
use xpute_link::layout;
use xpute_link::mem;
use xpute_link::{Answer, Args, Handle, Layout, Method, Object, Packet, Side, Tag};

// A module takes the host's `link_out` under `env` (@xpute/runtime-web
// worker.ts `instantiate`).
#[cfg_attr(target_arch = "wasm32", link(wasm_import_module = "env"))]
unsafe extern "C" {
    fn link_out(at: u64) -> i32;
}

/// Crosses with the frame the guest laid at `at` in its outbox, past the
/// inbox, by `link_out`.
fn cross_out(at: u64) -> i32 {
    // SAFETY: the frame just laid in the outbox.
    unsafe { link_out(at) }
}

thread_local! {
    static SIDE: Side = const { Side::new(|l| l.inbox + u64::from(l.cap), cross_out) };
    /// Whether the guest is in an entry.
    static ENTERED: Cell<bool> = const { Cell::new(false) };
    /// Whether the guest is laying its link (`init`), before the host can
    /// hear anything over it. A thread's own, it reads clear on any other
    /// thread, where the guest's layout reads unset as it does in `init`.
    static INIT: Cell<bool> = const { Cell::new(false) };
    /// The program's start and end of an entry (`init`).
    static ENTER: Cell<fn()> = const { Cell::new(|| {}) };
    static END: Cell<fn() -> bool> = const { Cell::new(|| false) };
}

/// Lays the link over frames of `cap` bytes, a multiple of 8 that holds a
/// frame's words, and runs nothing of the program; `enter` and `end` are the
/// program's start and end of every entry (`boot`, `recv`). It answers where
/// the link's layout lies: the inbox's address and a frame's cap
/// (frame.json5 `LAYOUT`), then the inbox and the outbox.
pub fn init(cap: u32, enter: fn(), end: fn() -> bool) -> u64 {
    INIT.set(true);
    xpute_kit::ensure!(cap.is_multiple_of(8) && cap >= HDR_SZ, EINVAL, cap);
    let words: &'static mut [u64] = Vec::leak(vec![0; (4 * layout::WORDS + 2 * cap as usize) / 8]);
    let at = mem::addr(&words[0]);
    let l = Layout {
        inbox: at + 4 * layout::WORDS as u64,
        cap,
    };
    // SAFETY: the layout's words, which only `init` writes.
    unsafe { l.lay(at) };
    SIDE.with(|s| s.set_layout(l));
    ENTER.set(enter);
    END.set(end);
    INIT.set(false);
    at
}

/// Whether the guest is laying its link on this thread (`init`): its own code
/// runs, and its failure is told by its trap alone, as the host cannot hear a
/// stop yet. A failed init leaves it set, the guest entered no more.
pub fn in_init() -> bool {
    INIT.get()
}

/// The guest's first entry, once the host has read the layout. Unlike every
/// later entry, it starts by `boot`, which opens what the program's `enter`
/// may read, and only then by `enter`; it ends as every entry does (`leave`).
pub fn boot(boot: impl FnOnce()) {
    xpute_kit::ensure!(!ENTERED.replace(true), EBUSY);
    boot();
    ENTER.get()();
    leave();
}

/// Puts `open` as the opener of the URIs that start with `prefix`, whose
/// objects are the resources of the interface of the named handle `h`.
pub fn mount(
    h: Handle,
    prefix: Vec<Token>,
    open: impl Fn(&[Token], Handle) -> Result<Rc<dyn Object>, Errno> + 'static,
) {
    SIDE.with(|s| s.mount(h, prefix, open));
}

/// Mounts `o` under its interface's named handle `h`.
pub fn mount_named(h: Handle, o: Rc<dyn Object>) {
    SIDE.with(|s| s.mount_named(h, o));
}

/// Adds `o` under a handle issued for it, for a method to answer.
pub fn add(o: Rc<dyn Object>) -> Handle {
    SIDE.with(|s| s.add(o))
}

/// Opens the object `uri` names on the host, and answers its handle.
pub fn open(uri: &[Token]) -> Result<Handle, Errno> {
    SIDE.with(|s| s.open(uri))
}

/// Lets the object `h` names on the host go.
pub fn drop(h: Handle) {
    SIDE.with(|s| s.drop(h));
}

/// Calls `m` of `h`, which answers at once: its result, and its return data,
/// readable until the next crossing.
pub fn call(h: Handle, m: Method, args: Args) -> (i32, Option<&'static [u8]>) {
    SIDE.with(|s| s.call(h, m, Packet::Args(args)))
}

/// Sends `m` of `h`, its answer pending from before the crossing, as the
/// host's is (link.ts `send`), and answers the request's tag.
pub fn send(h: Handle, m: Method, args: Args, answer: Option<Answer>) -> Tag {
    SIDE.with(|s| s.send(h, m, Packet::Args(args), answer))
}

/// Cancels the request `t`, while its answer is pending (frame.json5 `CANCEL`).
pub fn cancel(t: Tag) {
    SIDE.with(|s| s.cancel(t));
}

/// Puts `stop` as how the request `t` the guest answers stops if it is
/// canceled.
pub fn on_cancel(t: Tag, stop: Box<dyn FnOnce()>) {
    SIDE.with(|s| s.on_cancel(t, stop));
}

/// Answers the request `t`, whose method answered EINPROGRESS, with `result`
/// and its return data. A later answer is found by its tag alone.
pub fn task_return(t: Tag, result: i32, data: Args) {
    SIDE.with(|s| s.task_return(t, result, Packet::Args(data)));
}

/// Whether control is in the guest's own code on this thread: inside an
/// entry, and not out on the host through `link_out`. A program's report
/// of its failures takes only these, as hooks are the process's natively.
pub fn in_guest() -> bool {
    ENTERED.get() && !SIDE.with(Side::crossing)
}

/// Runs `f`, the body of a native export, its panic caught as `trap`.
#[cfg(not(target_arch = "wasm32"))]
pub fn caught<T>(trap: T, f: impl FnOnce() -> T) -> T {
    std::panic::catch_unwind(std::panic::AssertUnwindSafe(f)).unwrap_or(trap)
}

/// The frame the host laid in the inbox: a later answer, to its request,
/// or a method, to its object, in an entry of its own.
pub fn recv() -> i32 {
    enter();
    let result = SIDE.with(|s| s.recv(s.layout().inbox));
    leave();
    result
}

/// Starts an entry by the program's `enter`. Every entry ends by `leave`.
fn enter() {
    xpute_kit::ensure!(!ENTERED.replace(true), EBUSY);
    ENTER.get()();
}

/// Ends an entry: runs the answers due and the program's `end`, which answers
/// whether it left work, again until neither has work left, and ends the
/// entry.
fn leave() {
    loop {
        SIDE.with(Side::run_due);
        let again = END.get()();
        if !again && !SIDE.with(Side::has_due) {
            break;
        }
    }
    ENTERED.set(false);
}
