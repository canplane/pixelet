// xpute-guest/sched/edge.test.rs

use core::cell::Cell;
use core::pin::pin;
use core::task::{Context, RawWaker, RawWakerVTable};

use super::*;
use crate::clock;
use crate::global::{serial, Global};

thread_local! {
    static CLOCK_MS: Cell<f64> = const { Cell::new(0.0) };
    static WOKEN: Cell<u32> = const { Cell::new(0) };
}

fn clock() -> f64 {
    CLOCK_MS.with(|c| c.get())
}

fn advance(ms: f64) {
    CLOCK_MS.with(|c| c.set(c.get() + ms));
}

const VTABLE: RawWakerVTable = RawWakerVTable::new(
    |p| RawWaker::new(p, &VTABLE),
    |_| WOKEN.with(|w| w.set(w.get() + 1)),
    |_| WOKEN.with(|w| w.set(w.get() + 1)),
    |_| {},
);

fn waker() -> Waker {
    // SAFETY: the vtable counts wakes and holds nothing.
    unsafe { Waker::from_raw(RawWaker::new(core::ptr::null(), &VTABLE)) }
}

static EDGE: Global<Edge<2>> = Global::new();

fn edge() -> &'static mut Edge<2> {
    EDGE.get(Edge::new)
}

#[test]
fn spent_quota_parks() {
    let _g = serial();
    clock::set(clock);
    CLOCK_MS.with(|c| c.set(0.0));
    EDGE.set(Edge::new());
    WOKEN.with(|w| w.set(0));
    let w = waker();
    let mut cx = Context::from_waker(&w);

    edge().rise(4.0);
    let mut fut = pin!(yield_if_spent(edge));
    advance(5.0);
    assert!(fut.as_mut().poll(&mut cx).is_pending());
    assert_eq!(WOKEN.with(|w| w.get()), 0);
    assert_eq!(edge().fall(), 0.0);

    edge().rise(4.0);
    assert_eq!(WOKEN.with(|w| w.get()), 1);
    assert!(fut.as_mut().poll(&mut cx).is_ready());
}

#[test]
fn falling_edge_earliest_wait() {
    let _g = serial();
    clock::set(clock);
    CLOCK_MS.with(|c| c.set(0.0));
    EDGE.set(Edge::new());
    edge().rise(4.0);
    edge().wake_at(clock() + 300.0);
    edge().wake_at(clock() + 100.0);
    assert_eq!(edge().fall(), 100.0);
    assert_eq!(edge().fall(), NO_WAKE);
    edge().ask_next();
    edge().wake_at(clock() + 100.0);
    assert_eq!(edge().fall(), 0.0);
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
    EDGE.set(Edge::new());
    NOTIFY.set(Notify::new());
    WOKEN.with(|w| w.set(0));
    let w = waker();
    let mut cx = Context::from_waker(&w);

    let mut fut = pin!(notified(notify));
    assert!(fut.as_mut().poll(&mut cx).is_pending());
    assert_eq!(edge().fall(), NO_WAKE);
    notify().notify();
    assert_eq!(WOKEN.with(|w| w.get()), 1);
    assert!(fut.as_mut().poll(&mut cx).is_ready());

    notify().notify();
    assert!(pin!(notified(notify)).as_mut().poll(&mut cx).is_ready());
}
