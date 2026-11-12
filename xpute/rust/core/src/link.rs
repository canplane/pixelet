// xpute-core/link.rs

//! The core's link (wit/link.wit). Whatever is called is an object: named by
//! a URI, a tuple of tokens, opened by the root's `open` to a handle the
//! serving side issues, and called by its methods. A frame is one method
//! called: its tag the request, unique among the sender's pending ones, its
//! command the handle, the method and a RES bit (io/frame.rs), its packet the
//! args in XTP. A frame crosses one way through one function, `link_out` from
//! the core and `link_in` into it, and each crossing returns at once what the
//! method came to: its result, or EINPROGRESS for one that answers later, by
//! a frame marked RES with the request's tag. So each side has the same ways:
//!
//! - `call` takes what the crossing returns, for a method that always answers
//!   at once, and stops as a bug on EINPROGRESS;
//! - `send` hands it to its answer, and an `async func` is sent;
//! - `mount` puts the opener of the objects the core serves under a URI's
//!   leading tokens, the longest matching winning; `open` and `release` reach
//!   the root's open and a handle's release on the other side.
//!
//! The link holds these on both ends:
//!
//! - One core entry at a time: no `link_in` starts before another returns,
//!   and the outside never enters the core nested.
//! - An answer is never run inside `send`, and a result at once and a later
//!   one take the same path. On the core, the answers run at the end of the
//!   entry, once its handler has returned, just before `link_in` returns; on
//!   the runtime, after the work at hand (`queueMicrotask`). An entry ends
//!   when no answer is due and the program's end has no work left: those made
//!   due while answers run, or by the end's work, run in it too.
//! - A frame's packet is the receiver's for the crossing alone, and memory its
//!   args lend by (at, len) is the callee's until the answer: the caller keeps
//!   it, as the answer it owns.
//! - The callee answers in the frame the request came in: it lays it again,
//!   marked RES, with its result and its return data as the packet. A call's
//!   return data is readable until the next crossing; a send's answer gets a
//!   copy, which it owns.

use std::cell::{Cell, RefCell};
use std::collections::{BTreeMap, VecDeque};
use std::rc::Rc;

use xpute_kit::status::bug::OrBug;
use xpute_kit::status::errno::Errno;
use xpute_kit::status::error::MarshalError;
use xpute_kit::wire::xtp::{BranchCursor, PacketWriter, TreeReader};

use crate::abi::link::Token;
use crate::io::frame::{Frame, WORD_BYTES};
use crate::mem;
use crate::resource::Table;

pub use crate::resource::Handle;

// A module takes the runtime's `link_out` under `env` (TS link.ts `imports`).
#[cfg_attr(target_arch = "wasm32", link(wasm_import_module = "env"))]
unsafe extern "C" {
    fn link_out(at: u32) -> i32;
}

/// What an async method lends: a buffer that says where it lies in the
/// core's memory and how long (wit/link.wit `lent`), and stays put while the
/// answer owns it.
pub trait Lend: Unpin + 'static {
    fn span(&mut self) -> (u32, u32);
}

impl<T: Copy + Unpin + 'static> Lend for Vec<T> {
    fn span(&mut self) -> (u32, u32) {
        (mem::off(self.as_ptr().cast()), core::mem::size_of_val(&self[..]) as u32)
    }
}

pub type Tag = u32;
pub type Method = u32;

/// The root's handle, and its one method.
pub const ROOT: Handle = 0;
pub const OPEN: Method = 0;
/// The method every handle has, which lets it go.
pub const RELEASE: Method = 0xff;

/// An object the core serves: given a method, the request's tag, for a later
/// answer (`complete`), and the args, it answers.
pub trait Object {
    fn call(&self, m: Method, tag: Tag, args: Option<BranchCursor<'_>>) -> Reply;
}

/// What hears an async method's value, once it has one, and answers the
/// request with it.
pub type Return<T> = Box<dyn FnOnce(Result<T, Errno>)>;

/// What opens the object a URI names, given the handle it is issued.
type Opener = dyn Fn(&[Token], Handle) -> Result<Rc<dyn Object>, Errno>;

/// What a method answers: its result, a status or a value, and its return
/// data, written once its args are read.
pub struct Reply {
    pub result: i32,
    pub data: Data,
}

impl From<i32> for Reply {
    fn from(result: i32) -> Reply {
        Reply { result, data: None }
    }
}

/// Return data, owned until it is written: how many fields, and what writes
/// them. None, or no fields, is no packet.
pub type Data = Option<(u32, Box<dyn Fn(&mut PacketWriter)>)>;

/// What hears a sent method's answer: its result, and a copy of its return
/// data, which it owns.
pub type Answer = Box<dyn FnOnce(i32, Option<Vec<u8>>)>;

/// A method's args: how many fields, and what writes them. None, or no
/// fields, is no packet.
pub type Args<'a> = Option<(u32, &'a dyn Fn(&mut PacketWriter))>;

/// The inbox, and a frame's room. The outbox lies right past the inbox.
#[derive(Clone, Copy)]
struct Link {
    inbox: u32,
    room: u32,
}

thread_local! {
    static LINK: Cell<Option<Link>> = const { Cell::new(None) };
    /// Where the next request's tag is sought.
    static NEXT: Cell<Tag> = const { Cell::new(0) };
    /// The openers, by the URI's leading tokens.
    static MOUNTS: RefCell<Vec<(Vec<Token>, Rc<Opener>)>> = const { RefCell::new(Vec::new()) };
    /// The objects the core serves, by the handles it issued.
    static OBJECTS: RefCell<Table<Rc<dyn Object>>> = const { RefCell::new(Table::new()) };
    /// The answers of the sent requests that answer later, by tag.
    static PENDING: RefCell<BTreeMap<Tag, Answer>> = const { RefCell::new(BTreeMap::new()) };
    /// Whether the core is in an entry.
    static ENTERED: Cell<bool> = const { Cell::new(false) };
    /// The answers due, with their results, run as the entry ends.
    static DUE: RefCell<VecDeque<(Answer, i32, Option<Vec<u8>>)>> = const { RefCell::new(VecDeque::new()) };
    /// The program's start and end of an entry (`init`).
    static ENTER: Cell<fn()> = const { Cell::new(|| {}) };
    static END: Cell<fn() -> bool> = const { Cell::new(|| false) };
}

fn link() -> Link {
    LINK.get().or_bug(Errno::ENOTRECOVERABLE)
}

/// Defines the core's three exports (wit/link.wit), over frames of `room`
/// bytes, with `boot` mounting what the core serves and opening what it
/// calls, and `enter` and `end` starting and ending each entry.
#[macro_export]
macro_rules! link {
    ($room:expr, $boot:expr, $enter:expr, $end:expr) => {
        #[unsafe(no_mangle)]
        pub extern "C" fn link_init() -> u32 {
            $crate::link::init($room, $enter, $end)
        }
        #[unsafe(no_mangle)]
        pub extern "C" fn link_boot() {
            $crate::link::boot($boot)
        }
        #[unsafe(no_mangle)]
        pub extern "C" fn link_in() -> i32 {
            $crate::link::recv()
        }
    };
}

/// Lays the link over frames of `room` bytes, a multiple of 8 that holds a
/// frame's words, and runs nothing of the program; `enter` and `end` are the
/// program's start and end of every entry (`boot`, `recv`). It answers where
/// the link's layout lies: two words, the inbox and a frame's room, then the
/// inbox and the outbox.
pub fn init(room: u32, enter: fn(), end: fn() -> bool) -> u32 {
    xpute_kit::ensure!(room % 8 == 0 && room >= WORD_BYTES, EINVAL, room);
    let words: &'static mut [u64] = Vec::leak(vec![0; 1 + 2 * room as usize / 8]);
    let at = mem::off(words.as_ptr().cast());
    let inbox = at + 8;
    // SAFETY: the layout's two words, which only `init` writes.
    let layout = unsafe { mem::bytes(at, 8) };
    layout[..4].copy_from_slice(&inbox.to_le_bytes());
    layout[4..].copy_from_slice(&room.to_le_bytes());
    LINK.set(Some(Link { inbox, room }));
    ENTER.set(enter);
    END.set(end);
    at
}

/// The core's first entry, once the runtime has read the layout. Unlike every
/// later entry, it starts by `boot`, which opens what the program's `enter`
/// may read, and only then by `enter`; it ends as every entry does (`leave`).
pub fn boot(boot: impl FnOnce()) {
    xpute_kit::ensure!(!ENTERED.replace(true), EBUSY);
    boot();
    ENTER.get()();
    leave();
}

/// Puts `open` as the opener of the URIs that start with `prefix`.
pub fn mount(prefix: Vec<Token>, open: impl Fn(&[Token], Handle) -> Result<Rc<dyn Object>, Errno> + 'static) {
    MOUNTS.with_borrow_mut(|m| m.push((prefix, Rc::new(open))));
}

/// The URI of names alone, as a singleton's (package, interface).
pub fn uri(names: &[&str]) -> Vec<Token> {
    names.iter().map(|n| Token::Str((*n).to_owned())).collect()
}

/// Opens the object `uri` names on the runtime, and answers its handle.
pub fn open(uri: &[Token]) -> Result<Handle, Errno> {
    let write = |w: &mut PacketWriter| {
        w.branch(uri.len() as u32, |w| uri.iter().for_each(|t| t.write(w)));
    };
    answer(call(ROOT, OPEN, Some((1, &write))).0)
}

/// Lets the object `h` names on the runtime go.
pub fn release(h: Handle) {
    send(h, RELEASE, None, None);
}

/// Calls `m` of `h`, which answers at once: its result, and its return data,
/// readable until the next crossing.
pub fn call(h: Handle, m: Method, args: Args) -> (i32, Option<&'static [u8]>) {
    let (result, data) = out(tag(), h, m, false, 0, args);
    xpute_kit::ensure!(result != Errno::EINPROGRESS as i32, EINPROGRESS, h, m);
    (result, data)
}

/// Sends `m` of `h`, its answer pending from before the crossing, as the
/// runtime's is (link.ts `send`).
pub fn send(h: Handle, m: Method, args: Args, answer: Option<Answer>) {
    let t = tag();
    if let Some(a) = answer {
        PENDING.with_borrow_mut(|p| p.insert(t, a));
    }
    let (result, data) = out(t, h, m, false, 0, args);
    if result != Errno::EINPROGRESS as i32 {
        if let Some(a) = PENDING.with_borrow_mut(|p| p.remove(&t)) {
            DUE.with_borrow_mut(|d| d.push_back((a, result, data.map(<[u8]>::to_vec))));
        }
    }
}

/// Answers the request `t`, whose method answered EINPROGRESS, with `result`
/// and its return data. A later answer is found by its tag alone.
pub fn complete(t: Tag, result: i32, data: Args) {
    out(t, ROOT, 0, true, result, data);
}

/// A method's result as its value, or the errno it refused with.
pub fn answer(result: i32) -> Result<u32, Errno> {
    match u32::try_from(result) {
        Ok(v) => Ok(v),
        Err(_) => Err(Errno::of(result).or_bug(Errno::EPROTO)),
    }
}

/// The frame the runtime laid in the inbox: a later answer, to its request,
/// or a method, to its object.
pub fn recv() -> i32 {
    enter();
    let result = apply();
    leave();
    result
}

/// Takes the inbox's frame: a later answer, made due, or a method, run by its
/// object.
fn apply() -> i32 {
    let l = link();
    let f = Frame::at(l.inbox);
    let pkt = f.pkt(l.inbox, l.room);
    if f.res {
        if let Some(a) = PENDING.with_borrow_mut(|p| p.remove(&f.tag)) {
            let data = pkt.ok().flatten().map(<[u8]>::to_vec);
            DUE.with_borrow_mut(|d| d.push_back((a, f.result, data)));
        }
        return 0;
    }
    let reply = match pkt.and_then(args_of) {
        Ok(args) => serve(f.handle, f.method, f.tag, args),
        Err(e) => Reply::from(e.errnum as i32),
    };
    // The args are read: the answer goes in the frame they came in.
    let data = reply.data.as_ref().map(|(n, w)| (*n, &**w as &dyn Fn(&mut PacketWriter)));
    lay(l.inbox, f.tag, f.handle, f.method, true, reply.result, data);
    reply.result
}

/// Runs `m` of `h`: the root's open, a release, or an object's method.
fn serve(h: Handle, m: Method, tag: Tag, args: Option<BranchCursor<'_>>) -> Reply {
    match (h, m) {
        (ROOT, OPEN) => Reply::from(opened(args).map_or_else(|e| e as i32, |h| h as i32)),
        (_, RELEASE) => match OBJECTS.with_borrow_mut(|t| t.remove(h)) {
            Some(_) => Reply::from(0),
            None => Reply::from(Errno::EBADF as i32),
        },
        _ => match OBJECTS.with_borrow(|t| t.get(h).cloned()) {
            Some(o) => o.call(m, tag, args),
            None => Reply::from(Errno::EBADF as i32),
        },
    }
}

/// Opens the object the args' URI names, by the opener of its longest
/// matching mount, and issues its handle.
fn opened(args: Option<BranchCursor<'_>>) -> Result<Handle, Errno> {
    let uri = args
        .ok_or(MarshalError::new(Errno::EBADMSG))
        .and_then(|b| b.at(0)?.as_branch())
        .and_then(|b| b.iter().map(|c| Token::read(c?)).collect::<Result<Vec<_>, _>>())
        .map_err(|e| e.errnum)?;
    let open = MOUNTS
        .with_borrow(|m| {
            m.iter()
                .filter(|(p, _)| uri.starts_with(p))
                .rev()
                .max_by_key(|(p, _)| p.len())
                .map(|(_, o)| o.clone())
        })
        .ok_or(Errno::ENOENT)?;
    let h = OBJECTS.with_borrow_mut(Table::issue);
    let o = open(&uri, h)?;
    OBJECTS.with_borrow_mut(|t| t.put(h, o));
    Ok(h)
}

/// Starts an entry by the program's `enter`. Every entry ends by `leave`.
fn enter() {
    xpute_kit::ensure!(!ENTERED.replace(true), EBUSY);
    ENTER.get()();
}

/// Ends an entry: runs the answers due and the program's `end`, which answers
/// whether it left work, again until neither has work
/// left, and ends the entry.
fn leave() {
    loop {
        while let Some((a, result, data)) = DUE.with_borrow_mut(|d| d.pop_front()) {
            a(result, data);
        }
        let again = END.get()();
        if !again && DUE.with_borrow(VecDeque::is_empty) {
            break;
        }
    }
    ENTERED.set(false);
}

/// The next request's tag: a counter that wraps, past every tag still
/// pending, so a late answer, which comes only for a pending tag, finds its
/// own request.
fn tag() -> Tag {
    PENDING.with_borrow(|p| {
        let mut t = NEXT.get();
        while p.contains_key(&t) {
            t = t.wrapping_add(1);
        }
        NEXT.set(t.wrapping_add(1));
        t
    })
}

fn args_of(pkt: Option<&'static [u8]>) -> Result<Option<BranchCursor<'static>>, MarshalError> {
    pkt.map(|p| TreeReader::new(p)?.read_branch()).transpose()
}

/// Lays a frame at `at`, with the packet `args` write.
fn lay(at: u32, tag: Tag, handle: Handle, method: Method, res: bool, result: i32, args: Args) {
    let l = link();
    let pkt_len = match args {
        Some((fields, write)) if fields > 0 => {
            // SAFETY: the frame's room past its words, which only `lay` writes.
            let room = unsafe { mem::bytes(at + WORD_BYTES, l.room - WORD_BYTES) };
            let mut w = PacketWriter::new(room, fields);
            write(&mut w);
            w.fit_len().or_bug(Errno::EMSGSIZE)
        }
        _ => 0,
    };
    Frame {
        tag,
        res,
        handle,
        method,
        result,
        pkt_len,
    }
    .lay(at);
}

/// Lays a frame in the outbox and crosses with it, and answers what the
/// callee laid there: its result, and its return data.
fn out(tag: Tag, h: Handle, m: Method, res: bool, result: i32, args: Args) -> (i32, Option<&'static [u8]>) {
    let l = link();
    let outbox = l.inbox + l.room;
    lay(outbox, tag, h, m, res, result, args);
    // SAFETY: the frame just laid in the outbox.
    let result = unsafe { link_out(outbox) };
    let f = Frame::at(outbox);
    let data = if f.res && !res {
        f.pkt(outbox, l.room).ok().flatten()
    } else {
        None
    };
    (result, data)
}
