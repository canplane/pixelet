// xpute-runtime/native/thread.rs

//! This is the core's own thread, a vCPU that runs a turn (`Vcpu::turn`) at
//! a display frame and sleeps otherwise.
//!
//! The platform's display link calls `vsync` at each frame. A frame begins a
//! turn when one is wanted and the pacing admits it (`pacing`): the core asked
//! for one, at once or for a time now past, or the runtime or a device did
//! (`wake`). So what the runtime's thread is busy with moves no turn. The turn
//! is handed the frame's numbers (`xpute_core::sched::frame`). A ring (`ring`)
//! runs a turn at once, whatever the display. A caller's work (`run`), such as
//! a device's, runs on this thread between turns while the caller waits.
//!
//! While paused (`pause`), the thread runs only the turns a caller rings.
//!
//! The core's calls are answered on a thread outside the core. A call made
//! by a caller's work is handed to that caller. A call made by a turn is
//! handed to a caller waiting in `ring` if there is one; otherwise the
//! program is told (`notify_on`) and answers it on its own thread
//! (`answer`). The core's thread waits meanwhile, as a vCPU waits out an
//! exit.
//!
//! Four rules hold the loop:
//!
//! - No turn asked for is lost. A ring and a frame set their mark under
//!   `LOOP`'s lock and wake the thread. The thread reads the marks and its
//!   work under that lock before it waits, and `Condvar::wait` gives the lock
//!   up as it sleeps, so a mark either lands before the check or wakes the
//!   wait.
//! - Every wait has someone to answer it. The core's thread waits for an
//!   answer only while a call is in `LOOP`. A caller waiting in `ring`, or
//!   in `run` on the thread that rings, answers the call there; when none
//!   waits, the program is told and answers it on its own thread. The
//!   runtime's thread waits on the core only in `ring` and `run`, and
//!   answers calls in both, so the two threads never wait on each other.
//! - A turn begins only when the core can post all that one turn may post
//!   (`Vcpu::ready`, the door's room in the completion ring), read under the
//!   lock. When it cannot, the thread tells a caller in `ring` and the
//!   runtime, and the next frame after the runtime's read tries again.
//! - Every turn drains the submission ring first, applying what waits there
//!   before any phase's budget (xpute-core ipc/door.rs `receive`), so a turn
//!   the core asked for itself also applies what the runtime sent.

use core::cell::Cell;
use core::panic::AssertUnwindSafe;
use std::sync::{Condvar, Mutex, MutexGuard, PoisonError};
use std::time::{Duration, Instant, SystemTime, UNIX_EPOCH};

use xpute_core::global::on_marked_thread;
use xpute_core::sched::frame::{Frame, Pace};

use super::pacing::{Grant, Pacing};

/// The core's thread calls these as it loops: `turn` runs a turn with a
/// deadline and answers when the core wants the next
/// (`xpute_core::entry::Entry`), and `ready` says whether a turn may begin.
#[derive(Clone, Copy)]
pub struct Vcpu {
    pub turn: fn(deadline_ms: f64) -> f64,
    pub ready: fn() -> bool,
}

/// A job is a caller's work, borrowed from the caller, which waits until it has
/// run.
struct Job(*mut (dyn FnMut() + Send + 'static));

// SAFETY: the closure it points at is `Send`, and only the core's thread
// runs it, while the caller that owns it waits.
unsafe impl Send for Job {}

/// What made a call decides who answers it.
#[derive(Clone, Copy, PartialEq, Eq)]
enum Asker {
    /// A caller's work made the call, and that caller answers it.
    Job,
    /// A turn made the call, and the thread that rings answers it.
    Turn,
}

struct Loop {
    /// A caller rang a turn.
    rung: bool,
    /// The next turn's deadline is the latest any ring has given since the
    /// last turn began.
    deadline: f64,
    pacing: Pacing,
    /// The display frame the next turn begins at.
    frame: Option<Grant>,
    /// The next frame begins a turn: a turn answered 0, or the runtime or a
    /// device asked for one.
    want: bool,
    /// This is when the last turn asked to be woken, if that is later than
    /// now. The first frame past it begins a turn.
    wake_at: Option<Instant>,
    /// What the last turn answered.
    wake_ms: f64,
    job: Option<Job>,
    job_end_cnt: u64,
    start_cnt: u64,
    end_cnt: u64,
    /// `look_cnt` counts the times the thread checked whether a turn may begin;
    /// `stall_at` is the count at the last check that found it could not.
    look_cnt: u64,
    stall_at: u64,
    call: Option<(u32, Asker)>,
    ret: Option<i32>,
    /// This counts the callers waiting in `ring`.
    ringer_cnt: u32,
    /// This is the turn those callers wait for, counted in `start_cnt`. While
    /// paused, turns begin only until that one has begun.
    ring_until: u64,
    /// This counts the callers in `while_idle`.
    idler_cnt: u32,
    /// This is set while the core's thread runs a turn or a caller's work.
    busy: bool,
    /// Only a turn a caller rings may begin (`pause`, `resume`).
    paused: bool,
    /// The runtime has been told and has not heard since (`heard`), so it is
    /// not told again.
    told: bool,
    terminated: bool,
}

static LOOP: Mutex<Loop> = Mutex::new(Loop {
    rung: false,
    deadline: 0.0,
    pacing: Pacing::new(),
    frame: None,
    want: false,
    wake_at: None,
    wake_ms: 0.0,
    job: None,
    job_end_cnt: 0,
    start_cnt: 0,
    end_cnt: 0,
    look_cnt: 0,
    stall_at: 0,
    call: None,
    ret: None,
    ringer_cnt: 0,
    ring_until: 0,
    idler_cnt: 0,
    busy: false,
    paused: false,
    told: false,
    terminated: false,
});

/// The core's thread waits on this for a ring or a frame, for work, for an answer, or
/// for the last `while_idle` to end.
static CORE_CV: Condvar = Condvar::new();
/// Callers wait on this for a turn that ended or stalled, for work done, for
/// a call to answer, or for the core to go idle or stop.
static RUNTIME_CV: Condvar = Condvar::new();

/// This lets one caller's work through at a time.
static JOBS: Mutex<()> = Mutex::new(());

fn state() -> MutexGuard<'static, Loop> {
    LOOP.lock().unwrap_or_else(PoisonError::into_inner)
}

fn wait<'a>(cv: &Condvar, g: MutexGuard<'a, Loop>) -> MutexGuard<'a, Loop> {
    cv.wait(g).unwrap_or_else(PoisonError::into_inner)
}

/// This is the runtime's function that answers a call, and it runs on the
/// answering thread.
pub type Answer = extern "C" fn(at: u32) -> i32;

static ANSWER: Mutex<Option<Answer>> = Mutex::new(None);

/// Makes `answer` answer the core's calls from the next call on. With none,
/// every call fails with ENOSYS.
pub fn answer_with(answer: Option<Answer>) {
    *ANSWER.lock().unwrap_or_else(PoisonError::into_inner) = answer;
}

fn answer_of(at: u32) -> i32 {
    let answer = *ANSWER.lock().unwrap_or_else(PoisonError::into_inner);
    answer.map_or(xpute_kit::status::errno::Errno::ENOSYS as i32, |f| f(at))
}

/// Tells the runtime, from the core's thread or a device's, one of three
/// pieces of news:
/// the core has posted or asked for a turn since the runtime last read
/// `turns`; a turn's call waits for its answer (`answer`); or the core has
/// stopped. It must not call into the core; it hands the news to the
/// runtime's thread.
pub type Notify = extern "C" fn();

static NOTIFY: Mutex<Option<Notify>> = Mutex::new(None);

/// Makes `notify` tell the runtime from the next piece of news on. With
/// none, nothing is told.
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
    /// This is set on the core's thread while it runs a caller's work rather
    /// than a turn.
    #[cfg_attr(target_os = "android", allow(clippy::missing_const_for_thread_local, reason = "already const; the lint misreads Android's expansion"))]
    static IN_JOB: Cell<bool> = const { Cell::new(false) };
}

/// Marks this thread as waiting on the core until the value drops.
struct Hold;

impl Drop for Hold {
    fn drop(&mut self) {
        IN_CALL.with(|h| h.set(false));
    }
}

/// Panics where the wait would wait for itself: on the core's thread, or on
/// a thread that already waits on the core.
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

enum Next {
    Job(Job),
    Turn {
        deadline_ms: f64,
        frame: Option<Grant>,
    },
}

/// The core's thread runs turns and callers' work until a panic unwinds out
/// of one. That panic is the core's stop, which the stop hook has already
/// told (xpute-core stop.rs, `stopped`). The thread then ends the core here,
/// outside the hook (`terminate`), and parks for good, leaving the core's
/// memory as the panic left it, so that the process goes on without the
/// core.
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
                // seen `job_end_cnt` move.
                unsafe { (*job)() };
                IN_JOB.with(|j| j.set(false));
                let mut g = state();
                g.busy = false;
                g.job_end_cnt += 1;
                RUNTIME_CV.notify_all();
            }
            Next::Turn { deadline_ms, frame } => {
                let framed = frame.is_some();
                if let Some(grant) = frame {
                    xpute_core::sched::frame::give(frame_of(grant));
                }
                let wake = (vcpu.turn)(deadline_ms);
                let mut g = state();
                g.busy = false;
                g.end_cnt += 1;
                g.wake_at = (wake > 0.0).then(|| Instant::now() + duration_of(wake));
                g.wake_ms = wake;
                g.want |= wake == 0.0;
                if framed {
                    g.pacing.asked_next(wake == 0.0);
                }
                RUNTIME_CV.notify_all();
                let tell = g.ringer_cnt == 0 && !core::mem::replace(&mut g.told, true);
                drop(g);
                if tell {
                    notify();
                }
            }
        }
    }
}

fn frame_of(grant: Grant) -> Frame {
    Frame {
        unix_ms: SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .map_or(0.0, |d| d.as_secs_f64() * 1000.0),
        refresh_ms: grant.refresh_ms,
        paced_ms: grant.paced_ms,
        interactive: grant.interactive,
    }
}

fn duration_of(ms: f64) -> Duration {
    Duration::try_from_secs_f64(ms / 1000.0).unwrap_or(Duration::MAX)
}

/// Waits for what the thread runs next. A caller's work goes first. A turn
/// follows once a frame or a ring asks for one, if a turn may begin and the
/// thread is either not paused or rung by a caller. A frame that finds no room
/// in the completion ring is given up, and the next one tries again.
fn next(vcpu: Vcpu) -> Next {
    let mut g = state();
    loop {
        let runs = !g.paused || g.start_cnt < g.ring_until;
        if g.idler_cnt == 0 {
            if let Some(job) = g.job.take() {
                g.busy = true;
                return Next::Job(job);
            }
            if runs && (g.rung || g.frame.is_some()) {
                g.look_cnt += 1;
                g.rung = false;
                g.wake_at = None;
                if (vcpu.ready)() {
                    let frame = g.frame.take();
                    let left_ms = frame.map_or(0.0, |f| f.deadline_at - super::now());
                    let deadline_ms = core::mem::take(&mut g.deadline).max(left_ms);
                    g.start_cnt += 1;
                    g.busy = true;
                    return Next::Turn { deadline_ms, frame };
                }
                g.frame = None;
                g.want = true;
                g.stall_at = g.look_cnt;
                RUNTIME_CV.notify_all();
                if g.ringer_cnt == 0 && !core::mem::replace(&mut g.told, true) {
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

fn ring_locked(g: &mut Loop, deadline_ms: f64) {
    g.rung = true;
    g.deadline = g.deadline.max(deadline_ms);
    CORE_CV.notify_all();
}

/// Asks for a turn at the next frame, from whichever thread learned that the
/// core has something to take: the runtime after a send or a read of the
/// completion ring, or a device at a completion.
pub fn wake() {
    state().want = true;
}

/// An input came now, so the turns are interactive for a while (`Pace`).
pub fn input() {
    state().pacing.input(super::now());
}

/// The core sets the pacing of its turns (`Form::pace`).
pub fn pace(pace: Pace) {
    state().pacing.set(pace);
}

/// The platform's display link calls this at each display frame, `age_ms` after
/// that frame's time. A frame that comes while a turn runs takes the place of
/// one already given, so the turn after it has the later deadline.
pub fn vsync(age_ms: f64) {
    let at = super::now() - age_ms;
    let mut g = state();
    let g = &mut *g;
    g.pacing.frame(at);
    if g.paused {
        return;
    }
    if g.frame.is_none() {
        let due = g.want || g.wake_at.is_some_and(|w| w <= Instant::now());
        if !due || !g.pacing.admit(at) {
            return;
        }
    }
    g.want = false;
    g.frame = Some(g.pacing.grant(at, super::now()));
    CORE_CV.notify_all();
}

/// Answers here the call waiting in `LOOP`, if `takes` says this thread is
/// its answerer, and says whether it did. The lock `g` is let go while the
/// answer runs, then handed back.
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

/// From now on, only a turn a caller rings (`ring`) may begin. A caller's
/// work still runs.
pub fn pause() {
    let mut g = state();
    g.paused = true;
    g.frame = None;
}

/// Turns begin again at the display's frames, the next frame beginning one.
pub fn resume() {
    let mut g = state();
    g.paused = false;
    g.want = true;
}

/// The core makes a call on its own thread. The call is handed to its
/// answerer, which runs the runtime's answer while the core waits.
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
    let tell = asker == Asker::Turn && g.ringer_cnt == 0;
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

/// Answers, on this thread of the program's, the turn's call that waits, if
/// one does; this is the call the program's `Notify` was told of.
pub fn answer() {
    RINGS.with(|r| r.set(true));
    let g = state();
    let _ = answer_here(g, |a| a == Asker::Turn);
}

/// The runtime's thread has heard the news (`Notify`), and is told again of
/// what comes after. Answers whether the core has stopped.
pub fn heard() -> bool {
    let mut g = state();
    g.told = false;
    g.terminated
}

/// Rings a turn that ends no sooner than `deadline_ms` after its rise, and
/// waits until a turn begun after this call has ended, answering the turns'
/// calls meanwhile. Returns when the core wants the next turn, in ms. It
/// returns early if no turn may begin until the runtime reads the completion
/// ring, and returns `None` if the core has stopped.
pub fn ring(deadline_ms: f64) -> Option<f64> {
    let _hold = hold();
    let mut g = state();
    if g.terminated {
        return None;
    }
    RINGS.with(|r| r.set(true));
    let want = g.start_cnt + 1;
    let looked = g.look_cnt;
    ring_locked(&mut g, deadline_ms);
    g.ringer_cnt += 1;
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
        if g.stall_at > looked {
            break Some(0.0);
        }
        if g.end_cnt >= want {
            break Some(g.wake_ms);
        }
        g = wait(&RUNTIME_CV, g);
    };
    g.ringer_cnt -= 1;
    if g.ringer_cnt == 0 {
        g.ring_until = g.start_cnt;
    }
    out
}

/// The stop hook calls this on the core's thread, inside the panic hook,
/// once the core has stopped (say.rs `stop_hook`). Where panics unwind, it
/// returns, and the thread ends the core outside the hook (`run_loop`).
pub(super) fn stopped() {
    // Where panics abort, nothing unwinds out of the hook, and returning from
    // it would end the process. The process must outlive its core to show the
    // stop's report, so the core is ended here and its thread stays inside the
    // hook for good. Nothing in such a program replaces the hook afterwards.
    #[cfg(panic = "abort")]
    {
        terminate();
        loop {
            std::thread::park();
        }
    }
}

/// Ends the core, from its own thread: every waiting caller is told, and so
/// is the runtime.
fn terminate() {
    let mut g = state();
    g.terminated = true;
    g.busy = false;
    g.told = true;
    RUNTIME_CV.notify_all();
    drop(g);
    notify();
}

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
    let ends = g.job_end_cnt;
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
        if g.job_end_cnt > ends {
            break;
        }
        g = wait(&RUNTIME_CV, g);
    }
    drop(g);
    Some(out.expect("the core's thread ran the call"))
}

pub(super) fn while_idle<R>(f: impl FnOnce() -> R) -> R {
    let _hold = hold();
    let mut g = state();
    g.idler_cnt += 1;
    while g.busy {
        g = wait(&RUNTIME_CV, g);
    }
    drop(g);
    struct Release;
    impl Drop for Release {
        fn drop(&mut self) {
            state().idler_cnt -= 1;
            CORE_CV.notify_all();
        }
    }
    let _release = Release;
    f()
}

#[cfg(test)]
#[path = "thread.test.rs"]
mod test;
