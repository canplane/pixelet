// xpute-host/native/thread.rs

//! The guest's own thread, a vCPU that runs while the guest has work: it runs
//! a turn (`Vcpu::turn`) whenever it is kicked, and sleeps otherwise. A kick
//! carries the least quota the next turn is given; a turn no kick gave one is
//! given the last quota a kick gave (the slice). A caller's work (`run`), such
//! as a device's, runs on it between turns while the caller waits. The thread
//! keeps no timer: when a turn asks to be woken, at once or in `n` ms, it is
//! told the program (`ack`, `ring`), whose strobe asks its policy for the
//! moment, as a page's does (@xpute/host sched/strobe.ts), and kicks then, so
//! no turn runs between a display's frames. Paused (`pause`), it runs only
//! the turns a caller rings, and its kicks wait for `resume`.
//!
//! The guest's hostcalls are answered on a thread outside it. A hostcall made
//! by a caller's work is handed to that caller. One made by a turn is handed
//! to a caller waiting in `ring` where there is one; otherwise it is told the
//! program (`notify_on`), which answers it on its own thread (`answer`),
//! while the guest's thread waits, as a vCPU waits out an exit.
//!
//! Three rules hold the loop:
//!
//! - No kick is lost. A kick sets `kicked` under `LOOP`'s lock and wakes the
//!   thread; the thread reads `kicked` and its work under that lock before
//!   it waits, and `Condvar::wait` gives the lock up as it sleeps, so a kick
//!   lands before the check or wakes the wait.
//! - Every wait has its answerer. The guest's thread waits for an answer only
//!   while a hostcall is in `LOOP`: a caller waiting in `ring`, or in `run`
//!   on the thread that rings, answers it there, and with none waiting the
//!   program is told and answers it on that thread. The program's thread
//!   waits on the guest only in `ring` and `run`, and answers in both, so the
//!   two never wait on each other.
//! - A turn begins only where the guest can post what one may (`Vcpu::ready`,
//!   the door's room in the completion ring), read under the lock. Where it
//!   cannot, the thread says so to a caller in `ring` and to the program, and
//!   waits for the kick that follows the program's read.
//! - Every pass drains the submission ring first: a turn applies what waits
//!   there before any phase's budget (xpute-guest ipc/door.rs `receive`), so
//!   a turn the guest asked for itself carries the program's kicks too.

use core::cell::Cell;
use std::sync::{Condvar, Mutex, MutexGuard, PoisonError};
use std::time::{Duration, Instant};

use xpute_guest::global::on_marked_thread;

/// What the guest's thread runs as it loops: a turn of a quota, answering
/// when the guest wants the next (`xpute_guest::entry::Entry`), and whether
/// one may begin.
#[derive(Clone, Copy)]
pub struct Vcpu {
    pub turn: fn(quota_ms: f64) -> f64,
    pub ready: fn() -> bool,
}

/// A caller's work, borrowed: the caller waits until it has run.
struct Job(*mut (dyn FnMut() + Send + 'static));

// SAFETY: the closure it points at is `Send`, and only the guest's thread
// runs it, while the caller that owns it waits.
unsafe impl Send for Job {}

/// Who answers a hostcall.
#[derive(Clone, Copy, PartialEq, Eq)]
enum Asker {
    /// The caller whose work made it.
    Job,
    /// The thread that rings: a turn made it.
    Turn,
}

/// What the program reads of the turns since it last asked (`ack`).
struct Ack {
    /// A turn answered 0: the guest wants the next at once.
    resched: bool,
    /// The longest turn, in ms.
    turn_ms: f64,
    /// The program is told and has not asked since, so it is not told again.
    told: bool,
}

struct Loop {
    kicked: bool,
    /// The next turn's quota, the most a kick since gave.
    quota: Option<f64>,
    /// What a turn no kick gave a quota is given: the last quota a kick gave.
    slice: f64,
    /// When the last turn asked to be woken, past now, as the program is
    /// told it (`wake_of`); one that answered 0 is `Ack::resched`. The thread
    /// waits on it not: the program's kick wakes it.
    wake_at: Option<Instant>,
    job: Option<Job>,
    job_ends: u64,
    starts: u64,
    ends: u64,
    /// The thread's looks at whether a turn may begin, and the last that found
    /// it could not.
    looks: u64,
    stall_at: u64,
    hostcall: Option<(u32, Asker)>,
    ret: Option<i32>,
    /// Callers waiting in `ring`.
    ringers: u32,
    /// The turn those callers wait for, by `starts`: while paused, turns begin
    /// only until it has.
    ring_until: u64,
    /// Callers in `while_idle`.
    idlers: u32,
    /// The guest's thread runs a turn or a caller's work.
    busy: bool,
    /// No turn begins but a caller's in `ring` (`pause`, `resume`).
    paused: bool,
    ack: Ack,
    terminated: bool,
}

static LOOP: Mutex<Loop> = Mutex::new(Loop {
    kicked: false,
    quota: None,
    slice: 0.0,
    wake_at: None,
    job: None,
    job_ends: 0,
    starts: 0,
    ends: 0,
    looks: 0,
    stall_at: 0,
    hostcall: None,
    ret: None,
    ringers: 0,
    ring_until: 0,
    idlers: 0,
    busy: false,
    paused: false,
    ack: Ack {
        resched: false,
        turn_ms: 0.0,
        told: false,
    },
    terminated: false,
});

/// What the guest's thread waits on: a kick, work, an answer, a hold let go.
static GUEST_CV: Condvar = Condvar::new();
/// What a caller waits on: a turn ended or stalled, work done, a hostcall,
/// the guest idle or stopped.
static HOST_CV: Condvar = Condvar::new();

/// One caller's work at a time.
static JOBS: Mutex<()> = Mutex::new(());

fn state() -> MutexGuard<'static, Loop> {
    LOOP.lock().unwrap_or_else(PoisonError::into_inner)
}

fn wait<'a>(cv: &Condvar, g: MutexGuard<'a, Loop>) -> MutexGuard<'a, Loop> {
    cv.wait(g).unwrap_or_else(PoisonError::into_inner)
}

/// The program's answer to a hostcall, run on the answering thread.
pub type Answer = extern "C" fn(at: u32) -> i32;

static ANSWER: Mutex<Option<Answer>> = Mutex::new(None);

/// `answer` answers the guest's hostcalls from the next on; none, ENOSYS.
pub fn answer_with(answer: Option<Answer>) {
    *ANSWER.lock().unwrap_or_else(PoisonError::into_inner) = answer;
}

fn answer_of(at: u32) -> i32 {
    let answer = *ANSWER.lock().unwrap_or_else(PoisonError::into_inner);
    answer.map_or(xpute_kit::status::errno::Errno::ENOSYS as i32, |f| f(at))
}

/// What tells the program, from the guest's thread, that the guest has
/// posted or asked since it last read (`ack`), that a turn's hostcall
/// waits for its answer (`answer`), or that the guest has stopped. It must
/// not call into the guest: it hands the news to the program's thread.
pub type Notify = extern "C" fn();

static NOTIFY: Mutex<Option<Notify>> = Mutex::new(None);

/// `notify` tells the program from the next news on; none, nothing is told.
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
    /// This thread rings the guest, so it answers a turn's hostcalls.
    #[cfg_attr(target_os = "android", allow(clippy::missing_const_for_thread_local, reason = "already const; the lint misreads Android's expansion"))]
    static RINGS: Cell<bool> = const { Cell::new(false) };
    /// On the guest's thread: a caller's work runs, not a turn.
    #[cfg_attr(target_os = "android", allow(clippy::missing_const_for_thread_local, reason = "already const; the lint misreads Android's expansion"))]
    static IN_JOB: Cell<bool> = const { Cell::new(false) };
}

/// This thread waits on the guest until it is let go.
struct Hold;

impl Drop for Hold {
    fn drop(&mut self) {
        IN_CALL.with(|h| h.set(false));
    }
}

/// Refused where waiting would wait for itself: on the guest's thread, or
/// on one that already waits on it.
fn hold() -> Hold {
    assert!(
        !on_marked_thread(),
        "a call into the guest from its own thread would wait for itself"
    );
    assert!(
        !IN_CALL.with(Cell::get),
        "a call into the guest inside another on the same thread would wait for itself"
    );
    IN_CALL.with(|h| h.set(true));
    Hold
}

/// Starts the guest's thread, which runs `first` before anything else, then
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
        .expect("the system would not start the guest's thread");
    run(first);
}

/// What the guest's thread does next: a caller's work, or a turn of a quota.
enum Next {
    Job(Job),
    Turn { quota_ms: f64 },
}

fn run_loop(vcpu: Vcpu) {
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
                HOST_CV.notify_all();
            }
            Next::Turn { quota_ms } => {
                let t0 = Instant::now();
                let wake = (vcpu.turn)(quota_ms);
                let turn_ms = t0.elapsed().as_secs_f64() * 1000.0;
                let mut g = state();
                g.busy = false;
                g.ends += 1;
                g.wake_at = (wake > 0.0).then(|| Instant::now() + duration_of(wake));
                g.ack.resched |= wake == 0.0;
                g.ack.turn_ms = g.ack.turn_ms.max(turn_ms);
                HOST_CV.notify_all();
                let tell = g.ringers == 0 && !core::mem::replace(&mut g.ack.told, true);
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
                    let quota_ms = g.quota.take().unwrap_or(g.slice);
                    g.starts += 1;
                    g.busy = true;
                    return Next::Turn { quota_ms };
                }
                g.stall_at = g.looks;
                HOST_CV.notify_all();
                if g.ringers == 0 && !core::mem::replace(&mut g.ack.told, true) {
                    drop(g);
                    notify();
                    g = state();
                    continue;
                }
            }
        }
        g = wait(&GUEST_CV, g);
    }
}

/// A turn wanted at once, the next given at least `quota` where one is
/// given, from any thread; it returns at once.
fn kick_with(quota: Option<f64>) {
    kick_locked(&mut state(), quota);
}

fn kick_locked(g: &mut Loop, quota: Option<f64>) {
    g.kicked = true;
    if let Some(q) = quota {
        g.quota = Some(g.quota.map_or(q, |p| p.max(q)));
        if q > 0.0 {
            g.slice = q;
        }
    }
    GUEST_CV.notify_all();
}

/// When the guest wants the next turn, in ms from now: 0 where a turn since
/// the program last read answered 0, `NO_WAKE` for none.
fn wake_of(g: &Loop) -> f64 {
    match g.wake_at {
        _ if g.ack.resched => 0.0,
        Some(at) => at.saturating_duration_since(Instant::now()).as_secs_f64() * 1000.0,
        None => xpute_guest::sched::wake::NO_WAKE,
    }
}

/// A device's kick, from whichever thread learned of a completion: the next
/// turn reads it.
pub fn kick() {
    kick_with(None);
}

/// The program's kick, after it has written the submission ring: the next
/// turn given at least `quota_ms`, none of its own where negative. This
/// thread is the one that answers a turn's hostcalls from now on.
pub fn kick_quota(quota_ms: f64) {
    RINGS.with(|r| r.set(true));
    kick_with((quota_ms >= 0.0).then_some(quota_ms));
}

/// The hostcall waiting in `LOOP`, if this thread is its answerer, answered
/// here; whether one was. `g` is the lock, given back.
fn answer_here<'a>(
    mut g: MutexGuard<'a, Loop>,
    takes: impl Fn(Asker) -> bool,
) -> (MutexGuard<'a, Loop>, bool) {
    let Some((at, asker)) = g.hostcall else {
        return (g, false);
    };
    if !takes(asker) {
        return (g, false);
    }
    g.hostcall = None;
    drop(g);
    let res = answer_of(at);
    let mut g = state();
    g.ret = Some(res);
    GUEST_CV.notify_all();
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
    GUEST_CV.notify_all();
}

/// The guest's hostcall, on its own thread: handed to its answerer, which
/// runs the program's answer, the guest waiting.
pub(super) fn hostcall(at: u32) -> i32 {
    let asker = if IN_JOB.with(Cell::get) {
        Asker::Job
    } else {
        Asker::Turn
    };
    let mut g = state();
    g.hostcall = Some((at, asker));
    g.ret = None;
    HOST_CV.notify_all();
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
        g = wait(&GUEST_CV, g);
    }
}

/// The turn's hostcall waiting, if one does, answered on this thread, the
/// program's: what its `Notify` was told of.
pub fn answer() {
    RINGS.with(|r| r.set(true));
    let g = state();
    let _ = answer_here(g, |a| a == Asker::Turn);
}

/// What the turns since the last call asked, for the program's thread: when
/// the guest wants the next, in ms (0 where a turn answered 0, `NO_WAKE` for
/// none, NaN once it has stopped), and the longest of them, in ms.
pub fn ack() -> (f64, f64) {
    let mut g = state();
    g.ack.told = false;
    let wake = if g.terminated { f64::NAN } else { wake_of(&g) };
    g.ack.resched = false;
    (wake, core::mem::take(&mut g.ack.turn_ms))
}

/// A turn given at least `quota_ms`, this thread waiting until one begun
/// after this call has ended, answering the turns' hostcalls meanwhile: when
/// the guest wants the next, in ms. Back early where no turn may begin until
/// the program reads the completion ring; none where the guest has stopped.
pub fn ring(quota_ms: f64) -> Option<f64> {
    let _hold = hold();
    let mut g = state();
    if g.terminated {
        return None;
    }
    RINGS.with(|r| r.set(true));
    let want = g.starts + 1;
    let looked = g.looks;
    kick_locked(&mut g, Some(quota_ms));
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
        g = wait(&HOST_CV, g);
    };
    g.ringers -= 1;
    if g.ringers == 0 {
        g.ring_until = g.starts;
    }
    out
}

/// Terminates the guest, on its own thread: every caller waiting is told, and
/// so is the program, and this thread stays here, with the guest's stack and
/// locks as they are, so the process goes on without it.
pub fn terminate() -> ! {
    let mut g = state();
    g.terminated = true;
    g.ack.told = true;
    HOST_CV.notify_all();
    drop(g);
    notify();
    loop {
        std::thread::park();
    }
}

/// `f` on the guest's thread between turns, the caller waiting; each hostcall
/// it makes meanwhile answered on the caller's, and on the thread that rings
/// a turn's as well. None where the guest has stopped, in this call or
/// before it.
pub(super) fn run<R: Send>(f: impl FnOnce() -> R + Send) -> Option<R> {
    let _hold = hold();
    let _one = JOBS.lock().unwrap_or_else(PoisonError::into_inner);
    let rings = RINGS.with(Cell::get);
    let mut f = Some(f);
    let mut out = None;
    let mut job = || out = f.take().map(|f| f());
    let job: *mut (dyn FnMut() + Send + '_) = &mut job;
    // SAFETY: the borrow is made `'static` only to cross to the guest's
    // thread; this call waits for the job to finish, or the guest to stop
    // with it on a stack that never runs again, before `job` goes.
    let job: *mut (dyn FnMut() + Send + 'static) = unsafe { core::mem::transmute(job) };
    let mut g = state();
    if g.terminated {
        return None;
    }
    let done = g.job_ends;
    g.job = Some(Job(job));
    GUEST_CV.notify_all();
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
        g = wait(&HOST_CV, g);
    }
    drop(g);
    Some(out.expect("the guest's thread ran the call"))
}

/// `f` on this thread while the guest's runs nothing.
pub(super) fn while_idle<R>(f: impl FnOnce() -> R) -> R {
    let _hold = hold();
    let mut g = state();
    g.idlers += 1;
    while g.busy {
        g = wait(&HOST_CV, g);
    }
    drop(g);
    struct Release;
    impl Drop for Release {
        fn drop(&mut self) {
            state().idlers -= 1;
            GUEST_CV.notify_all();
        }
    }
    let _release = Release;
    f()
}

#[cfg(test)]
#[path = "thread.test.rs"]
mod test;
