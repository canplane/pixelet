// xpute-core/sched/turn.test.rs

use core::cell::Cell;
use core::pin::pin;
use core::task::{Context, RawWaker, RawWakerVTable};

use super::*;
use crate::clock;
use crate::global::{serial, Global};

thread_local! {
    static CLOCK_MS: Cell<f64> = const { Cell::new(0.0) };
    static WAKES: Cell<u32> = const { Cell::new(0) };
}

fn clock() -> f64 {
    CLOCK_MS.with(|c| c.get())
}

fn advance(ms: f64) {
    CLOCK_MS.with(|c| c.set(c.get() + ms));
}

const VTABLE: RawWakerVTable = RawWakerVTable::new(
    |p| RawWaker::new(p, &VTABLE),
    |_| WAKES.with(|w| w.set(w.get() + 1)),
    |_| WAKES.with(|w| w.set(w.get() + 1)),
    |_| {},
);

fn waker() -> Waker {
    // SAFETY: the vtable counts wakes and holds nothing.
    unsafe { Waker::from_raw(RawWaker::new(core::ptr::null(), &VTABLE)) }
}

static TURN: Global<Turn> = Global::new();

fn turn() -> &'static mut Turn {
    TURN.get(Turn::new)
}

#[test]
fn past_deadline_parks() {
    let _g = serial();
    clock::set(clock);
    CLOCK_MS.with(|c| c.set(0.0));
    TURN.set(Turn::new());
    WAKES.with(|w| w.set(0));
    let w = waker();
    let mut cx = Context::from_waker(&w);

    turn().start(4.0);
    let mut fut = pin!(yield_if_spent(turn));
    advance(5.0);
    assert!(fut.as_mut().poll(&mut cx).is_pending());
    assert_eq!(WAKES.with(|w| w.get()), 0);
    assert_eq!(turn().end(), 0.0);

    turn().start(4.0);
    assert_eq!(WAKES.with(|w| w.get()), 1);
    assert!(fut.as_mut().poll(&mut cx).is_ready());
}

#[test]
fn end_earliest_wait() {
    let _g = serial();
    clock::set(clock);
    CLOCK_MS.with(|c| c.set(0.0));
    TURN.set(Turn::new());
    turn().start(4.0);
    turn().wake_at(clock() + 300.0);
    turn().wake_at(clock() + 100.0);
    assert_eq!(turn().end(), 100.0);
    assert_eq!(turn().end(), NO_WAKE);
    turn().ask_next();
    turn().wake_at(clock() + 100.0);
    assert_eq!(turn().end(), 0.0);
}

/// A task waiting on its runtime asks for no turn, and is woken at the next
/// turn the runtime begins.
#[test]
fn runtime_turn_asks_none() {
    let _g = serial();
    clock::set(clock);
    CLOCK_MS.with(|c| c.set(0.0));
    TURN.set(Turn::new());
    WAKES.with(|w| w.set(0));
    let w = waker();
    let mut cx = Context::from_waker(&w);

    turn().start(4.0);
    let mut fut = pin!(runtime_turn(turn));
    assert!(fut.as_mut().poll(&mut cx).is_pending());
    assert_eq!(turn().end(), NO_WAKE);

    turn().start(4.0);
    assert_eq!(WAKES.with(|w| w.get()), 1);
    assert!(fut.as_mut().poll(&mut cx).is_ready());
}

static NOTIFY: Global<Notify> = Global::new();

fn notify() -> &'static mut Notify {
    NOTIFY.get(Notify::new)
}

#[test]
fn notice_wakes_task() {
    let _g = serial();
    clock::set(clock);
    CLOCK_MS.with(|c| c.set(0.0));
    TURN.set(Turn::new());
    NOTIFY.set(Notify::new());
    WAKES.with(|w| w.set(0));
    let w = waker();
    let mut cx = Context::from_waker(&w);

    let mut fut = pin!(notified(notify));
    assert!(fut.as_mut().poll(&mut cx).is_pending());
    assert_eq!(turn().end(), NO_WAKE);
    notify().notify();
    assert_eq!(WAKES.with(|w| w.get()), 1);
    assert!(fut.as_mut().poll(&mut cx).is_ready());

    notify().notify();
    assert!(pin!(notified(notify)).as_mut().poll(&mut cx).is_ready());
}
