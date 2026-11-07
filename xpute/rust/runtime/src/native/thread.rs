// xpute-runtime/native/thread.rs

//! The core's own thread, a vCPU that runs while the core has work: it runs
//! a turn (`Vcpu::turn`) whenever it is kicked, and sleeps otherwise. A kick
//! carries the next turn's deadline, ms from its rise, the latest of the
//! kicks since; a turn no kick gave one has a deadline of 0, and applies what
//! was sent, as a page's doorbell rings for it. A caller's work (`run`), such
//! as a device's, runs on it between turns while the caller waits. The thread
//! keeps no timer: when a turn asks to be woken, at once or in `n` ms, it is
//! told the runtime (`turns`, `ring`), whose scheduler asks its policy for the
//! moment, as a page's does (@xpute/runtime sched/scheduler.ts), and kicks then, so
//! no turn runs between a display's frames. Paused (`pause`), it runs only
//! the turns a caller rings, and its kicks wait for `resume`.
//!
//! The core's calls are answered on a thread outside it. A call made
//! by a caller's work is handed to that caller. One made by a turn is handed
//! to a caller waiting in `ring` where there is one; otherwise it is told the
//! program (`notify_on`), which answers it on its own thread (`answer`),
//! while the core's thread waits, as a vCPU waits out an exit.
//!
//! Three rules hold the loop:
//!
//! - No kick is lost. A kick sets `kicked` under `LOOP`'s lock and wakes the
//!   thread; the thread reads `kicked` and its work under that lock before
//!   it waits, and `Condvar::wait` gives the lock up as it sleeps, so a kick
//!   lands before the check or wakes the wait.
//! - Every wait has its answerer. The core's thread waits for an answer only
//!   while a call is in `LOOP`: a caller waiting in `ring`, or in `run`
//!   on the thread that rings, answers it there, and with none waiting the
//!   program is told and answers it on that thread. The runtime's thread
//!   waits on the core only in `ring` and `run`, and answers in both, so the
//!   two never wait on each other.
//! - A turn begins only where the core can post what one may (`Vcpu::ready`,
//!   the door's room in the completion ring), read under the lock. Where it
//!   cannot, the thread says so to a caller in `ring` and to the runtime, and
//!   waits for the kick that follows the runtime's read.
//! - Every pass drains the submission ring first: a turn applies what waits
//!   there before any phase's budget (xpute-core ipc/door.rs `receive`), so
//!   a turn the core asked for itself carries the runtime's kicks too.

use core::cell::Cell;
use core::panic::AssertUnwindSafe;
use std::sync::{Condvar, Mutex, MutexGuard, PoisonError};
use std::time::{Duration, Instant};

use xpute_core::global::on_marked_thread;

/// What the core's thread runs as it loops: a turn of a deadline, answering
/// when the core wants the next (`xpute_core::entry::Entry`), and whether
/// one may begin.
#[derive(Clone, Copy)]
pub struct Vcpu {
    pub turn: fn(deadline_ms: f64) -> f64,
    pub ready: fn() -> bool,
}

/// A caller's work, borrowed: the caller waits until it has run.
struct Job(*mut (dyn FnMut() + Send + 'static));

// SAFETY: the closure it points at is `Send`, and only the core's thread
// runs it, while the caller that owns it waits.
unsafe impl Send for Job {}

/// Who answers a call.
#[derive(Clone, Copy, PartialEq, Eq)]
enum Asker {
    /// The caller whose work made it.
    Job,
    /// The thread that rings: a turn made it.
    Turn,
}

/// What the runtime reads of the turns since it last asked (`turns`).
struct Turns {
    /// A turn answered 0: the core wants the next at once.
    resched: bool,
    /// The longest turn, in ms.
    turn_ms: f64,
    /// The runtime is told and has not asked since, so it is not told again.
    told: bool,
}

struct Loop {
    kicked: bool,
    /// The next turn's deadline, the latest a kick since gave.
    deadline: Option<f64>,
    /// When the last turn asked to be woken, past now, as the runtime is
    /// told it (`wake_of`); one that answered 0 is `Turns::resched`. The thread
    /// waits on it not: the runtime's kick wakes it.
    wake_at: Option<Instant>,
    job: Option<Job>,
    job_ends: u64,
    starts: u64,
    ends: u64,
    /// The thread's looks at whether a turn may begin, and the last that found
    /// it could not.
    looks: u64,
    stall_at: u64,
    call: Option<(u32, Asker)>,
    ret: Option<i32>,
    /// Callers waiting in `ring`.
    ringers: u32,
    /// The turn those callers wait for, by `starts`: while paused, turns begin
    /// only until it has.
    ring_until: u64,
    /// Callers in `while_idle`.
    idlers: u32,
    /// The core's thread runs a turn or a caller's work.
    busy: bool,
    /// No turn begins but a caller's in `ring` (`pause`, `resume`).
    paused: bool,
    turns: Turns,
    terminated: bool,
}

static LOOP: Mutex<Loop> = Mutex::new(Loop {
    kicked: false,
    deadline: None,
    wake_at: None,
    job: None,
    job_ends: 0,
    starts: 0,
    ends: 0,
    looks: 0,
    stall_at: 0,
    call: None,
    ret: None,
    ringers: 0,
    ring_until: 0,
    idlers: 0,
    busy: false,
    paused: false,
    turns: Turns {
        resched: false,
        turn_ms: 0.0,
        told: false,
    },
    terminated: false,
});

/// What the core's thread waits on: a kick, work, an answer, a hold let go.
static CORE_CV: Condvar = Condvar::new();
/// What a caller waits on: a turn ended or stalled, work done, a call,
/// the core idle or stopped.
static RUNTIME_CV: Condvar = Condvar::new();

/// One caller's work at a time.
static JOBS: Mutex<()> = Mutex::new(());

fn state() -> MutexGuard<'static, Loop> {
    LOOP.lock().unwrap_or_else(PoisonError::into_inner)
}

fn wait<'a>(cv: &Condvar, g: MutexGuard<'a, Loop>) -> MutexGuard<'a, Loop> {
    cv.wait(g).unwrap_or_else(PoisonError::into_inner)
}

/// The runtime's answer to a call, run on the answering thread.
pub type Answer = extern "C" fn(at: u32) -> i32;

static ANSWER: Mutex<Option<Answer>> = Mutex::new(None);

/// `answer` answers the core's calls from the next on; none, ENOSYS.
pub fn answer_with(answer: Option<Answer>) {
    *ANSWER.lock().unwrap_or_else(PoisonError::into_inner) = answer;
}

fn answer_of(at: u32) -> i32 {
    let answer = *ANSWER.lock().unwrap_or_else(PoisonError::into_inner);
    answer.map_or(xpute_kit::status::errno::Errno::ENOSYS as i32, |f| f(at))
}

/// What tells the runtime, from the core's thread, that the core has
/// posted or asked since it last read (`turns`), that a turn's call
/// waits for its answer (`answer`), or that the core has stopped. It must
/// not call into the core: it hands the news to the runtime's thread.
pub type Notify = extern "C" fn();

static NOTIFY: Mutex<Option<Notify>> = Mutex::new(None);

/// `notify` tells the runtime from the next news on; none, nothing is told.
pub fn notify_on(notify: Option<Notify>) {
    *NOTIFY.lock().unwrap_or_else(PoisonError::into_inner) = notify;
}

fn notify() {
    let notify = *NOTIFY.lock().unwrap_or_else(PoisonError::into_inner);
    if let Some(f) = notify {
        f();
    }
}

thread_local! {
    /// A `run`, `ring` or `while_idle` is under way on this thread.
    #[cfg_attr(target_os = "android", allow(clippy::missing_const_for_thread_local, reason = "already const; the lint misreads Android's expansion"))]
    static IN_CALL: Cell<bool> = const { Cell::new(false) };
    /// This thread rings the core, so it answers a turn's calls.
    #[cfg_attr(target_os = "android", allow(clippy::missing_const_for_thread_local, reason = "already const; the lint misreads Android's expansion"))]
    static RINGS: Cell<bool> = const { Cell::new(false) };
    /// On the core's thread: a caller's work runs, not a turn.
    #[cfg_attr(target_os = "android", allow(clippy::missing_const_for_thread_local, reason = "already const; the lint misreads Android's expansion"))]
    static IN_JOB: Cell<bool> = const { Cell::new(false) };
}

/// This thread waits on the core until it is let go.
struct Hold;

impl Drop for Hold {
    fn drop(&mut self) {
        IN_CALL.with(|h| h.set(false));
    }
}

/// Refused where waiting would wait for itself: on the core's thread, or
/// on one that already waits on it.
fn hold() -> Hold {
    assert!(
        !on_marked_thread(),
        "a call into the core from its own thread would wait for itself"
    );
    assert!(
        !IN_CALL.with(Cell::get),
        "a call into the core inside another on the same thread would wait for itself"
    );
    IN_CALL.with(|h| h.set(true));
    Hold
}

/// Starts the core's thread, which runs `first` before anything else, then
/// loops on `vcpu`.
pub(super) fn start(
    name: &'static str,
    stack_bytes: usize,
    vcpu: Vcpu,
    first: impl FnOnce() + Send,
) {
    std::thread::Builder::new()
        .name(name.into())
        .stack_size(stack_bytes)
        .spawn(move || run_loop(vcpu))
        .expect("the system would not start the core's thread");
    run(first);
}

/// What the core's thread does next: a caller's work, or a turn of a deadline.
enum Next {
    Job(Job),
    Turn { deadline_ms: f64 },
}

/// The core's thread: its turns and callers' work until a panic unwinds out of one, the
/// core's stop, which its hook has told (xpute-core stop.rs, `stopped`). The thread then
/// ends the core here, outside the hook (`terminate`), and stays, with the core's memory
/// as the panic left it, so the process goes on without it.
fn run_loop(vcpu: Vcpu) {
    if std::panic::catch_unwind(AssertUnwindSafe(|| serve(vcpu))).is_err() {
        terminate();
    }
    loop {
        std::thread::park();
    }
}

fn serve(vcpu: Vcpu) {
    loop {
        let next = next(vcpu);
        match next {
            Next::Job(Job(job)) => {
                IN_JOB.with(|j| j.set(true));
                // SAFETY: the caller's closure, which it keeps until it has
                // seen `job_ends` move.
                unsafe { (*job)() };
                IN_JOB.with(|j| j.set(false));
                let mut g = state();
                g.busy = false;
                g.job_ends += 1;
                RUNTIME_CV.notify_all();
            }
            Next::Turn { deadline_ms } => {
                let t0 = Instant::now();
                let wake = (vcpu.turn)(deadline_ms);
                let turn_ms = t0.elapsed().as_secs_f64() * 1000.0;
                let mut g = state();
                g.busy = false;
                g.ends += 1;
                g.wake_at = (wake > 0.0).then(|| Instant::now() + duration_of(wake));
                g.turns.resched |= wake == 0.0;
                g.turns.turn_ms = g.turns.turn_ms.max(turn_ms);
                RUNTIME_CV.notify_all();
                let tell = g.ringers == 0 && !core::mem::replace(&mut g.turns.told, true);
                drop(g);
                if tell {
                    notify();
                }
            }
        }
    }
}

fn duration_of(ms: f64) -> Duration {
    Duration::try_from_secs_f64(ms / 1000.0).unwrap_or(Duration::MAX)
}

/// The next work or turn, waiting for it: a caller's work first, then a
/// turn once kicked, where one may begin and the thread is not paused
/// or a caller rings. Whether one may is read under the lock, so the
/// program's read of the completion ring lands before it or is followed by
/// the kick that wakes the wait below.
fn next(vcpu: Vcpu) -> Next {
    let mut g = state();
    loop {
        let runs = !g.paused || g.starts < g.ring_until;
        if g.idlers == 0 {
            if let Some(job) = g.job.take() {
                g.busy = true;
                return Next::Job(job);
            }
            if runs && g.kicked {
                g.looks += 1;
                g.kicked = false;
                g.wake_at = None;
                if (vcpu.ready)() {
                    let deadline_ms = g.deadline.take().unwrap_or(0.0);
                    g.starts += 1;
                    g.busy = true;
                    return Next::Turn { deadline_ms };
                }
                g.stall_at = g.looks;
                RUNTIME_CV.notify_all();
                if g.ringers == 0 && !core::mem::replace(&mut g.turns.told, true) {
                    drop(g);
                    notify();
                    g = state();
                    continue;
                }
            }
        }
        g = wait(&CORE_CV, g);
    }
}

/// A turn wanted at once, the next ending no sooner than `deadline` from its rise where one is
/// given, from any thread; it returns at once.
fn kick_with(deadline: Option<f64>) {
    kick_locked(&mut state(), deadline);
}

fn kick_locked(g: &mut Loop, deadline: Option<f64>) {
    g.kicked = true;
    if let Some(q) = deadline {
        g.deadline = Some(g.deadline.map_or(q, |p| p.max(q)));
    }
    CORE_CV.notify_all();
}

/// When the core wants the next turn, in ms from now: 0 where a turn since
/// the runtime last read answered 0, `NO_WAKE` for none.
fn wake_of(g: &Loop) -> f64 {
    match g.wake_at {
        _ if g.turns.resched => 0.0,
        Some(at) => at.saturating_duration_since(Instant::now()).as_secs_f64() * 1000.0,
        None => xpute_core::sched::wake::NO_WAKE,
    }
}

/// A device's kick, from whichever thread learned of a completion: the next
/// turn reads it.
pub fn kick() {
    kick_with(None);
}

/// The runtime's kick, after it has written the submission ring: the next
/// turn ending no sooner than `deadline_ms` from its rise, none of its own where negative. This
/// thread is the one that answers a turn's calls from now on.
pub fn kick_deadline(deadline_ms: f64) {
    RINGS.with(|r| r.set(true));
    kick_with((deadline_ms >= 0.0).then_some(deadline_ms));
}

/// The call waiting in `LOOP`, if this thread is its answerer, answered
/// here; whether one was. `g` is the lock, given back.
fn answer_here<'a>(
    mut g: MutexGuard<'a, Loop>,
    takes: impl Fn(Asker) -> bool,
) -> (MutexGuard<'a, Loop>, bool) {
    let Some((at, asker)) = g.call else {
        return (g, false);
    };
    if !takes(asker) {
        return (g, false);
    }
    g.call = None;
    drop(g);
    let res = answer_of(at);
    let mut g = state();
    g.ret = Some(res);
    CORE_CV.notify_all();
    (g, true)
}

/// No turn begins from now on but one a caller rings (`ring`); a kick waits
/// for `resume`. A caller's work still runs.
pub fn pause() {
    state().paused = true;
}

/// Turns begin again as kicks ask, one that came while paused at once.
pub fn resume() {
    state().paused = false;
    CORE_CV.notify_all();
}

/// The core's call, on its own thread: handed to its answerer, which
/// runs the runtime's answer, the core waiting.
pub(super) fn call(at: u32) -> i32 {
    let asker = if IN_JOB.with(Cell::get) {
        Asker::Job
    } else {
        Asker::Turn
    };
    let mut g = state();
    g.call = Some((at, asker));
    g.ret = None;
    RUNTIME_CV.notify_all();
    let tell = asker == Asker::Turn && g.ringers == 0;
    if tell {
        drop(g);
        notify();
        g = state();
    }
    loop {
        if let Some(res) = g.ret.take() {
            return res;
        }
        g = wait(&CORE_CV, g);
    }
}

/// The turn's call waiting, if one does, answered on this thread, the
/// program's: what its `Notify` was told of.
pub fn answer() {
    RINGS.with(|r| r.set(true));
    let g = state();
    let _ = answer_here(g, |a| a == Asker::Turn);
}

/// What the turns since the last call asked, for the runtime's thread: when
/// the core wants the next, in ms (0 where a turn answered 0, `NO_WAKE` for
/// none, NaN once it has stopped), and the longest of them, in ms.
pub fn turns() -> (f64, f64) {
    let mut g = state();
    g.turns.told = false;
    let wake = if g.terminated { f64::NAN } else { wake_of(&g) };
    g.turns.resched = false;
    (wake, core::mem::take(&mut g.turns.turn_ms))
}

/// A turn ending no sooner than `deadline_ms` from its rise, this thread waiting until one begun
/// after this call has ended, answering the turns' calls meanwhile: when
/// the core wants the next, in ms. Back early where no turn may begin until
/// the runtime reads the completion ring; none where the core has stopped.
pub fn ring(deadline_ms: f64) -> Option<f64> {
    let _hold = hold();
    let mut g = state();
    if g.terminated {
        return None;
    }
    RINGS.with(|r| r.set(true));
    let want = g.starts + 1;
    let looked = g.looks;
    kick_locked(&mut g, Some(deadline_ms));
    g.ringers += 1;
    g.ring_until = g.ring_until.max(want);
    let out = loop {
        if g.terminated {
            break None;
        }
        let answered;
        (g, answered) = answer_here(g, |a| a == Asker::Turn);
        if answered {
            continue;
        }
        if g.ends >= want || g.stall_at > looked {
            break Some(wake_of(&g));
        }
        g = wait(&RUNTIME_CV, g);
    };
    g.ringers -= 1;
    if g.ringers == 0 {
        g.ring_until = g.starts;
    }
    out
}

/// The core has stopped, told by its hook on the core's thread, inside the panic hook
/// (say.rs `stop_hook`). Where a panic unwinds it returns, and the thread ends the core
/// outside the hook (`run_loop`).
pub(super) fn stopped() {
    // Where a panic aborts, nothing unwinds out of the hook, and returning would end the
    // process, which outlives its core to show the stop's report: the core is ended here
    // and its thread kept inside the hook, which nothing in such a program replaces.
    #[cfg(panic = "abort")]
    {
        terminate();
        loop {
            std::thread::park();
        }
    }
}

/// The core has stopped, told from its own thread: every caller waiting is told, and so
/// is the runtime.
fn terminate() {
    let mut g = state();
    g.terminated = true;
    g.busy = false;
    g.turns.told = true;
    RUNTIME_CV.notify_all();
    drop(g);
    notify();
}

/// `f` on the core's thread between turns, the caller waiting; each call
/// it makes meanwhile answered on the caller's, and on the thread that rings
/// a turn's as well. None where the core has stopped, in this call or
/// before it.
pub(super) fn run<R: Send>(f: impl FnOnce() -> R + Send) -> Option<R> {
    let _hold = hold();
    let _one = JOBS.lock().unwrap_or_else(PoisonError::into_inner);
    let rings = RINGS.with(Cell::get);
    let mut f = Some(f);
    let mut out = None;
    let mut job = || out = f.take().map(|f| f());
    let job: *mut (dyn FnMut() + Send + '_) = &mut job;
    // SAFETY: the borrow is made `'static` only to cross to the core's
    // thread; this call waits for the job to finish, or the core to stop
    // with it on a stack that never runs again, before `job` goes.
    let job: *mut (dyn FnMut() + Send + 'static) = unsafe { core::mem::transmute(job) };
    let mut g = state();
    if g.terminated {
        return None;
    }
    let done = g.job_ends;
    g.job = Some(Job(job));
    CORE_CV.notify_all();
    loop {
        if g.terminated {
            return None;
        }
        let answered;
        (g, answered) = answer_here(g, |a| a == Asker::Job || rings);
        if answered {
            continue;
        }
        if g.job_ends > done {
            break;
        }
        g = wait(&RUNTIME_CV, g);
    }
    drop(g);
    Some(out.expect("the core's thread ran the call"))
}

/// `f` on this thread while the core's runs nothing.
pub(super) fn while_idle<R>(f: impl FnOnce() -> R) -> R {
    let _hold = hold();
    let mut g = state();
    g.idlers += 1;
    while g.busy {
        g = wait(&RUNTIME_CV, g);
    }
    drop(g);
    struct Release;
    impl Drop for Release {
        fn drop(&mut self) {
            state().idlers -= 1;
            CORE_CV.notify_all();
        }
    }
    let _release = Release;
    f()
}

#[cfg(test)]
#[path = "thread.test.rs"]
mod test;
