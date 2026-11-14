// xpute-native-runtime/remote.rs

//! The link's side of remote.ts `serve`, for a UI on the JavaScript thread,
//! and the link's serving half for what the UI serves across it. The UI's
//! methods come through the queue (lib.rs `post`), and what the UI hears is
//! batched and handed over on one notify (`take`), as a page's port posts.
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

use std::rc::Rc;
use std::sync::Mutex;

use xpute_kit::status::bug::OrBug;
use xpute_kit::status::errno::Errno;
use xpute_kit::wire::xtp::{PacketWriter, MAX_PKT_SZ};
use xpute_link::abi::types::Token;
use xpute_link::{Handle, Lower, Method, Object, Packet, Reply, Tag};

use crate::link;

/// The batch the UI has not taken, and whether it was told of it.
static OUT: Mutex<(Vec<u8>, bool)> = Mutex::new((Vec::new(), false));

/// Appends to the batch one message of `cnt` fields that `write` writes.
fn tell(cnt: u32, write: impl Fn(&mut PacketWriter)) {
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
    let mut out = OUT.lock().unwrap_or_else(|e| e.into_inner());
    out.0.extend_from_slice(&(pkt.len() as u32).to_le_bytes());
    out.0.extend_from_slice(&[0; 4]);
    out.0.extend_from_slice(&pkt);
    let end = out.0.len().next_multiple_of(8);
    out.0.resize(end, 0);
}

/// Tells the UI the method `m` of its object `h`, to answer by `tag` where
/// `later`.
fn tell_method(h: Handle, m: Method, later: bool, tag: Tag, args: Option<&[u8]>) {
    tell(6, |w| {
        w.u32(Some(2))
            .u32(Some(h))
            .u32(Some(m))
            .u32(Some(u32::from(later)))
            .u32(Some(tag))
            .array::<u8>(args);
    });
}

/// Tells the UI a method it answers later, and the guest's cancel of it.
fn forward(h: Handle, m: Method, tag: Tag, args: Option<&[u8]>) -> Reply {
    tell_method(h, m, true, tag, args);
    link::on_cancel(
        tag,
        Box::new(move || {
            tell(2, |w| {
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
        if self.later.contains(&(self.h, m)) {
            return forward(self.h, m, tag, args);
        }
        tell_method(self.h, m, false, tag, args);
        Reply::from(0)
    }
}

/// What stands for an object the UI opened: each method is answered later,
/// and its drop is told.
struct Opened(Handle);

impl Object for Opened {
    fn call(&self, m: Method, tag: Tag, args: Option<&[u8]>) -> Reply {
        forward(self.0, m, tag, args)
    }
}

impl Drop for Opened {
    fn drop(&mut self) {
        let h = self.0;
        tell(2, |w| {
            w.u32(Some(5)).u32(Some(h));
        });
    }
}

/// `uri` as a lowered `list<token>`, as ROOT OPEN's args are.
fn lowered(uri: &[Token]) -> Vec<u8> {
    let mut buf = vec![0; link::pkt_cap() as usize];
    let mut w = Lower::new(&mut buf, 8);
    if let Some(base) = w.list(0, uri.len(), Token::SZ, Token::ALIGN) {
        uri.iter().enumerate().for_each(|(k, t)| t.store(&mut w, base + k * Token::SZ));
    }
    let len = w.finish().ok().or_bug(Errno::EMSGSIZE);
    buf.truncate(len);
    buf
}

/// Mounts what the UI serves: its named interfaces under their handles
/// (remote.ts `Remote.named`), the methods `later` names answered later, and
/// an opener at each of its `prefixes` (remote.ts `Remote.prefixes`).
pub fn serve(named: &[Handle], prefixes: Vec<Vec<Token>>, later: &'static [(Handle, Method)]) {
    for &h in named {
        link::mount_named(h, Rc::new(Named { h, later }));
    }
    for p in prefixes {
        link::mount(p, |uri, h| {
            let uri = lowered(uri);
            tell(3, |w| {
                w.u32(Some(3)).u32(Some(h)).array::<u8>(Some(&uri));
            });
            Ok(Rc::new(Opened(h)) as Rc<dyn Object>)
        });
    }
}

/// Sends the UI's method on the link, its answer told back (remote.ts
/// `port.onmessage`); a packet past the room is refused whole, EMSGSIZE
/// (xpute-link link.rs `Side::send`).
pub(crate) fn recv(tag: Tag, h: u32, m: u32, pkt: Option<Vec<u8>>) {
    let answer = move |result: i32, data: Option<Vec<u8>>| {
        tell(4, |w| {
            w.u32(Some(0))
                .u32(Some(tag))
                .i32(Some(result))
                .array::<u8>(data.as_deref());
        });
    };
    let pkt = pkt.as_deref().map_or(Packet::Args(None), Packet::Bytes);
    link::send_pkt(h, m, pkt, Some(Box::new(answer)));
}

/// The UI's answer to the request `tag` it serves later.
pub(crate) fn answer(tag: Tag, result: i32, data: Option<Vec<u8>>) {
    link::task_return_pkt(tag, result, data);
}

/// Tells the UI once that a batch waits, after the work at hand.
pub fn flush(notify: fn()) {
    let tell = {
        let mut out = OUT.lock().unwrap_or_else(|e| e.into_inner());
        let tell = !out.0.is_empty() && !out.1;
        out.1 |= tell;
        tell
    };
    if tell {
        notify();
    }
}

/// Hands over the batch the UI has not taken. Called on the JavaScript
/// thread, on the notify.
pub fn take() -> Vec<u8> {
    let mut out = OUT.lock().unwrap_or_else(|e| e.into_inner());
    out.1 = false;
    core::mem::take(&mut out.0)
}
