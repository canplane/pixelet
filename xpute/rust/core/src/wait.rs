// xpute-core/wait.rs

//! A sent method as a future, over `link::send`, with no executor of its
//! own. It is lazy: the method is sent at the first poll. Its answer keeps
//! the result and the return data, and wakes the waker the last poll left. Dropping the future
//! drops the result and cancels nothing; what its args lend by (at, len)
//! stays alive until the answer, since the answer owns it.

use core::future::Future;
use core::pin::Pin;
use core::task::{Context, Poll, Waker};
use std::any::Any;
use std::cell::Cell;
use std::rc::Rc;

use xpute_kit::wire::xtp::PacketWriter;

use crate::link::{self, Handle, Method};

/// A method's args, owned until the send: how many fields, and what writes
/// them. None, no packet.
pub type Args = Option<(u32, Box<dyn Fn(&mut PacketWriter)>)>;

/// Where the answer leaves its result and return data, and the poll its
/// waker.
#[derive(Default)]
struct Slot {
    answer: Cell<Option<(i32, Option<Vec<u8>>)>>,
    waker: Cell<Option<Waker>>,
}

pub struct Wait {
    /// The method, its args and what they lend, until the first poll.
    send: Option<(Handle, Method, Args, Box<dyn Any>)>,
    slot: Rc<Slot>,
}

/// Sends `m` of `h` with `args` at the first poll, and answers its result and return
/// data. `lent` is what the args lend by (at, len), kept until the answer.
pub fn wait(h: Handle, m: Method, args: Args, lent: impl Any) -> Wait {
    Wait {
        send: Some((h, m, args, Box::new(lent))),
        slot: Rc::default(),
    }
}

impl Future for Wait {
    type Output = (i32, Option<Vec<u8>>);

    fn poll(mut self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<Self::Output> {
        if let Some((h, m, args, lent)) = self.send.take() {
            let slot = self.slot.clone();
            let args = args.as_ref().map(|(n, w)| (*n, &**w as &dyn Fn(&mut PacketWriter)));
            link::send(
                h,
                m,
                args,
                Some(Box::new(move |result, data| {
                    drop(lent);
                    slot.answer.set(Some((result, data)));
                    if let Some(w) = slot.waker.take() {
                        w.wake();
                    }
                })),
            );
        }
        match self.slot.answer.take() {
            Some(answer) => Poll::Ready(answer),
            None => {
                self.slot.waker.set(Some(cx.waker().clone()));
                Poll::Pending
            }
        }
    }
}
