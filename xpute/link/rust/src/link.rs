// xpute-link/link.rs

//! The link (`link/wit/types.wit`). Whatever is called is an object, called
//! by its methods under a handle: an interface's named handle, the hash of its
//! name (`frame.json5` `NAMED`), or a resource's, named by a URI, a tuple of
//! tokens, and opened by the root's `open` to a handle the implementing side
//! issues, or answered by a function that returns it. A frame is one method
//! called: its tag the request, unique among the sender's pending ones, its
//! command the handle, the method and a `RES` bit (`frame.rs`), its packet the
//! args in the Canonical ABI's layout (`cabi.rs`). A frame crosses one way
//! through one function, `link_out` from the guest and `link_in` into it, and
//! each crossing returns at once what the method came to: its result, or
//! EINPROGRESS for one that answers later, by a frame marked `RES` with the
//! request's tag. So each side has the same ways:
//!
//! - `call` takes what the crossing returns, for a method that always answers
//!   at once, and stops as a bug on EINPROGRESS;
//! - `send` hands it to its answer, and an `async func` is sent;
//! - `mount_named` mounts an interface's implementation under its named
//!   handle, `add` an object under a handle issued for a function to answer,
//!   and `mount` puts the opener of an interface's resources the side
//!   implements under a URI's leading tokens, the longest matching winning;
//!   `open` and `drop` reach the root's open and a handle's drop on the other
//!   side;
//! - `cancel` cancels a request sent and pending (the root's `CANCEL`), and
//!   `on_cancel` puts what runs if a request the side answers is canceled.
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
//!   marked `RES`, with its result and its return data as the packet. A call's
//!   return data is readable until the next crossing; a send's answer gets a
//!   copy, which it owns.
//! - An answer whose result is an errno carries the rest of its error, its
//!   place and its message, as its packet (`types.wit` `error-packet`), so
//!   the caller's error is the one that was made (`store_err`, `answer`),
//!   whoever refused: an implementation, or the link itself, of a handle it
//!   does not hold or a packet past the cap. A trapped link lays no frame, so
//!   its EPIPE alone is made where it is read.

use std::cell::{Cell, RefCell};
use std::collections::{BTreeMap, VecDeque};
use std::rc::Rc;

use crate::cabi::{Lift, Lower};
use xpute_kit::abi::word::{join64, split64};
use xpute_kit::status::bug::OrBug;
use xpute_kit::status::errno::Errno;
use xpute_kit::status::error::XputeError;
use xpute_kit::status::msg::MSG_CAP;

use crate::abi::types::{ErrorPacket, Lent, Token};

use crate::frame::{Frame, HDR_SZ};
use crate::mem;
use crate::resource::Table;

pub use crate::resource::Handle;

/// What an async method lends: a buffer that says where it lies in the guest's
/// memory and how long (link/wit/types.wit `lent`), and stays put while the
/// answer owns it.
pub trait Lend: Unpin + 'static {
    fn lent(&mut self) -> Lent;
}

impl<T: Copy + Unpin + 'static> Lend for Vec<T> {
    fn lent(&mut self) -> Lent {
        mem::lent(self)
    }
}

pub type Tag = u32;
pub type Method = u32;

pub use crate::frame::spec::{CANCEL, DROP, NAMED, OPEN, OPEN_AT, ROOT, TRAP, TRAP_AT, layout};

/// An object a side implements: given a method, the request's tag, for a
/// later answer (`task_return`), its args (`args`, empty for none) and the
/// writer of its answer's value (`ret`), it answers its result, a status
/// (link.ts `LinkObject`). The reader and the writer are over the frame the
/// request came in, so a method that answers a value writes it once its args
/// are read, starting the packet (`Lower::start`), one that fails its error
/// (`store_err`), and one that writes none answers no packet.
pub trait Object {
    fn call(&self, m: Method, tag: Tag, args: Lift, ret: &mut Lower) -> i32;
}

/// How a request a side answers stops if its caller cancels it (`CANCEL`).
pub type Cancel = Box<dyn FnOnce()>;

/// What answers an async method's request once the method has its value, and
/// where the method says how it stops if its caller cancels it (`CANCEL`).
pub struct Return<T> {
    /// The answering side's `on_cancel` for the request.
    on: Box<dyn Fn(Cancel)>,
    answer: Box<dyn FnOnce(Result<T, XputeError>)>,
}

impl<T> Return<T> {
    /// The answer to a request, `on` putting how it stops on the side that
    /// answers it.
    pub fn new(
        on: Box<dyn Fn(Cancel)>,
        answer: Box<dyn FnOnce(Result<T, XputeError>)>,
    ) -> Return<T> {
        Return { on, answer }
    }

    /// Answers the request with `v`.
    pub fn answer(self, v: Result<T, XputeError>) {
        (self.answer)(v);
    }

    /// Runs `stop` if the caller cancels the request before its answer; the
    /// method then answers ECANCELED once it has stopped. A method that
    /// cannot stop says nothing, runs on and answers as it would.
    pub fn on_cancel(&self, stop: impl FnOnce() + 'static) {
        (self.on)(Box::new(stop));
    }
}

/// What opens the object a URI names, given the handle it is issued.
type Opener = dyn Fn(&[Token], Handle) -> Result<Rc<dyn Object>, XputeError>;

/// What hears a sent method's answer: its result, and a copy of its return
/// data, which it owns.
pub type Answer = Box<dyn FnOnce(i32, Option<Vec<u8>>)>;

/// A method's args: their fixed part's size, and what writes them (`Lower`).
/// None is no packet.
pub type Args<'a> = Option<(usize, &'a dyn Fn(&mut Lower))>;

/// A frame's packet: its args, written as the frame is laid, or a packet
/// written already, as one the UI sent (link.ts `Packet`).
#[derive(Clone, Copy)]
pub enum Packet<'a> {
    Args(Args<'a>),
    Bytes(&'a [u8]),
}

/// The link's layout: the guest's inbox and a frame's cap. The outbox lies
/// right past the inbox.
#[derive(Clone, Copy)]
pub struct Layout {
    pub inbox: u64,
    pub cap: u32,
}

impl Layout {
    /// The layout `init` wrote at `at` (frame.json5 `LAYOUT`).
    pub fn at(at: u64) -> Layout {
        // SAFETY: the layout's words, which `init` wrote.
        let w = |k: usize| unsafe { mem::word(at + 4 * k as u64) };
        Layout {
            inbox: join64(w(layout::INBOX_AT), w(layout::INBOX_AT + 1)),
            cap: w(layout::CAP_AT),
        }
    }

    /// Writes the layout at `at`.
    ///
    /// # Safety
    /// The layout's words at `at` are the caller's.
    pub unsafe fn lay(&self, at: u64) {
        // SAFETY: the caller's.
        let set = |k: usize, v: u32| unsafe { mem::set_word(at + 4 * k as u64, v) };
        let [lo, hi] = split64(self.inbox);
        set(layout::INBOX_AT, lo);
        set(layout::INBOX_AT + 1, hi);
        set(layout::CAP_AT, self.cap);
    }
}

/// An answer due: what hears it, its result and its return data.
type Due = (Answer, i32, Option<Vec<u8>>);

/// A later answer given while this side is crossing: its request, and its
/// result and packet, written, or the error it failed with, which holds what
/// its packet is made of.
type Completion = (Tag, Result<(i32, Option<Vec<u8>>), XputeError>);

/// An opener mounted: the named handle of the interface whose resources it
/// opens, the URI's leading tokens it opens under, and the opener.
type Prefix = (Handle, Vec<Token>, Rc<Opener>);

/// One side of the link: what both sides do, once. A side lays its frames in
/// its own box and crosses by its own function: the guest lays in its outbox
/// and crosses by `link_out`, and the host lays in the guest's inbox and
/// crosses by `link_in`. It takes the other side's frame where that side laid
/// it (`recv`), and runs the answers due when its owner says the work at hand
/// is done (`run_due`): the guest as an entry ends, the host after each piece
/// of work. A later answer this side gives while it is crossing is written
/// then and laid with the answers due, as link.ts `End` and the C++ `Side`
/// hold it.
pub struct Side {
    layout: Cell<Option<Layout>>,
    /// The box this side lays its frames in.
    out_box: fn(Layout) -> u64,
    /// Crosses with the frame laid at `at`, and answers what the method came
    /// to.
    cross: fn(u64) -> i32,
    /// Whether this side is crossing: control is on the other side.
    crossing: Cell<bool>,
    /// Where the next request's tag is sought.
    next: Cell<Tag>,
    /// The openers mounted.
    mounts: RefCell<Vec<Prefix>>,
    /// The objects this side implements, by the handles it issued.
    objects: RefCell<Table<Rc<dyn Object>>>,
    /// The interfaces this side implements, by their named handles (`NAMED`).
    named: RefCell<BTreeMap<Handle, Rc<dyn Object>>>,
    /// The answers of the sent requests that answer later, by tag.
    pending: RefCell<BTreeMap<Tag, Answer>>,
    /// How each request this side answers stops if its caller cancels it, by
    /// tag, until it answers.
    cancels: RefCell<BTreeMap<Tag, Cancel>>,
    /// The answers due, run after the work at hand.
    due: RefCell<VecDeque<Due>>,
    /// The later answers given while crossing, laid after the work at hand.
    completions: RefCell<VecDeque<Completion>>,
    /// Whether the link can run no more, whichever side failed: nothing is
    /// laid or crossed, each frame answering EPIPE (`link.ts` `_trapped`).
    trapped: Cell<bool>,
}

impl Side {
    pub const fn new(out_box: fn(Layout) -> u64, cross: fn(u64) -> i32) -> Side {
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
            completions: RefCell::new(VecDeque::new()),
            trapped: Cell::new(false),
        }
    }

    /// Stops the link for good, as `link.ts` `End.trap` does; a host's
    /// runtime tells its platform, the first time (xpute-runtime-native
    /// link.rs `trap`). Each request this side sent that is still pending is
    /// answered EPIPE, once, among the answers due (`run_due`), as a frame to
    /// the trapped link is; none is pending after, so a late answer to one,
    /// or a trap again, finds nothing to answer.
    pub fn trap(&self) {
        self.trapped.set(true);
        let pending = core::mem::take(&mut *self.pending.borrow_mut());
        self.due.borrow_mut().extend(
            pending
                .into_values()
                .map(|a| (a, Errno::EPIPE as i32, None)),
        );
    }

    /// Whether the link is trapped (`trap`).
    pub fn trapped(&self) -> bool {
        self.trapped.get()
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

    /// Puts `open` as the opener of the URIs that start with `prefix`, whose
    /// objects are the resources of the interface of the named handle `h`.
    pub fn mount(
        &self,
        h: Handle,
        prefix: Vec<Token>,
        open: impl Fn(&[Token], Handle) -> Result<Rc<dyn Object>, XputeError> + 'static,
    ) {
        self.mounts.borrow_mut().push((h, prefix, Rc::new(open)));
    }

    /// Mounts `o` under its interface's named handle `h`, which the other
    /// side calls with no open.
    pub fn mount_named(&self, h: Handle, o: Rc<dyn Object>) {
        let was = self.named.borrow_mut().insert(h, o);
        xpute_kit::ensure!(was.is_none(), EEXIST, h);
    }

    /// Whether this side implements the interface of the named handle `h`:
    /// under that handle, or by an opener of its resources.
    pub fn implements(&self, h: Handle) -> bool {
        self.named.borrow().contains_key(&h) || self.mounts.borrow().iter().any(|(i, ..)| *i == h)
    }

    /// Adds `o` under a handle issued for it, which a method answers for
    /// the other side to call `o` by: a resource a function returns.
    pub fn add(&self, o: Rc<dyn Object>) -> Handle {
        self.objects.borrow_mut().add(o)
    }

    /// Opens the object `uri` names on the other side, and answers its
    /// handle.
    pub fn open(&self, uri: &[Token]) -> Result<Handle, XputeError> {
        let write = |w: &mut Lower| store_uri(w, 0, uri);
        let (result, data) = self.call(ROOT, OPEN, Packet::Args(Some((8, &write))));
        answer(result, data)?;
        Ok(Lift::new(data.ok_or(Errno::EBADMSG)?).u32(0)?)
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
            let write = |w: &mut Lower| w.u32(0, t);
            self.send(ROOT, CANCEL, Packet::Args(Some((4, &write))), None);
        }
    }

    /// Puts `stop` as how the request `t` this side answers stops if it is
    /// canceled (`Return::on_cancel`).
    pub fn on_cancel(&self, t: Tag, stop: Cancel) {
        self.cancels.borrow_mut().insert(t, stop);
    }

    /// Answers the request `t`, whose method answered EINPROGRESS, with
    /// `result` and its return data, as the component model's `task.return`.
    /// A later answer is found by its tag alone. While this side is crossing,
    /// its packet is written now and laid after the work at hand
    /// (`run_due`); a packet past the cap is answered EMSGSIZE, as one laid
    /// at once is.
    pub fn task_return(&self, t: Tag, result: i32, pkt: Packet) {
        self.cancels.borrow_mut().remove(&t);
        if !self.crossing.get() {
            self.out(t, ROOT, 0, true, result, pkt);
            return;
        }
        let written = match pkt {
            Packet::Bytes(b) => Ok(Some(b.to_vec())),
            Packet::Args(None) => Ok(None),
            Packet::Args(Some((fixed_sz, write))) => {
                let mut buf = vec![0; (self.layout().cap - HDR_SZ) as usize];
                let mut w = Lower::new(&mut buf, fixed_sz);
                write(&mut w);
                w.finish().map(|len| {
                    buf.truncate(len);
                    Some(buf)
                })
            }
        };
        let completion = written.map(|pkt| (result, pkt)).map_err(XputeError::from);
        self.completions.borrow_mut().push_back((t, completion));
    }

    /// Answers the request `t`, whose method answered EINPROGRESS, with the
    /// error `e` it failed with, as `task_return` answers its value. While
    /// this side is crossing, the error itself waits and is laid after the
    /// work at hand, so answering one allocates nothing, as an error answered
    /// at once does not.
    pub fn task_err(&self, t: Tag, e: XputeError) {
        self.cancels.borrow_mut().remove(&t);
        if self.crossing.get() {
            self.completions.borrow_mut().push_back((t, Err(e)));
        } else {
            self.out_err(t, &e);
        }
    }

    /// Lays the later answer of the error `e` to the request `t`.
    fn out_err(&self, t: Tag, e: &XputeError) {
        let write = |w: &mut Lower| lower_err(w, e);
        let pkt = Packet::Args(Some((ErrorPacket::SZ, &write)));
        self.out(t, ROOT, 0, true, e.errnum as i32, pkt);
    }

    /// The frame the other side laid at `at`: a later answer, made due, or a
    /// method, run by its object, its answer written in the frame it came in.
    /// A request's packet past the cap is refused, EBADMSG; an answer past it
    /// EMSGSIZE (link.ts `Side.recv`).
    pub fn recv(&self, at: u64) -> i32 {
        let cap = self.layout().cap;
        let f = Frame::at(at);
        let pkt = f.pkt(at, cap);
        if f.res {
            let a = self.pending.borrow_mut().remove(&f.tag);
            if let Some(a) = a {
                let data = pkt.ok().flatten().map(<[u8]>::to_vec);
                self.due.borrow_mut().push_back((a, f.result, data));
            }
            return 0;
        }
        // The args and the answer lie in the frame's packet, read and written
        // by address, the answer once the args are read.
        let pkt_at = at + u64::from(HDR_SZ);
        // SAFETY: the frame's bytes past its words, which only this side
        // touches while it answers.
        let writer = || unsafe { Lower::from_raw_parts(mem::ptr(pkt_at), (cap - HDR_SZ) as usize) };
        let mut w = writer();
        let ran = pkt.map_err(XputeError::from).and_then(|p| {
            let len = p.map_or(0, <[u8]>::len);
            // SAFETY: as the writer's.
            let args = unsafe { Lift::from_raw_parts(mem::ptr(pkt_at), len) };
            self.run(f.handle, f.method, f.tag, args, &mut w)
        });
        let mut result = ran.unwrap_or_else(|e| store_err(&mut w, &e));
        // An answer past the cap is refused as a later one is (`out`), its
        // error laid in its place.
        let pkt_len = w.finish().unwrap_or_else(|e| {
            let mut w = writer();
            result = store_err(&mut w, &e.into());
            w.finish().or_bug(Errno::EMSGSIZE)
        });
        let words = Frame {
            res: true,
            result,
            pkt_len: pkt_len as u32,
            ..f
        };
        words.lay(at);
        words.result
    }

    /// Lays the later answers given while crossing, then runs the answers
    /// due, those made due while they run too.
    pub fn run_due(&self) {
        loop {
            let next = self.completions.borrow_mut().pop_front();
            match next {
                Some((t, Ok((result, pkt)))) => {
                    let pkt = pkt.as_deref().map_or(Packet::Args(None), Packet::Bytes);
                    self.out(t, ROOT, 0, true, result, pkt);
                }
                Some((t, Err(e))) => self.out_err(t, &e),
                None => break,
            }
        }
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
        !self.due.borrow().is_empty() || !self.completions.borrow().is_empty()
    }

    /// Runs `m` of `h`: the root's open or cancel, a drop, or an object's
    /// method (`Object::call`), which answers its status, its own error laid
    /// where it failed; or the error the link itself refuses the call with,
    /// made here.
    fn run(
        &self,
        h: Handle,
        m: Method,
        tag: Tag,
        args: Lift,
        ret: &mut Lower,
    ) -> Result<i32, XputeError> {
        match (h, m) {
            (ROOT, OPEN) => {
                let h = self.opened(args)?;
                ret.start(4);
                ret.u32(0, h);
                Ok(0)
            }
            (ROOT, CANCEL) => {
                let t = args.u32(0)?;
                let stop = self.cancels.borrow_mut().remove(&t);
                if let Some(stop) = stop {
                    stop();
                }
                Ok(0)
            }
            (_, DROP) => {
                let o = self.objects.borrow_mut().remove(h);
                o.map(|_| 0).ok_or(Errno::EBADF).map_err(XputeError::from)
            }
            // A named handle no interface is mounted under is no object
            // there; an issued one no longer held is a bad handle.
            _ if h & NAMED != 0 => {
                let o = self.named.borrow().get(&h).cloned();
                Ok(o.ok_or(Errno::ENOENT)?.call(m, tag, args, ret))
            }
            _ => {
                let o = self.objects.borrow().get(h).cloned();
                Ok(o.ok_or(Errno::EBADF)?.call(m, tag, args, ret))
            }
        }
    }

    /// Opens the object the args' URI names, by the opener of its longest
    /// matching mount, and issues its handle.
    fn opened(&self, args: Lift) -> Result<Handle, XputeError> {
        let uri = load_uri(args, 0)?;
        let open = self
            .mounts
            .borrow()
            .iter()
            .filter(|(_, p, _)| uri.starts_with(p))
            .rev()
            .max_by_key(|(_, p, _)| p.len())
            .map(|(.., o)| o.clone())
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
    /// it takes; EMSGSIZE, laying nothing, where the packet is past the cap
    /// (link.ts `Side._lay`).
    fn lay(&self, at: u64, words: Frame, pkt: Packet) -> Result<(), Errno> {
        let cap = self.layout().cap;
        let pkt_len = match pkt {
            Packet::Bytes(b) => {
                if u64::from(HDR_SZ) + b.len() as u64 > u64::from(cap) {
                    return Err(Errno::EMSGSIZE);
                }
                // SAFETY: the frame's bytes past its words, which only `lay` writes.
                unsafe { mem::bytes(at + u64::from(HDR_SZ), b.len() as u32) }.copy_from_slice(b);
                b.len() as u32
            }
            Packet::Args(Some((fixed_sz, write))) => {
                // SAFETY: the frame's bytes past its words, which only `lay` writes.
                let buf = unsafe { mem::bytes(at + u64::from(HDR_SZ), cap - HDR_SZ) };
                let mut w = Lower::new(buf, fixed_sz);
                write(&mut w);
                w.finish()? as u32
            }
            Packet::Args(None) => 0,
        };
        Frame { pkt_len, ..words }.lay(at);
        Ok(())
    }

    /// Lays a frame in this side's box and crosses with it, and answers what
    /// the callee laid there: its result, and its return data. A packet past
    /// the cap is refused: a packet written elsewhere, as one a runtime
    /// carried from another thread, is its sender's fault, answered
    /// EMSGSIZE here, the error laid in the box with nothing crossed; a later
    /// answer's is answered EMSGSIZE, so its request still hears; args this
    /// side writes past it are a bug (link.ts `Side._out`). A trapped link
    /// lays nothing, so its EPIPE is the one answer with no error laid.
    fn out(
        &self,
        tag: Tag,
        h: Handle,
        m: Method,
        res: bool,
        result: i32,
        pkt: Packet,
    ) -> (i32, Option<&'static [u8]>) {
        // A trapped link lays nothing, its layout perhaps never read.
        if self.trapped.get() {
            return (Errno::EPIPE as i32, None);
        }
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
            xpute_kit::ensure!(res || written, EMSGSIZE, h, m);
            let e = XputeError::from(e);
            let refused = Frame {
                res: true,
                result: e.errnum as i32,
                ..words
            };
            let write = |w: &mut Lower| lower_err(w, &e);
            self.lay(at, refused, Packet::Args(Some((ErrorPacket::SZ, &write))))
                .or_bug(Errno::EMSGSIZE);
            if !res {
                return (refused.result, Frame::at(at).pkt(at, l.cap).ok().flatten());
            }
        }
        let result = self.crossed(|| (self.cross)(at));
        let f = Frame::at(at);
        let data = if f.res && !res {
            f.pkt(at, l.cap).ok().flatten()
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

/// Writes `uri` at `at` as a `list<token>`, as `ROOT` `OPEN`'s args hold it
/// (`types.wit`).
pub fn store_uri(w: &mut Lower, at: usize, uri: &[Token]) {
    if let Some(base) = w.list(at, uri.len(), Token::SZ, Token::ALIGN) {
        uri.iter()
            .enumerate()
            .for_each(|(k, t)| t.store(w, base + k * Token::SZ));
    }
}

/// The URI the `list<token>` at `at` holds (`store_uri`).
pub fn load_uri(r: Lift, at: usize) -> Result<Vec<Token>, Errno> {
    let (base, len) = r.list(at, Token::SZ, Token::ALIGN)?;
    (0..len)
        .map(|k| Token::load(r, base + k * Token::SZ))
        .collect()
}

/// The most bytes an error's answer holds as its packet: its fixed part, and
/// its file and its message, each as far as a message's cap. A frame's cap
/// holds one (xpute-guest link.rs `init`), so no error's answer is past it.
pub const ERR_PKT_CAP: usize = ErrorPacket::SZ + 2 * MSG_CAP as usize;

/// Writes the fixed part of the error `e`'s packet from 0 on, and its file
/// and message past it.
fn lower_err(w: &mut Lower, e: &XputeError) {
    ErrorPacket::store_of(w, 0, &e.file, e.line, e.msg.as_deref());
}

/// Writes the error `e` as an answer at once: the rest of it as the packet
/// (`types.wit` `error-packet`). It answers the answer's result, the error's
/// errno.
pub fn store_err(w: &mut Lower, e: &XputeError) -> i32 {
    w.start(ErrorPacket::SZ);
    lower_err(w, e);
    e.errnum as i32
}

/// A method's result, its status, with its return data `data`: answered, its
/// value, if it has one, in its return data; or the error it failed with (a
/// refusal, or a method's error where its WIT has no `result`), the one that
/// was made, with the place and the message the data holds (`store_err`).
/// Data that holds no error's packet is EBADMSG. A trapped link's EPIPE
/// alone holds no data, no frame having crossed, and its error is made here.
pub fn answer(result: i32, data: Option<&[u8]>) -> Result<(), XputeError> {
    if result == 0 {
        return Ok(());
    }
    let errnum = Errno::of(result).or_bug(Errno::EPROTO);
    let Some(data) = data else {
        return Err(XputeError::new(errnum));
    };
    let pkt = ErrorPacket::load(Lift::new(data), 0)?;
    Err(XputeError::at(errnum, pkt.file, pkt.line, pkt.msg))
}

#[cfg(test)]
#[path = "link.test.rs"]
mod test;
