// xpute-runtime-native/remote.rs

//! The guest's thread's half of the UI's crossing, as @xpute/runtime-web
//! remote.ts is a page's: the runtime stands in for the UI on the JavaScript
//! thread on the guest's link, and the UI's half is @xpute/expo ui.ts,
//! over the C ABI. The UI's methods come through the queue (lib.rs `post`),
//! and what the UI hears is batched and handed over on one notify (`take`),
//! as a page's port posts.
//!
//! What the UI serves is mounted before the guest boots: its named
//! interfaces under their handles, and an opener at each prefix it serves
//! (`serve`). Every method of them is an `async func`, which the runtime
//! holds as it starts (link.rs `init`): the UI answers each later (lib.rs
//! `answer`), and a cancel of it is told to the UI, which answers ECANCELED
//! once it has stopped. The guest's end issues the handle of an object the
//! UI serves as the guest opens it, by the opener of the prefix that
//! matched, and the UI serves it under that handle.
//!
//! A batch is messages, each an XTP packet laid after its length (a u32,
//! then four bytes so the packet starts on a word), its first field its
//! kind:
//!
//! - 0 `(tag, result, data)`: the answer to the UI's request `tag`;
//! - 2 `(h, m, tag, pkt)`: a method of an object the UI serves, which the UI
//!   answers by `tag`;
//! - 3 `(h, k, uri)`: open the object at `uri`, a lowered `list<token>`,
//!   under `h`, by the UI's opener of its `k`th prefix;
//! - 4 `(tag)`: the guest canceled the request `tag`;
//! - 5 `(h)`: the object `h` the UI opened is dropped,
//!
//! where data, pkt and uri are a packet's bytes or nil.
//!
//! The UI's connection is a handle: each start attaches a new UI on the same
//! runtime, as a reload's (`accept`), which serves what the first did, and
//! answers its handle, which each of its posts, answers and takes names. One
//! UI is attached at a time; a new one drops the last, and with it, on the
//! guest's thread, what the last owned under its handle (`Ui`): the requests
//! it owed are answered ECANCELED, and the guest's objects it opened dropped
//! on the guest, by the link's own drop, so the guest's load starts again;
//! each object it serves the guest answers ECANCELED until the guest drops
//! it; then the guest is told the new UI attached (wit/life/life.wit
//! `attached`). Nothing an old UI sent that the guest's thread has not run
//! when a new one attaches reaches the guest, its methods and its answers
//! alike, as an old JavaScript runtime may still send while a reload makes
//! the new one; the UI's tags cross as they are.

use std::cell::RefCell;
use std::collections::BTreeSet;
use std::rc::{Rc, Weak};
use std::sync::{Mutex, MutexGuard};

use xpute_kit::status::bug::OrBug;
use xpute_kit::status::errno::Errno;
use xpute_kit::wire::xtp::{MAX_PKT_SZ, PacketWriter};
use xpute_link::abi::types::Token;
use xpute_link::frame::HDR_SZ;
use xpute_link::resource::Table;
use xpute_link::{
    DROP, Handle, Lift, Lower, Method, OPEN, Object, Packet, ROOT, Reply, Tag, store_uri,
};

use crate::event_loop::post_task;
use crate::life;
use crate::link::side;
use crate::listener;

/// What the runtime sends the UI: the batch it has not taken, whether it was
/// told of it, the handle of the UI attached, 0 before any, and the table
/// that issues the UIs' handles.
struct Out {
    batch: Vec<u8>,
    told: bool,
    ui: Handle,
    uis: Table<()>,
}

static OUT: Mutex<Out> = Mutex::new(Out {
    batch: Vec::new(),
    told: false,
    ui: 0,
    uis: Table::new(),
});

fn out() -> MutexGuard<'static, Out> {
    OUT.lock().unwrap_or_else(|e| e.into_inner())
}

/// What a UI owns under its handle, on the guest's thread: the requests it
/// answers later, until it answers each, and the guest's objects it opened,
/// until it drops each. Its drop, as a new UI attaches, answers each request
/// ECANCELED and drops each object on the guest.
struct Ui {
    h: Handle,
    pending: RefCell<BTreeSet<Tag>>,
    opened: RefCell<BTreeSet<Handle>>,
}

impl Drop for Ui {
    fn drop(&mut self) {
        for t in self.pending.take() {
            side().task_return(t, Errno::ECANCELED as i32, Packet::Args(None));
        }
        for o in self.opened.take() {
            side().drop(o);
        }
    }
}

thread_local! {
    /// The UI attached, as the guest's thread knows it (`attach`).
    static UI: RefCell<Option<Rc<Ui>>> = const { RefCell::new(None) };
}

/// The UI attached, as the guest's thread knows it.
fn ui() -> Option<Rc<Ui>> {
    UI.with_borrow(Clone::clone)
}

/// Makes the UI the guest's thread knows the one attached now, dropping the
/// last (`Ui`). On the guest's thread, outside a crossing: before the guest's
/// first entry, and as a task.
pub(crate) fn attach() {
    let h = out().ui;
    if ui().is_some_and(|u| u.h == h) {
        return;
    }
    let last = UI.replace(Some(Rc::new(Ui {
        h,
        pending: RefCell::default(),
        opened: RefCell::default(),
    })));
    drop(last);
}

/// Attaches a new UI to the runtime, as a reload's: the last UI's handle is
/// dropped and the batch it did not take goes, and on the guest's thread
/// what it owned is dropped (`attach`), then the guest is told the new UI
/// attached (life.rs `attached`). Called on the JavaScript thread, by each
/// start; answers the new UI's handle.
pub fn accept() -> Handle {
    let h = {
        let mut out = out();
        let last = out.ui;
        out.uis.remove(last);
        out.ui = out.uis.issue();
        out.batch.clear();
        out.told = false;
        out.ui
    };
    post_task(Box::new(attach));
    post_task(Box::new(life::attached));
    h
}

/// Appends to the batch one message of `cnt` fields that `write` writes,
/// where the UI of the handle `ui` is attached.
fn tell(ui: Handle, cnt: u32, write: impl Fn(&mut PacketWriter)) {
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
    if out.ui != ui {
        return;
    }
    out.batch
        .extend_from_slice(&(pkt.len() as u32).to_le_bytes());
    out.batch.extend_from_slice(&[0; 4]);
    out.batch.extend_from_slice(&pkt);
    let end = out.batch.len().next_multiple_of(8);
    out.batch.resize(end, 0);
}

/// Sends `ui` the method `m` of its object `h`, which it answers later by
/// `tag`, and tells it the guest's cancel of it.
fn forward(ui: &Ui, h: Handle, m: Method, tag: Tag, args: Option<&[u8]>) -> Reply {
    let u = ui.h;
    tell(u, 5, |w| {
        w.u32(Some(2))
            .u32(Some(h))
            .u32(Some(m))
            .u32(Some(tag))
            .array::<u8>(args);
    });
    ui.pending.borrow_mut().insert(tag);
    side().on_cancel(
        tag,
        Box::new(move || {
            tell(u, 2, |w| {
                w.u32(Some(4)).u32(Some(tag));
            });
        }),
    );
    Reply::from(Errno::EINPROGRESS as i32)
}

/// What stands for one of the UI's named interfaces on the link: each method
/// is sent to the UI attached.
struct Named {
    h: Handle,
}

impl Object for Named {
    fn call(&self, m: Method, tag: Tag, args: Option<&[u8]>) -> Reply {
        let ui = ui().or_bug(Errno::ENOTCONN);
        forward(&ui, self.h, m, tag, args)
    }
}

/// What stands for an object a UI opened under `h`, owned under that UI's
/// handle: each method is sent to it, and its drop is told; once the UI is
/// dropped, each method answers ECANCELED, as the UI's drop answers what it
/// owed, and its drop is told to none.
struct Opened {
    h: Handle,
    ui: Weak<Ui>,
}

impl Object for Opened {
    fn call(&self, m: Method, tag: Tag, args: Option<&[u8]>) -> Reply {
        match self.ui.upgrade() {
            Some(ui) => forward(&ui, self.h, m, tag, args),
            None => Reply::from(Errno::ECANCELED as i32),
        }
    }
}

impl Drop for Opened {
    fn drop(&mut self) {
        let h = self.h;
        if let Some(ui) = self.ui.upgrade() {
            tell(ui.h, 2, |w| {
                w.u32(Some(5)).u32(Some(h));
            });
        }
    }
}

/// `uri` as a lowered `list<token>`, as ROOT OPEN's args are.
fn lowered(uri: &[Token]) -> Vec<u8> {
    let mut buf = vec![0; (side().layout().cap - HDR_SZ) as usize];
    let mut w = Lower::new(&mut buf, 8);
    store_uri(&mut w, 0, uri);
    let len = w.finish().ok().or_bug(Errno::EMSGSIZE);
    buf.truncate(len);
    buf
}

/// Mounts what the UI serves: its named interfaces under their handles, and
/// an opener at each of its `prefixes`, in its order, for its interface, and
/// attaches the first UI before the
/// guest's first entry may call them (`attach`).
pub(crate) fn serve(named: &[Handle], prefixes: Vec<(Handle, Vec<Token>)>) {
    attach();
    for &h in named {
        side().mount_named(h, Rc::new(Named { h }));
    }
    for (k, (i, p)) in (0..).zip(prefixes) {
        side().mount(i, p, move |uri, h| {
            let ui = ui().or_bug(Errno::ENOTCONN);
            let uri = lowered(uri);
            tell(ui.h, 4, |w| {
                w.u32(Some(3))
                    .u32(Some(h))
                    .u32(Some(k))
                    .array::<u8>(Some(&uri));
            });
            Ok(Rc::new(Opened {
                h,
                ui: Rc::downgrade(&ui),
            }) as Rc<dyn Object>)
        });
    }
}

/// Sends the method of the UI of the handle `u` on the link, its answer told
/// back to that UI alone, as @xpute/runtime-web remote.ts `attach` posts a
/// page's; a packet past the cap is refused whole, EMSGSIZE (xpute-link
/// link.rs `Side::send`). A method of a UI that is not the one attached
/// reaches nothing. The guest's object an open answers is owned under that
/// UI until it drops it, or dropped at once where the UI is dropped since.
pub(crate) fn recv(u: Handle, tag: Tag, h: u32, m: u32, pkt: Option<Vec<u8>>) {
    let Some(ui) = ui().filter(|ui| ui.h == u) else {
        return;
    };
    if m == DROP {
        ui.opened.borrow_mut().remove(&h);
    }
    let opens = h == ROOT && m == OPEN;
    let owner = Rc::downgrade(&ui);
    let answer = move |result: i32, data: Option<Vec<u8>>| {
        let opened = data
            .as_deref()
            .filter(|_| opens && result == 0)
            .and_then(|d| Lift::new(d).u32(0).ok());
        if let Some(o) = opened {
            match owner.upgrade() {
                Some(ui) => _ = ui.opened.borrow_mut().insert(o),
                None => side().drop(o),
            }
        }
        tell(u, 4, |w| {
            w.u32(Some(0))
                .u32(Some(tag))
                .i32(Some(result))
                .array::<u8>(data.as_deref());
        });
    };
    let pkt = pkt.as_deref().map_or(Packet::Args(None), Packet::Bytes);
    side().send(h, m, pkt, Some(Box::new(answer)));
}

/// The answer of the UI of the handle `u` to the request `tag` it serves,
/// refused, as `recv` refuses a method, where that UI is not the one
/// attached, and where it does not owe the request; a dropped UI's requests
/// its drop answers.
pub(crate) fn answer(u: Handle, tag: Tag, result: i32, data: Option<Vec<u8>>) {
    let Some(ui) = ui().filter(|ui| ui.h == u) else {
        return;
    };
    if ui.pending.borrow_mut().remove(&tag) {
        let pkt = data.as_deref().map_or(Packet::Args(None), Packet::Bytes);
        side().task_return(tag, result, pkt);
    }
}

/// Tells the UI once that a batch waits, after the work at hand (listener.rs
/// `notify`).
pub(crate) fn flush() {
    let tell = {
        let mut out = out();
        let tell = !out.batch.is_empty() && !out.told;
        out.told |= tell;
        tell
    };
    if tell {
        listener::notify();
    }
}

/// Hands over the batch the UI of the handle `u` has not taken, or none
/// where that UI is not the one attached. Called on the JavaScript thread,
/// on the notify.
pub fn take(u: Handle) -> Vec<u8> {
    let mut out = out();
    if out.ui != u {
        return Vec::new();
    }
    out.told = false;
    core::mem::take(&mut out.batch)
}
