// xpute-link/link.rs

//! The link (link/wit/types.wit). Whatever is called is an object, called by
//! its methods under a handle: an interface's named handle, the hash of its
//! name (frame.json `NAMED`), or a resource's, named by a URI, a tuple of
//! tokens, and opened by the root's `open` to a handle the serving side
//! issues. A frame is one method called: its
//! tag the request, unique among the sender's pending ones, its command the
//! handle, the method and a RES bit (frame.rs), its packet the args in XTP. A
//! frame crosses one way through one function, `link_out` from the guest and
//! `link_in` into it, and each crossing returns at once what the method came
//! to: its result, or EINPROGRESS for one that answers later, by a frame marked
//! RES with the request's tag. So each side has the same ways:
//!
//! - `call` takes what the crossing returns, for a method that always answers
//!   at once, and stops as a bug on EINPROGRESS;
//! - `send` hands it to its answer, and an `async func` is sent;
//! - `mount_named` serves an interface under its named handle, and `mount`
//!   puts the opener of the resources the side serves under a URI's leading
//!   tokens, the longest matching winning; `open` and `drop` reach the root's
//!   open and a handle's drop on the other side;
//! - `cancel` cancels a request sent and pending (the root's CANCEL), and
//!   `on_cancel` puts what runs if a request the side serves is canceled.
//!
//! The link holds these on both ends:
//!
//! - One guest entry at a time: no `link_in` starts before another returns,
//!   and the outside never enters the guest nested.
//! - An answer is never run inside `send`, and a result at once and a later
//!   one take the same path. On the guest, the answers run at the end of the
//!   entry, once its handler has returned, just before `link_in` returns; on
//!   the host, after the work at hand (`queueMicrotask`). An entry ends
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

use crate::abi::types::Token;
use crate::frame::{Frame, WORD_BYTES};
use crate::mem;
use crate::resource::Table;

pub use crate::resource::Handle;


/// What an async method lends: a buffer that says where it lies in the guest's
/// memory and how long (link/wit/types.wit `lent`), and stays put while the
/// answer owns it.
pub trait Lend: Unpin + 'static {
    fn span(&mut self) -> (u32, u32);
}

impl<T: Copy + Unpin + 'static> Lend for Vec<T> {
    fn span(&mut self) -> (u32, u32) {
        (
            mem::off(self.as_ptr().cast()),
            core::mem::size_of_val(&self[..]) as u32,
        )
    }
}

pub type Tag = u32;
pub type Method = u32;

pub use crate::frame::spec::{CANCEL, DROP, NAMED, OPEN, ROOT};

/// An object a side serves: given a method, the request's tag, for a later
/// answer (`task_return`), and the args, it answers.
pub trait Object {
    fn call(&self, m: Method, tag: Tag, args: Option<BranchCursor<'_>>) -> Reply;
}

/// What answers an async method's request once the method has its value, and
/// where the method says how it stops if its caller cancels it (`CANCEL`).
pub struct Return<T> {
    tag: Tag,
    /// The side's `on_cancel`.
    on: fn(Tag, Box<dyn FnOnce()>),
    answer: Box<dyn FnOnce(Result<T, Errno>)>,
}

impl<T> Return<T> {
    /// The answer to request `tag` on the side whose `on_cancel` is `on`.
    pub fn new(tag: Tag, on: fn(Tag, Box<dyn FnOnce()>), answer: Box<dyn FnOnce(Result<T, Errno>)>) -> Return<T> {
        Return { tag, on, answer }
    }

    /// Answers the request with `v`.
    pub fn answer(self, v: Result<T, Errno>) {
        (self.answer)(v)
    }

    /// Runs `stop` if the caller cancels the request before its answer; the
    /// method then answers ECANCELED once it has stopped. A method that
    /// cannot stop says nothing, runs on and answers as it would.
    pub fn on_cancel(&self, stop: impl FnOnce() + 'static) {
        (self.on)(self.tag, Box::new(stop));
    }
}

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

/// A frame's packet: its args, written as the frame is laid, or a packet
/// written already, as one the UI sent (link.ts `Packet`).
#[derive(Clone, Copy)]
pub enum Packet<'a> {
    Args(Args<'a>),
    Bytes(&'a [u8]),
}

/// The link's layout: the guest's inbox and a frame's room. The outbox lies
/// right past the inbox.
#[derive(Clone, Copy)]
pub struct Layout {
    pub inbox: u32,
    pub room: u32,
}

impl Layout {
    /// The layout `init` wrote at `at`: its two words.
    pub fn at(at: u32) -> Layout {
        // SAFETY: the layout's two words, which `init` wrote.
        let b = unsafe { mem::bytes(at, 8) };
        Layout {
            inbox: u32::from_le_bytes([b[0], b[1], b[2], b[3]]),
            room: u32::from_le_bytes([b[4], b[5], b[6], b[7]]),
        }
    }
}

/// An answer due: what hears it, its result and its return data.
type Due = (Answer, i32, Option<Vec<u8>>);

/// One side of the link: what both sides do, once. A side lays its frames in
/// its own box and crosses by its own function: the guest lays in its outbox
/// and crosses by `link_out`, and the host lays in the guest's inbox and
/// crosses by `link_in`. It takes the other side's frame where that side laid
/// it (`recv`), and runs the answers due when its owner says the work at hand
/// is done (`run_due`): the guest as an entry ends, the host after each piece
/// of work.
pub struct Side {
    layout: Cell<Option<Layout>>,
    /// The box this side lays its frames in.
    out_box: fn(Layout) -> u32,
    /// Crosses with the frame laid at `at`, and answers what the method came
    /// to.
    cross: fn(u32) -> i32,
    /// Whether this side is crossing: control is on the other side.
    crossing: Cell<bool>,
    /// Where the next request's tag is sought.
    next: Cell<Tag>,
    /// The openers, by the URI's leading tokens.
    mounts: RefCell<Vec<(Vec<Token>, Rc<Opener>)>>,
    /// The objects this side serves, by the handles it issued.
    objects: RefCell<Table<Rc<dyn Object>>>,
    /// The interfaces this side serves, by their named handles (`NAMED`).
    named: RefCell<BTreeMap<Handle, Rc<dyn Object>>>,
    /// The answers of the sent requests that answer later, by tag.
    pending: RefCell<BTreeMap<Tag, Answer>>,
    /// How each request this side serves stops if its caller cancels it, by
    /// tag, until it answers.
    cancels: RefCell<BTreeMap<Tag, Box<dyn FnOnce()>>>,
    /// The answers due, run after the work at hand.
    due: RefCell<VecDeque<Due>>,
}

impl Side {
    pub const fn new(out_box: fn(Layout) -> u32, cross: fn(u32) -> i32) -> Side {
        Side {
            layout: Cell::new(None),
            out_box,
            cross,
            crossing: Cell::new(false),
            next: Cell::new(0),
            mounts: RefCell::new(Vec::new()),
            objects: RefCell::new(Table::new()),
            named: RefCell::new(BTreeMap::new()),
            pending: RefCell::new(BTreeMap::new()),
            cancels: RefCell::new(BTreeMap::new()),
            due: RefCell::new(VecDeque::new()),
        }
    }

    pub fn set_layout(&self, l: Layout) {
        self.layout.set(Some(l));
    }

    pub fn layout(&self) -> Layout {
        self.layout.get().or_bug(Errno::ENOTRECOVERABLE)
    }

    /// Whether this side is crossing, its control on the other side.
    pub fn crossing(&self) -> bool {
        self.crossing.get()
    }

    /// Runs `f`, a crossing of this side's own that is no frame (the
    /// host's entering of the guest's init and boot), as crossing.
    pub fn crossed<T>(&self, f: impl FnOnce() -> T) -> T {
        let was = self.crossing.replace(true);
        let v = f();
        self.crossing.set(was);
        v
    }

    /// Puts `open` as the opener of the URIs that start with `prefix`.
    pub fn mount(
        &self,
        prefix: Vec<Token>,
        open: impl Fn(&[Token], Handle) -> Result<Rc<dyn Object>, Errno> + 'static,
    ) {
        self.mounts.borrow_mut().push((prefix, Rc::new(open)));
    }

    /// Serves `o` under its interface's named handle `h`, which the other
    /// side calls with no open.
    pub fn mount_named(&self, h: Handle, o: Rc<dyn Object>) {
        let was = self.named.borrow_mut().insert(h, o);
        xpute_kit::ensure!(was.is_none(), EEXIST, h);
    }

    /// Opens the object `uri` names on the other side, and answers its
    /// handle.
    pub fn open(&self, uri: &[Token]) -> Result<Handle, Errno> {
        let write = |w: &mut PacketWriter| {
            w.branch(uri.len() as u32, |w| uri.iter().for_each(|t| t.write(w)));
        };
        answer(self.call(ROOT, OPEN, Packet::Args(Some((1, &write)))).0)
    }

    /// Lets the object `h` names on the other side go.
    pub fn drop(&self, h: Handle) {
        self.send(h, DROP, Packet::Args(None), None);
    }

    /// Calls `m` of `h`, which answers at once: its result, and its return
    /// data, readable until the next crossing.
    pub fn call(&self, h: Handle, m: Method, pkt: Packet) -> (i32, Option<&'static [u8]>) {
        let (result, data) = self.out(self.tag(), h, m, false, 0, pkt);
        xpute_kit::ensure!(result != Errno::EINPROGRESS as i32, EINPROGRESS, h, m);
        (result, data)
    }

    /// Sends `m` of `h`, its answer pending from before the crossing, so a
    /// later answer given inside it finds its request (link.ts `send`), and
    /// answers the request's tag.
    pub fn send(&self, h: Handle, m: Method, pkt: Packet, answer: Option<Answer>) -> Tag {
        let t = self.tag();
        if let Some(a) = answer {
            self.pending.borrow_mut().insert(t, a);
        }
        let (result, data) = self.out(t, h, m, false, 0, pkt);
        if result != Errno::EINPROGRESS as i32 {
            let a = self.pending.borrow_mut().remove(&t);
            if let Some(a) = a {
                self.due
                    .borrow_mut()
                    .push_back((a, result, data.map(<[u8]>::to_vec)));
            }
        }
        t
    }

    /// Cancels the request `t` this side sent, while its answer is pending;
    /// its answer still comes, and is heard as any other (`CANCEL`).
    pub fn cancel(&self, t: Tag) {
        if self.pending.borrow().contains_key(&t) {
            let write = |w: &mut PacketWriter| {
                w.u32(Some(t));
            };
            self.send(ROOT, CANCEL, Packet::Args(Some((1, &write))), None);
        }
    }

    /// Puts `stop` as how the request `t` this side serves stops if it is
    /// canceled (`Return::on_cancel`).
    pub fn on_cancel(&self, t: Tag, stop: Box<dyn FnOnce()>) {
        self.cancels.borrow_mut().insert(t, stop);
    }

    /// Answers the request `t`, whose method answered EINPROGRESS, with
    /// `result` and its return data, as the component model's `task.return`.
    /// A later answer is found by its tag alone.
    pub fn task_return(&self, t: Tag, result: i32, pkt: Packet) {
        self.cancels.borrow_mut().remove(&t);
        self.out(t, ROOT, 0, true, result, pkt);
    }

    /// The frame the other side laid at `at`: a later answer, made due, or a
    /// method, run by its object, its answer laid in the frame it came in.
    pub fn recv(&self, at: u32) -> i32 {
        let room = self.layout().room;
        let f = Frame::at(at);
        let pkt = f.pkt(at, room);
        if f.res {
            let a = self.pending.borrow_mut().remove(&f.tag);
            if let Some(a) = a {
                let data = pkt.ok().flatten().map(<[u8]>::to_vec);
                self.due.borrow_mut().push_back((a, f.result, data));
            }
            return 0;
        }
        let reply = match pkt.and_then(args_of) {
            Ok(args) => self.serve(f.handle, f.method, f.tag, args),
            Err(e) => Reply::from(e.errnum as i32),
        };
        // The args are read: the answer goes in the frame they came in.
        let data = reply
            .data
            .as_ref()
            .map(|(n, w)| (*n, &**w as &dyn Fn(&mut PacketWriter)));
        let words = Frame {
            res: true,
            result: reply.result,
            ..f
        };
        // An answer past the room is refused as a later one is (`out`).
        match self.lay(at, words, Packet::Args(data)) {
            Ok(()) => reply.result,
            Err(e) => {
                let refused = Frame {
                    result: e as i32,
                    ..words
                };
                self.lay(at, refused, Packet::Args(None)).or_bug(Errno::EMSGSIZE);
                e as i32
            }
        }
    }

    /// Runs the answers due, those made due while they run too.
    pub fn run_due(&self) {
        loop {
            let next = self.due.borrow_mut().pop_front();
            let Some((a, result, data)) = next else {
                return;
            };
            a(result, data);
        }
    }

    /// Whether an answer is due.
    pub fn has_due(&self) -> bool {
        !self.due.borrow().is_empty()
    }

    /// Runs `m` of `h`: the root's open or cancel, a drop, or an object's
    /// method.
    fn serve(&self, h: Handle, m: Method, tag: Tag, args: Option<BranchCursor<'_>>) -> Reply {
        match (h, m) {
            (ROOT, OPEN) => Reply::from(self.opened(args).map_or_else(|e| e as i32, |h| h as i32)),
            (ROOT, CANCEL) => {
                let t = args
                    .ok_or(MarshalError::new(Errno::EBADMSG))
                    .and_then(|b| b.at(0)?.get_number());
                match t {
                    Ok(t) => {
                        let stop = self.cancels.borrow_mut().remove(&(t as Tag));
                        if let Some(stop) = stop {
                            stop();
                        }
                        Reply::from(0)
                    }
                    Err(e) => Reply::from(e.errnum as i32),
                }
            }
            (_, DROP) => match self.objects.borrow_mut().remove(h) {
                Some(_) => Reply::from(0),
                None => Reply::from(Errno::EBADF as i32),
            },
            // A named handle no interface is mounted under is no object
            // there; an issued one no longer held is a bad handle.
            _ if h & NAMED != 0 => {
                let o = self.named.borrow().get(&h).cloned();
                o.map_or(Reply::from(Errno::ENOENT as i32), |o| o.call(m, tag, args))
            }
            _ => {
                let o = self.objects.borrow().get(h).cloned();
                match o {
                    Some(o) => o.call(m, tag, args),
                    None => Reply::from(Errno::EBADF as i32),
                }
            }
        }
    }

    /// Opens the object the args' URI names, by the opener of its longest
    /// matching mount, and issues its handle.
    fn opened(&self, args: Option<BranchCursor<'_>>) -> Result<Handle, Errno> {
        let uri = args
            .ok_or(MarshalError::new(Errno::EBADMSG))
            .and_then(|b| b.at(0)?.as_branch())
            .and_then(|b| {
                b.iter()
                    .map(|c| Token::read(c?))
                    .collect::<Result<Vec<_>, _>>()
            })
            .map_err(|e| e.errnum)?;
        let open = self
            .mounts
            .borrow()
            .iter()
            .filter(|(p, _)| uri.starts_with(p))
            .rev()
            .max_by_key(|(p, _)| p.len())
            .map(|(_, o)| o.clone())
            .ok_or(Errno::ENOENT)?;
        let h = self.objects.borrow_mut().issue();
        let o = open(&uri, h).inspect_err(|_| {
            self.objects.borrow_mut().remove(h);
        })?;
        self.objects.borrow_mut().put(h, o);
        Ok(h)
    }

    /// The next request's tag: a counter that wraps, past every tag still
    /// pending, so a late answer, which comes only for a pending tag, finds
    /// its own request.
    fn tag(&self) -> Tag {
        let p = self.pending.borrow();
        let mut t = self.next.get();
        while p.contains_key(&t) {
            t = t.wrapping_add(1);
        }
        self.next.set(t.wrapping_add(1));
        t
    }

    /// Lays a frame at `at`: its words, and the packet `pkt`, whose length
    /// it takes; EMSGSIZE, laying nothing, where the packet is past the room
    /// (link.ts `_lay`).
    fn lay(&self, at: u32, words: Frame, pkt: Packet) -> Result<(), Errno> {
        let room = self.layout().room;
        let pkt_len = match pkt {
            Packet::Bytes(b) => {
                if u64::from(WORD_BYTES) + b.len() as u64 > u64::from(room) {
                    return Err(Errno::EMSGSIZE);
                }
                // SAFETY: the frame's room past its words, which only `lay` writes.
                unsafe { mem::bytes(at + WORD_BYTES, b.len() as u32) }.copy_from_slice(b);
                b.len() as u32
            }
            Packet::Args(Some((fields, write))) if fields > 0 => {
                // SAFETY: the frame's room past its words, which only `lay` writes.
                let room = unsafe { mem::bytes(at + WORD_BYTES, room - WORD_BYTES) };
                let mut w = PacketWriter::new(room, fields);
                write(&mut w);
                w.fit_len().ok_or(Errno::EMSGSIZE)?
            }
            _ => 0,
        };
        Frame { pkt_len, ..words }.lay(at);
        Ok(())
    }

    /// Lays a frame in this side's box and crosses with it, and answers what
    /// the callee laid there: its result, and its return data. A packet past
    /// the room is refused: a packet written elsewhere, as one from across a
    /// port, is its sender's fault, answered EMSGSIZE; a later answer's is
    /// answered EMSGSIZE with no data, so its request still hears; args this
    /// side writes past it are a bug (link.ts `_out`).
    fn out(
        &self,
        tag: Tag,
        h: Handle,
        m: Method,
        res: bool,
        result: i32,
        pkt: Packet,
    ) -> (i32, Option<&'static [u8]>) {
        let l = self.layout();
        let at = (self.out_box)(l);
        let words = Frame {
            tag,
            res,
            handle: h,
            method: m,
            result,
            pkt_len: 0,
        };
        let written = matches!(pkt, Packet::Bytes(_));
        if let Err(e) = self.lay(at, words, pkt) {
            if !res {
                xpute_kit::ensure!(written, EMSGSIZE, h, m);
                return (e as i32, None);
            }
            let refused = Frame {
                result: e as i32,
                ..words
            };
            self.lay(at, refused, Packet::Args(None)).or_bug(Errno::EMSGSIZE);
        }
        let result = self.crossed(|| (self.cross)(at));
        let f = Frame::at(at);
        let data = if f.res && !res {
            f.pkt(at, l.room).ok().flatten()
        } else {
            None
        };
        (result, data)
    }
}

/// The URI of names alone, as a resource's (package, resource).
pub fn uri(names: &[&str]) -> Vec<Token> {
    names.iter().map(|n| Token::Str((*n).to_owned())).collect()
}

/// What a native guest's export answers where the guest panicked, caught at
/// the export so nothing unwinds into its caller: as a module's trap. The
/// guest is over, and its host enters it no more.
pub const TRAP: i32 = i32::MIN;

/// A method's result as its value, or the errno it refused with.
pub fn answer(result: i32) -> Result<u32, Errno> {
    match u32::try_from(result) {
        Ok(v) => Ok(v),
        Err(_) => Err(Errno::of(result).or_bug(Errno::EPROTO)),
    }
}

fn args_of(pkt: Option<&'static [u8]>) -> Result<Option<BranchCursor<'static>>, MarshalError> {
    pkt.map(|p| TreeReader::new(p)?.read_branch()).transpose()
}
