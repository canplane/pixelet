// xpute-link/link.test.rs

use std::cell::Cell;

use super::*;
use crate::mem::test::alloc;

// A guest's side and its host's, each crossing into the other's `recv`, over
// a layout of this thread's own.
thread_local! {
    static GUEST: Side = const { Side::new(|l| l.inbox + u64::from(l.cap), |at| HOST.with(|h| h.recv(at))) };
    static HOST: Side = const { Side::new(|l| l.inbox, |at| GUEST.with(|g| g.recv(at))) };
}

const CAP: u32 = 64;

/// An interface the host implements: method 0 answers at once with a word,
/// method 1 later, and method 2 at once with a value past the cap.
const INTERFACE: Handle = NAMED | 1;

struct Implementation {
    later: Rc<Cell<Option<Tag>>>,
}

impl Object for Implementation {
    fn call(&self, m: Method, tag: Tag, _: Lift, ret: &mut Lower) -> i32 {
        match m {
            0 => {
                ret.start(4);
                ret.u32(0, 7);
                0
            }
            1 => {
                self.later.set(Some(tag));
                Errno::EINPROGRESS as i32
            }
            _ => {
                ret.start(CAP as usize);
                0
            }
        }
    }
}

/// Lays the link and mounts the host's implementation: the inbox, and the tag
/// of the request method 1 left to answer.
fn link() -> (u64, Rc<Cell<Option<Tag>>>) {
    let l = Layout {
        inbox: alloc(2 * CAP),
        cap: CAP,
    };
    let later = Rc::new(Cell::new(None));
    GUEST.with(|g| g.set_layout(l));
    HOST.with(|h| {
        h.set_layout(l);
        h.mount_named(
            INTERFACE,
            Rc::new(Implementation {
                later: later.clone(),
            }),
        );
    });
    (l.inbox, later)
}

/// The answer a side lays in the frame its request came in is marked `RES`
/// and carries the request's tag, a value past the cap refused EMSGSIZE.
#[test]
fn recv_answers_res_with_tag() {
    let (inbox, _) = link();
    let outbox = inbox + u64::from(CAP);
    for (m, result) in [(0, 0), (2, Errno::EMSGSIZE as i32)] {
        let t = GUEST.with(|g| g.send(INTERFACE, m, Packet::Args(None), None));
        let f = Frame::at(outbox);
        assert_eq!(
            (f.res, f.tag, f.handle, f.method, f.result),
            (true, t, INTERFACE, m, result),
            "method {m}"
        );
    }
}

/// A later answer is marked `RES` and carries the request's tag, and its
/// request hears it, a value past the cap refused EMSGSIZE.
#[test]
fn task_return_answers_res_with_tag() {
    let (inbox, later) = link();
    let word = |w: &mut Lower| w.u32(0, 9);
    for (data, result, value) in [
        (
            Some((4, &word as &dyn Fn(&mut Lower))),
            0,
            Some(vec![9, 0, 0, 0]),
        ),
        (
            Some((CAP as usize, &word as &dyn Fn(&mut Lower))),
            Errno::EMSGSIZE as i32,
            None,
        ),
    ] {
        let heard = Rc::new(Cell::new(None));
        let h = heard.clone();
        let t = GUEST.with(|g| {
            g.send(
                INTERFACE,
                1,
                Packet::Args(None),
                Some(Box::new(move |r, d| h.set(Some((r, d))))),
            )
        });
        assert_eq!(later.take(), Some(t));
        HOST.with(|h| h.task_return(t, 0, Packet::Args(data)));
        let f = Frame::at(inbox);
        assert_eq!((f.res, f.tag, f.result), (true, t, result));
        GUEST.with(Side::run_due);
        assert_eq!(heard.take(), Some((result, value)));
    }
}

/// A cancel runs the stop its request put once, and none once the request
/// has answered (`CANCEL`).
#[test]
fn cancel_stops_once_and_never_after_answer() {
    let (_, later) = link();
    let stops = Rc::new(Cell::new(0));
    let stop = || {
        let s = stops.clone();
        Box::new(move || s.set(s.get() + 1)) as Box<dyn FnOnce()>
    };
    let t = GUEST.with(|g| g.send(INTERFACE, 1, Packet::Args(None), Some(Box::new(|_, _| {}))));
    assert_eq!(later.take(), Some(t));
    HOST.with(|h| h.on_cancel(t, stop()));
    GUEST.with(|g| g.cancel(t));
    GUEST.with(|g| g.cancel(t));
    assert_eq!(stops.get(), 1);

    let t = GUEST.with(|g| g.send(INTERFACE, 1, Packet::Args(None), Some(Box::new(|_, _| {}))));
    assert_eq!(later.take(), Some(t));
    HOST.with(|h| h.on_cancel(t, stop()));
    HOST.with(|h| h.task_return(t, 0, Packet::Args(None)));
    let tag = |w: &mut Lower| w.u32(0, t);
    GUEST.with(|g| g.send(ROOT, CANCEL, Packet::Args(Some((4, &tag))), None));
    assert_eq!(stops.get(), 1);
}

/// A table holds an object under a handle the other end issued, as under its
/// own, and its slot follows that end's next generation (`Table::put`).
#[test]
fn table_holds_other_ends_handle() {
    let mut issuer = Table::new();
    let mut holder = Table::new();
    let a = issuer.add(1);
    let b = issuer.add(2);
    holder.put(b, 2);
    assert_eq!(holder.get(b), Some(&2));
    assert_eq!(holder.get(a), None);
    assert_eq!(holder.remove(b), Some(2));
    assert_eq!(issuer.remove(b), Some(2));
    let c = issuer.add(3);
    holder.put(c, 3);
    holder.put(a, 1);
    assert_eq!(holder.get(b), None);
    assert_eq!(holder.get(c), Some(&3));
    assert_eq!(holder.get(a), Some(&1));
}
