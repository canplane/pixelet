// xpute-runtime-native/remote.rs

//! The guest's thread's half of the UI's crossing, as @xpute/runtime-web
//! remote.ts is the web's: the runtime stands in for the UI on the JavaScript
//! thread on the guest's link, and the UI's half is @xpute/expo ui.ts,
//! over the C ABI. The UI's methods come through the queue (lib.rs `post`),
//! and what the UI hears is batched and handed over on one notify (`take`),
//! as the web's port posts.
//!
//! What the UI implements is mounted before the guest boots: its named
//! interfaces under their handles, and an opener at each prefix it implements
//! (`mount`). Every method of them is an `async func`, which the runtime
//! holds as it starts (link.rs `init`): the UI answers each later (lib.rs
//! `answer`), and a cancel of it is told to the UI, which answers ECANCELED
//! once it has stopped. The guest's end issues the handle of an object the UI
//! implements as the guest opens it, by the opener of the prefix that
//! matched, and the UI holds it under that handle.
//!
//! A batch is frames, each the link's own (link/spec/frame.json5): its four
//! words, then its packet, padded so the next starts on a word of eight
//! bytes:
//!
//! - a method of an object the UI implements, which the UI answers by its
//!   tag;
//! - the answer to the UI's request, marked `RES`, with its tag;
//! - the root's `OPEN_AT`: open the object at a URI under the handle the
//!   guest's end issued, by the UI's opener of its `k`th prefix;
//! - the root's `CANCEL`: the guest canceled the request of that tag;
//! - a handle's `DROP`: the object the UI opened is dropped.
//!
//! The UI's connection is a handle: each start attaches a new UI on the same
//! runtime, as a reload's (`accept`), which implements what the first did,
//! and answers its handle, which each of its posts, answers and takes names.
//! One UI is attached at a time; a new one drops the last, and with it, on
//! the guest's thread, what the last owned under its handle (`Ui`): the
//! requests it owed are answered ECANCELED, and the guest's objects it opened
//! dropped on the guest, by the link's own drop, so the guest's load starts
//! again; each object it implements for the guest answers ECANCELED until the
//! guest drops it; then the guest is told the new UI attached
//! (wit/life/life.wit `attached`). Nothing an old UI sent that the guest's
//! thread has not run when a new one attaches reaches the guest, its methods
//! and its answers alike, as an old JavaScript runtime may still send while a
//! reload makes the new one; the UI's tags cross as they are.

use std::cell::RefCell;
use std::collections::BTreeSet;
use std::rc::{Rc, Weak};
use std::sync::{Mutex, MutexGuard};

use xpute_kit::status::bug::OrBug;
use xpute_kit::status::errno::Errno;
use xpute_link::abi::types::Token;
use xpute_link::frame::{Frame, HDR_SZ};
use xpute_link::resource::Table;
use xpute_link::{
    CANCEL, DROP, Handle, Lift, Lower, Method, OPEN, OPEN_AT, Object, Packet, ROOT, Tag, store_uri,
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

/// The frame of the request `tag` for the method `m` of `h`.
fn request(tag: Tag, h: Handle, m: Method) -> Frame {
    Frame {
        tag,
        res: false,
        handle: h,
        method: m,
        result: 0,
        pkt_len: 0,
    }
}

/// Appends to the batch the frame `f` with its packet `pkt`, where the UI of
/// the handle `ui` is attached.
fn tell(ui: Handle, f: Frame, pkt: Option<&[u8]>) {
    let pkt = pkt.unwrap_or_default();
    let f = Frame {
        pkt_len: pkt.len() as u32,
        ..f
    };
    let mut out = out();
    if out.ui != ui {
        return;
    }
    for w in f.words() {
        out.batch.extend_from_slice(&w.to_le_bytes());
    }
    out.batch.extend_from_slice(pkt);
    let end = out.batch.len().next_multiple_of(8);
    out.batch.resize(end, 0);
}

/// Sends `ui` the method `m` of its object `h`, which it answers later by
/// `tag`, and tells it the guest's cancel of it.
fn forward(ui: &Ui, h: Handle, m: Method, tag: Tag, args: Lift) -> i32 {
    let u = ui.h;
    tell(u, request(tag, h, m), args.pkt());
    ui.pending.borrow_mut().insert(tag);
    side().on_cancel(
        tag,
        Box::new(move || {
            tell(u, request(0, ROOT, CANCEL), Some(&tag.to_le_bytes()));
        }),
    );
    Errno::EINPROGRESS as i32
}

/// What stands for one of the UI's named interfaces on the link: each method
/// is sent to the UI attached.
struct Named {
    h: Handle,
}

impl Object for Named {
    fn call(&self, m: Method, tag: Tag, args: Lift, _: &mut Lower) -> i32 {
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
    fn call(&self, m: Method, tag: Tag, args: Lift, _: &mut Lower) -> i32 {
        match self.ui.upgrade() {
            Some(ui) => forward(&ui, self.h, m, tag, args),
            None => Errno::ECANCELED as i32,
        }
    }
}

impl Drop for Opened {
    fn drop(&mut self) {
        let h = self.h;
        if let Some(ui) = self.ui.upgrade() {
            tell(ui.h, request(0, h, DROP), None);
        }
    }
}

/// The root's `OPEN_AT`'s args, lowered: the handle `h`, the opener's place
/// `k` and `uri`, a `list<token>`.
fn lowered(h: Handle, k: u32, uri: &[Token]) -> Vec<u8> {
    let mut buf = vec![0; (side().layout().cap - HDR_SZ) as usize];
    let mut w = Lower::new(&mut buf, 16);
    w.u32(0, h);
    w.u32(4, k);
    store_uri(&mut w, 8, uri);
    let len = w.finish().ok().or_bug(Errno::EMSGSIZE);
    buf.truncate(len);
    buf
}

/// Mounts what the UI implements: its named interfaces under their handles,
/// and an opener at each of its `prefixes`, in its order, for its interface,
/// and attaches the first UI before the guest's first entry may call them
/// (`attach`).
pub(crate) fn mount(named: &[Handle], prefixes: Vec<(Handle, Vec<Token>)>) {
    attach();
    for &h in named {
        side().mount_named(h, Rc::new(Named { h }));
    }
    for (k, (i, p)) in (0..).zip(prefixes) {
        side().mount(i, p, move |uri, h| {
            let ui = ui().or_bug(Errno::ENOTCONN);
            tell(ui.h, request(0, ROOT, OPEN_AT), Some(&lowered(h, k, uri)));
            Ok(Rc::new(Opened {
                h,
                ui: Rc::downgrade(&ui),
            }) as Rc<dyn Object>)
        });
    }
}

/// Sends the method of the UI of the handle `u` on the link, its answer told
/// back to that UI alone, as @xpute/runtime-web remote.ts `attach` posts the
/// web UI's; a packet past the cap is refused whole, EMSGSIZE (xpute-link
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
        let answered = Frame {
            tag,
            res: true,
            handle: ROOT,
            method: 0,
            result,
            pkt_len: 0,
        };
        tell(u, answered, data.as_deref());
    };
    let pkt = pkt.as_deref().map_or(Packet::Args(None), Packet::Bytes);
    side().send(h, m, pkt, Some(Box::new(answer)));
}

/// The answer of the UI of the handle `u` to the request `tag` it owes,
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
