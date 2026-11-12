// xpute-core/wait.rs

//! A sent method as a future, over `link::send`, with no executor of its
//! own. It is lazy: the method is sent at the first poll. Its answer keeps
//! the result, the return data and what the args lend, and wakes the waker
//! the last poll left; the future hands all three back. Dropping the future
//! cancels nothing: what the args lend stays alive until the answer, since
//! the answer owns it (wit/link.wit `lent`).

use core::future::Future;
use core::pin::Pin;
use core::task::{Context, Poll, Waker};
use std::cell::Cell;
use std::rc::Rc;

use xpute_kit::wire::xtp::PacketWriter;

use crate::link::{self, Handle, Method};

/// A method's args, owned until the send: how many fields, and what writes
/// them. None, no packet.
pub type Args = Option<(u32, Box<dyn Fn(&mut PacketWriter)>)>;

/// Where the answer leaves its result, its return data and what was lent,
/// and the poll its waker.
struct Slot<L> {
    answer: Cell<Option<(i32, Option<Vec<u8>>, L)>>,
    waker: Cell<Option<Waker>>,
}

pub struct Wait<L> {
    /// The method, its args and what they lend, until the first poll.
    send: Option<(Handle, Method, Args, L)>,
    slot: Rc<Slot<L>>,
}

/// Sends `m` of `h` with `args` at the first poll, and answers its result,
/// its return data and `lent`, what the args lend by (at, len), kept until
/// the answer.
pub fn wait<L: Unpin + 'static>(h: Handle, m: Method, args: Args, lent: L) -> Wait<L> {
    Wait {
        send: Some((h, m, args, lent)),
        slot: Rc::new(Slot {
            answer: Cell::new(None),
            waker: Cell::new(None),
        }),
    }
}

impl<L: Unpin + 'static> Future for Wait<L> {
    type Output = (i32, Option<Vec<u8>>, L);

    fn poll(self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<Self::Output> {
        let this = self.get_mut();
        if let Some((h, m, args, lent)) = this.send.take() {
            let slot = this.slot.clone();
            let args = args.as_ref().map(|(n, w)| (*n, &**w as &dyn Fn(&mut PacketWriter)));
            link::send(
                h,
                m,
                args,
                Some(Box::new(move |result, data| {
                    slot.answer.set(Some((result, data, lent)));
                    if let Some(w) = slot.waker.take() {
                        w.wake();
                    }
                })),
            );
        }
        match this.slot.answer.take() {
            Some(answer) => Poll::Ready(answer),
            None => {
                this.slot.waker.set(Some(cx.waker().clone()));
                Poll::Pending
            }
        }
    }
}
