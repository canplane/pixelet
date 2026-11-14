// xpute-native-runtime/remote.rs

//! The link's side of a UI on the JavaScript thread, as @xpute/web-runtime
//! remote.ts `serve` is a page's, and the link's serving half for what the UI
//! serves across it. The UI's methods come through the queue (lib.rs `post`),
//! and what the UI hears is batched and handed over on one notify (`take`), as
//! a page's port posts.
//!
//! What the UI serves is mounted before the guest boots: its named
//! interfaces under their handles, and an opener at each prefix it serves
//! (`serve`). A method of a named interface is told to the UI and answered 0
//! at once, unless WIT marks it `async func` (`LATER`, which the generated
//! bindings give); that one, and every method of an object the UI opened,
//! the UI answers later (lib.rs `answer`), and a cancel of it is told to the
//! UI, which answers ECANCELED once it has stopped.
//!
//! A batch is messages, each an XTP packet laid after its length (a u32,
//! then four bytes so the packet starts on a word), its first field its
//! kind:
//!
//! - 0 `(tag, result, data)`: the answer to the UI's request `tag`;
//! - 2 `(h, m, later, tag, pkt)`: a method of an object the UI serves, which
//!   the UI answers by `tag` where `later` is 1;
//! - 3 `(h, uri)`: open the object at `uri`, a lowered `list<token>`, under
//!   `h`;
//! - 4 `(tag)`: the guest canceled the request `tag`;
//! - 5 `(h)`: the object `h` the UI opened is dropped,
//!
//! where data, pkt and uri are a packet's bytes or nil.
//!
//! Each start is a new UI on the same runtime, as a reload's (`accept`),
//! which serves what the first did: the UI's generation counts them. A
//! request the UI sent and a request it answers later carry the generation
//! of the UI they crossed with, so an old UI's answer is told to no new UI
//! and a new UI's answer to an old UI's request is refused; what an old UI
//! held answers ECANCELED, the requests it left pending and each method of
//! the objects it serves the guest until the guest drops them, so the
//! guest's load starts again; and the guest's objects it opened are dropped
//! on the guest, so none is held for a UI that is gone; then the guest is
//! told the new UI attached (wit/life/life.wit `attached`). A start answers the
//! UI its generation, which each of its posts, answers and takes carries, so
//! nothing of an old UI's that the guest's thread has not run when a new one
//! is taken reaches the guest, its methods and its answers alike, as an old
//! JavaScript runtime may still send while a reload makes the new one; the
//! UI's tags cross as they are.

use std::cell::RefCell;
use std::collections::{BTreeMap, BTreeSet};
use std::rc::Rc;
use std::sync::{Mutex, MutexGuard};

use xpute_kit::status::bug::OrBug;
use xpute_kit::status::errno::Errno;
use xpute_kit::wire::xtp::{PacketWriter, MAX_PKT_SZ};
use xpute_link::abi::types::Token;
use xpute_link::{store_uri, Handle, Lift, Lower, Method, Object, Packet, Reply, Tag, DROP, OPEN, ROOT};

use crate::event_loop::post_task;
use crate::life;
use crate::link;

/// What the runtime sends the UI: the batch it has not taken, whether it was
/// told of it, and the UI's generation, which each `accept` counts.
struct Out {
    batch: Vec<u8>,
    told: bool,
    generation: u32,
}

static OUT: Mutex<Out> = Mutex::new(Out {
    batch: Vec::new(),
    told: false,
    generation: 0,
});

thread_local! {
    /// The requests the UI answers later, by tag, each with the generation
    /// of the UI it was told to, until it is answered.
    static PENDING: RefCell<BTreeMap<Tag, u32>> = const { RefCell::new(BTreeMap::new()) };
    /// The guest's objects the UI opened, until it drops them; each is the
    /// UI's now, since `hang_up` drops them all before a new UI's first post.
    static GUEST_OBJECTS: RefCell<BTreeSet<Handle>> = const { RefCell::new(BTreeSet::new()) };
}

fn out() -> MutexGuard<'static, Out> {
    OUT.lock().unwrap_or_else(|e| e.into_inner())
}

/// The UI's generation now.
fn generation() -> u32 {
    out().generation
}

/// Takes a new UI on the runtime, as a reload's: the batch the last UI did
/// not take goes, the new UI is told its first, and on the guest's thread
/// the requests the last left pending are answered ECANCELED and the guest's
/// objects it opened dropped (`hang_up`), then the guest is told the new UI
/// attached (life.rs `attached`).
/// Called on the JavaScript thread, by each start; answers the new UI's
/// generation.
pub fn accept() -> u32 {
    let generation = {
        let mut out = out();
        out.generation = out.generation.wrapping_add(1);
        out.batch.clear();
        out.told = false;
        out.generation
    };
    post_task(Box::new(hang_up));
    post_task(Box::new(life::attached));
    generation
}

/// Answers ECANCELED each request pending on a UI before this one, and
/// drops on the guest each of its objects a UI before this one opened.
fn hang_up() {
    let g = generation();
    let old: Vec<Tag> = PENDING.with_borrow_mut(|l| l.extract_if(.., |_, ui| *ui != g).map(|(t, _)| t).collect());
    for t in old {
        link::task_return_pkt(t, Errno::ECANCELED as i32, None);
    }
    for h in GUEST_OBJECTS.take() {
        link::drop(h);
    }
}

/// Appends to the batch one message of `cnt` fields that `write` writes,
/// where `generation` is the UI's.
fn tell(generation: u32, cnt: u32, write: impl Fn(&mut PacketWriter)) {
    let mut cap = 1 << 10;
    let pkt = loop {
        let mut buf = vec![0u8; cap];
        let mut w = PacketWriter::new(&mut buf, cnt);
        write(&mut w);
        if let Some(len) = w.fit_len() {
            buf.truncate(len as usize);
            break buf;
        }
        xpute_kit::ensure!(cap < MAX_PKT_SZ as usize, EOVERFLOW);
        cap = (cap * 2).min(MAX_PKT_SZ as usize);
    };
    let mut out = out();
    if out.generation != generation {
        return;
    }
    out.batch.extend_from_slice(&(pkt.len() as u32).to_le_bytes());
    out.batch.extend_from_slice(&[0; 4]);
    out.batch.extend_from_slice(&pkt);
    let end = out.batch.len().next_multiple_of(8);
    out.batch.resize(end, 0);
}

/// Tells the UI of `generation` the method `m` of its object `h`, to answer
/// by `tag` where `later`.
fn tell_method(generation: u32, h: Handle, m: Method, later: bool, tag: Tag, args: Option<&[u8]>) {
    tell(generation, 6, |w| {
        w.u32(Some(2))
            .u32(Some(h))
            .u32(Some(m))
            .u32(Some(u32::from(later)))
            .u32(Some(tag))
            .array::<u8>(args);
    });
}

/// Tells the UI of `generation` a method it answers later, and the guest's
/// cancel of it.
fn forward(generation: u32, h: Handle, m: Method, tag: Tag, args: Option<&[u8]>) -> Reply {
    tell_method(generation, h, m, true, tag, args);
    PENDING.with_borrow_mut(|l| l.insert(tag, generation));
    link::on_cancel(
        tag,
        Box::new(move || {
            tell(generation, 2, |w| {
                w.u32(Some(4)).u32(Some(tag));
            });
        }),
    );
    Reply::from(Errno::EINPROGRESS as i32)
}

/// What stands for one of the UI's named interfaces on the link: a method
/// `LATER` names is answered later, any other told and answered 0 at once.
struct Named {
    h: Handle,
    later: &'static [(Handle, Method)],
}

impl Object for Named {
    fn call(&self, m: Method, tag: Tag, args: Option<&[u8]>) -> Reply {
        let g = generation();
        if self.later.contains(&(self.h, m)) {
            return forward(g, self.h, m, tag, args);
        }
        tell_method(g, self.h, m, false, tag, args);
        Reply::from(0)
    }
}

/// What stands for an object the UI of `generation` opened under `h`: each
/// method is answered later, and its drop is told; once a new UI is taken,
/// each method answers ECANCELED, as `hang_up` answers what it left
/// pending, and its drop is told to none.
struct Opened {
    h: Handle,
    generation: u32,
}

impl Object for Opened {
    fn call(&self, m: Method, tag: Tag, args: Option<&[u8]>) -> Reply {
        if self.generation != generation() {
            return Reply::from(Errno::ECANCELED as i32);
        }
        forward(self.generation, self.h, m, tag, args)
    }
}

impl Drop for Opened {
    fn drop(&mut self) {
        let h = self.h;
        tell(self.generation, 2, |w| {
            w.u32(Some(5)).u32(Some(h));
        });
    }
}

/// `uri` as a lowered `list<token>`, as ROOT OPEN's args are.
fn lowered(uri: &[Token]) -> Vec<u8> {
    let mut buf = vec![0; link::pkt_cap() as usize];
    let mut w = Lower::new(&mut buf, 8);
    store_uri(&mut w, 0, uri);
    let len = w.finish().ok().or_bug(Errno::EMSGSIZE);
    buf.truncate(len);
    buf
}

/// Mounts what the UI serves: its named interfaces under their handles
/// (@xpute/link remote.ts `Remote.named`), the methods `later` names answered
/// later, and an opener at each of its `prefixes`, for its interface
/// (`Remote.prefixes`).
pub(crate) fn serve(named: &[Handle], prefixes: Vec<(Handle, Vec<Token>)>, later: &'static [(Handle, Method)]) {
    for &h in named {
        link::mount_named(h, Rc::new(Named { h, later }));
    }
    for (i, p) in prefixes {
        link::mount(i, p, |uri, h| {
            let uri = lowered(uri);
            let generation = generation();
            tell(generation, 3, |w| {
                w.u32(Some(3)).u32(Some(h)).array::<u8>(Some(&uri));
            });
            Ok(Rc::new(Opened { h, generation }) as Rc<dyn Object>)
        });
    }
}

/// Sends the method of the UI of `generation` on the link, its answer told
/// back to that UI alone, as @xpute/web-runtime remote.ts `serve` tells a
/// page's; a packet past the room is refused whole, EMSGSIZE (xpute-link
/// link.rs `Side::send`). A method of a UI that is not the one now reaches
/// nothing. The guest's object an open answers is held for that UI until it
/// drops it, or dropped at once where a new UI has been taken since
/// (`hang_up`).
pub(crate) fn recv(generation: u32, tag: Tag, h: u32, m: u32, pkt: Option<Vec<u8>>) {
    if generation != self::generation() {
        return;
    }
    if m == DROP {
        GUEST_OBJECTS.with_borrow_mut(|l| l.remove(&h));
    }
    let opens = h == ROOT && m == OPEN;
    let answer = move |result: i32, data: Option<Vec<u8>>| {
        let opened = data.as_deref().filter(|_| opens && result == 0).and_then(|d| Lift::new(d).u32(0).ok());
        if let Some(o) = opened {
            if generation == self::generation() {
                GUEST_OBJECTS.with_borrow_mut(|l| l.insert(o));
            } else {
                link::drop(o);
            }
        }
        tell(generation, 4, |w| {
            w.u32(Some(0))
                .u32(Some(tag))
                .i32(Some(result))
                .array::<u8>(data.as_deref());
        });
    };
    let pkt = pkt.as_deref().map_or(Packet::Args(None), Packet::Bytes);
    link::send_pkt(h, m, pkt, Some(Box::new(answer)));
}

/// The answer of the UI of `generation` to the request `tag` it serves
/// later, refused, as `recv` refuses a method, where that UI is not the one
/// now, and where it was not told the request; `hang_up` answers an old UI's
/// requests itself.
pub(crate) fn answer(generation: u32, tag: Tag, result: i32, data: Option<Vec<u8>>) {
    if generation != self::generation() {
        return;
    }
    let pending = PENDING.with_borrow_mut(|l| l.get(&tag) == Some(&generation) && l.remove(&tag).is_some());
    if pending {
        link::task_return_pkt(tag, result, data);
    }
}

/// Tells the UI once that a batch waits, after the work at hand.
pub(crate) fn flush(notify: fn()) {
    let tell = {
        let mut out = out();
        let tell = !out.batch.is_empty() && !out.told;
        out.told |= tell;
        tell
    };
    if tell {
        notify();
    }
}

/// Hands over the batch the UI of `generation` has not taken, or none where
/// that UI is not the one now. Called on the JavaScript thread, on the
/// notify.
pub fn take(generation: u32) -> Vec<u8> {
    let mut out = out();
    if out.generation != generation {
        return Vec::new();
    }
    out.told = false;
    core::mem::take(&mut out.batch)
}
