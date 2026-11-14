// xpute-native-host/remote.rs

//! The link's side of remote.ts `serve`, for a UI on the JavaScript thread.
//! The stand-ins of the interfaces the UI serves are mounted under their
//! named handles before the guest boots, and each method is told to the UI
//! and answered 0 at once. The
//! UI's methods come through the queue (lib.rs `post`), and what the UI hears is
//! batched and handed over on one notify (`take`), as a page's port posts.
//!
//! A batch is the messages remote.ts `ToUi` carries, each an XTP packet laid
//! after its length (a u32, then four bytes so the packet starts on a word):
//! a sent method's answer `(0, tag, result, data)`, or a method of one of
//! the UI's interfaces `(2, h, m, pkt)`, where data and pkt are a packet's
//! bytes or nil.

use std::rc::Rc;
use std::sync::Mutex;

use xpute_link::Packet;
use xpute_kit::wire::xtp::{PacketWriter, MAX_PKT_SZ};

use crate::link;
use xpute_link::{Handle, Method, Object, Reply, Tag};

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

/// What stands for one of the UI's objects on the link: its methods are told
/// to the UI, and answered 0 at once.
struct StandIn(Handle);

impl Object for StandIn {
    fn call(&self, m: Method, _tag: Tag, args: Option<&[u8]>) -> Reply {
        tell(4, |w| {
            w.u32(Some(2)).u32(Some(self.0)).u32(Some(m)).array::<u8>(args);
        });
        Reply::from(0)
    }
}

/// Mounts the stand-ins of the interfaces the UI serves, under their named
/// handles (remote.ts `Remote.named`).
pub fn serve(named: &[Handle]) {
    for &h in named {
        link::mount_named(h, Rc::new(StandIn(h)));
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
    link::send_packet(h, m, pkt, Some(Box::new(answer)));
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
