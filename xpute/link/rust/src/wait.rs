// xpute-link/wait.rs

//! A sent method as a future, over a side's `send`, with no executor of its
//! own. The method is sent when the future is made. Its answer keeps the
//! result, the return data and what the args lend, and wakes the waker the
//! last poll left; the future hands all three back. Dropping the future
//! before its answer cancels the call (`frame.json5` `CANCEL`), as the
//! component model's dropped subtask is canceled; what the args lend stays
//! alive until the answer, which still comes and owns it
//! (`link/wit/types.wit` `lent`).

use core::pin::Pin;
use core::task::{Context, Poll, Waker};
use std::cell::Cell;
use std::rc::Rc;

use crate::link::{self, Answer, Handle, Method, Tag};

/// What a sent method comes to: its result, its return data and what its args
/// lent.
pub type Outcome<L> = (i32, Option<Vec<u8>>, L);

/// Where the answer leaves its outcome, and the poll its waker.
struct Slot<L> {
    answer: Cell<Option<Outcome<L>>>,
    waker: Cell<Option<Waker>>,
}

pub struct Wait<L> {
    slot: Rc<Slot<L>>,
    /// The request, and the side's `cancel`, for a drop before the answer.
    tag: Tag,
    cancel: Box<dyn Fn(Tag)>,
}

/// Sends `m` of `h` by `by`, a side's send, which answers the request's tag,
/// with `args`, now, and answers its outcome; `lent` is what the args lend by
/// (at, len), kept until the answer. `cancel` is the side's, which a drop
/// before the answer calls.
pub fn wait_by<L: 'static>(
    by: impl FnOnce(Handle, Method, link::Args<'_>, Option<Answer>) -> Tag,
    cancel: Box<dyn Fn(Tag)>,
    h: Handle,
    m: Method,
    args: link::Args<'_>,
    lent: L,
) -> Wait<L> {
    let slot = Rc::new(Slot {
        answer: Cell::new(None),
        waker: Cell::new(None),
    });
    let answered = slot.clone();
    let tag = by(
        h,
        m,
        args,
        Some(Box::new(move |result, data| {
            answered.answer.set(Some((result, data, lent)));
            if let Some(w) = answered.waker.take() {
                w.wake();
            }
        })),
    );
    Wait { slot, tag, cancel }
}

impl<L> Drop for Wait<L> {
    fn drop(&mut self) {
        // The answer holds the slot until it comes; once it has, a drop
        // cancels nothing.
        if Rc::strong_count(&self.slot) > 1 {
            (self.cancel)(self.tag);
        }
    }
}

impl<L> Future for Wait<L> {
    type Output = Outcome<L>;

    fn poll(self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<Self::Output> {
        match self.slot.answer.take() {
            Some(answer) => Poll::Ready(answer),
            None => {
                self.slot.waker.set(Some(cx.waker().clone()));
                Poll::Pending
            }
        }
    }
}
