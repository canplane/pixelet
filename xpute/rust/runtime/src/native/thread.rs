// xpute-runtime/native/thread.rs

//! This is the core's own thread, which runs a turn at a display frame and
//! sleeps otherwise.
//!
//! The platform's display link calls `vsync` at each frame. A frame begins a
//! turn when one is wanted and the pacing admits it (`pacing`): the core asked
//! for one, at once or for a time now past, or the runtime or a device did
//! (`wake`). The frame's grant is queued for the core as SYS_FRAME and
//! SYS_TURN by the runtime's thread (`serve`), which kicks the turn once they
//! are there (`kick`). While paused, a kick waits for
//! `resume`. A caller's `run` runs a turn at once, whatever the display. A
//! caller's work (`run_on`), such as a device's, runs on this thread between
//! turns while the caller waits.
//!
//! While paused (`pause`), the thread runs only the turns a caller runs, and
//! the one a pause begins to take what waits for the core.
//!
//! What the core sends is served by the runtime's own thread (`serve`), which
//! hears here when a turn has ended. The core's calls are answered on a thread outside the core. A call made
//! by a caller's work is handed to that caller. A call made by a turn is
//! handed to a caller waiting in `run` if there is one; otherwise the
//! program is told (`notify_on`) and answers it on its own thread
//! (`answer`). The core's thread waits meanwhile.
//!
//! Three rules hold the loop:
//!
//! - No turn asked for is lost. A `run` and a frame set their mark under
//!   `LOOP`'s lock and wake the thread. The thread reads the marks and its
//!   work under that lock before it waits, and `Condvar::wait` gives the lock
//!   up as it sleeps, so a mark either lands before the check or wakes the
//!   wait.
//! - Every wait has someone to answer it. The core's thread waits for an
//!   answer only while a call is in `LOOP`. A caller waiting in `run`, or in
//!   `run_on` on the thread that runs turns, answers the call there; when none
//!   waits, the program is told and answers it on its own thread. The
//!   runtime's thread waits on the core only in `run` and `run_on`, and
//!   answers calls in both, so the two threads never wait on each other.
//! - Every turn receives what waits for the core first, applying it before
//!   any phase's budget (xpute-core io/door.rs `receive`), so a turn the
//!   core asked for itself also applies what the runtime sent.

use core::cell::Cell;
use core::panic::AssertUnwindSafe;
use std::sync::{Condvar, Mutex, MutexGuard, PoisonError};
use std::time::{Duration, Instant};

pub(super) use super::pacing::Grant;
pub use super::pacing::Pace;
use super::pacing::Pacing;
use xpute_core::global::on_marked_thread;
use xpute_core::io::frame::{cmd_major, Frame, PROGRAM_MAJOR, WORD_BYTES};

/// The core's thread calls this as it loops: it runs a turn with a deadline
/// and answers when the core wants the next (`xpute_core::entry::Entry`).
pub type Turn = fn(deadline_ms: f64) -> f64;

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
    /// A turn made the call, and the thread that runs turns answers it.
    Turn,
}

struct Loop {
    /// A caller or a frame asked for a turn.
    asked: bool,
    /// The next turn's deadline is the latest any `run` has given since the
    /// last turn began.
    deadline: f64,
    pacing: Pacing,
    /// When the frame a pending turn was granted at wants that turn ended, on
    /// the clock's ms; none when no frame's turn is pending (`kick`).
    frame_end: Option<f64>,
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
    call: Option<(u32, Asker)>,
    ret: Option<i32>,
    /// This counts the callers waiting in `run`.
    runner_cnt: u32,
    /// This is the turn those callers wait for, or the one a pause lets take
    /// what waits for the core, counted in `start_cnt`. While paused, turns
    /// begin only until that one has begun.
    run_until: u64,
    /// Only a turn a caller runs may begin (`pause`, `resume`).
    paused: bool,
    terminated: bool,
}

static LOOP: Mutex<Loop> = Mutex::new(Loop {
    asked: false,
    deadline: 0.0,
    pacing: Pacing::new(),
    frame_end: None,
    want: false,
    wake_at: None,
    wake_ms: 0.0,
    job: None,
    job_end_cnt: 0,
    start_cnt: 0,
    end_cnt: 0,
    call: None,
    ret: None,
    runner_cnt: 0,
    run_until: 0,
    paused: false,
    terminated: false,
});

/// The core's thread waits on this for a `run` or a frame, for work, or for an
/// answer.
static CORE_CV: Condvar = Condvar::new();
/// Callers wait on this for a turn that ended, for work done, for a call to
/// answer, or for the core to stop.
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

pub(super) fn notify() {
    let notify = *NOTIFY.lock().unwrap_or_else(PoisonError::into_inner);
    if let Some(f) = notify {
        f();
    }
}

thread_local! {
    /// A `run` or a `run_on` is under way on this thread.
    #[cfg_attr(target_os = "android", allow(clippy::missing_const_for_thread_local, reason = "already const; the lint misreads Android's expansion"))]
    static IN_CALL: Cell<bool> = const { Cell::new(false) };
    /// This thread runs the core's turns, so it answers a turn's calls.
    #[cfg_attr(target_os = "android", allow(clippy::missing_const_for_thread_local, reason = "already const; the lint misreads Android's expansion"))]
    static RUNS: Cell<bool> = const { Cell::new(false) };
    /// This is set on the core's thread while it runs a caller's work rather
    /// than a turn.
    #[cfg_attr(target_os = "android", allow(clippy::missing_const_for_thread_local, reason = "already const; the lint misreads Android's expansion"))]
    static IN_JOB: Cell<bool> = const { Cell::new(false) };
}

/// Marks this thread as waiting on the core until the value drops.
struct InCall;

impl Drop for InCall {
    fn drop(&mut self) {
        IN_CALL.with(|h| h.set(false));
    }
}

/// Panics where the wait would wait for itself: on the core's thread, or on
/// a thread that already waits on the core.
fn in_call() -> InCall {
    assert!(
        !on_marked_thread(),
        "a call into the core from its own thread would wait for itself"
    );
    assert!(
        !IN_CALL.with(Cell::get),
        "a call into the core inside another on the same thread would wait for itself"
    );
    IN_CALL.with(|h| h.set(true));
    InCall
}

/// Starts the core's thread, which runs `first` before anything else, then
/// loops on `turn`, saying a turn slower than `slow_ms` (`Config::slow_ms`).
pub(super) fn start(
    name: &'static str,
    stack_bytes: usize,
    turn: Turn,
    slow_ms: Option<u32>,
    first: impl FnOnce() + Send,
) {
    std::thread::Builder::new()
        .name(name.into())
        .stack_size(stack_bytes)
        .spawn(move || run_loop(turn, slow_ms))
        .expect("the system would not start the core's thread");
    run_on(first);
}

enum Next {
    Job(Job),
    Turn { deadline_ms: f64, framed: bool },
}

/// The core's thread runs turns and callers' work until a panic unwinds out
/// of one. That panic is the core's stop, which the stop hook has already
/// told (xpute-core stop.rs, `stopped`). The thread then ends the core here,
/// outside the hook (`terminate`), and parks for good, leaving the core's
/// memory as the panic left it, so that the process goes on without the
/// core.
fn run_loop(turn: Turn, slow_ms: Option<u32>) {
    if std::panic::catch_unwind(AssertUnwindSafe(|| run_core(turn, slow_ms))).is_err() {
        terminate();
    }
    loop {
        std::thread::park();
    }
}

fn run_core(turn: Turn, slow_ms: Option<u32>) {
    loop {
        let next = next();
        match next {
            Next::Job(Job(job)) => {
                IN_JOB.with(|j| j.set(true));
                // SAFETY: the caller's closure, which it keeps until it has
                // seen `job_end_cnt` move.
                unsafe { (*job)() };
                IN_JOB.with(|j| j.set(false));
                let mut g = state();
                g.job_end_cnt += 1;
                RUNTIME_CV.notify_all();
            }
            Next::Turn {
                deadline_ms,
                framed,
            } => {
                let t0 = slow_ms.map(|slow| (slow, Instant::now()));
                let wake = turn(deadline_ms);
                if let Some((slow, t0)) = t0 {
                    let ms = t0.elapsed().as_millis();
                    if ms > u128::from(slow) {
                        super::say(&format!("[xpute] turn took {ms} ms"));
                    }
                }
                let mut g = state();
                g.end_cnt += 1;
                g.wake_at = (wake > 0.0).then(|| Instant::now() + duration_of(wake));
                g.wake_ms = wake;
                g.want |= wake == 0.0;
                if framed {
                    g.pacing.asked_next(wake == 0.0);
                }
                RUNTIME_CV.notify_all();
                drop(g);
                super::serve::news();
            }
        }
    }
}

fn duration_of(ms: f64) -> Duration {
    Duration::try_from_secs_f64(ms / 1000.0).unwrap_or(Duration::MAX)
}

/// Waits for what the thread runs next. A caller's work goes first. A turn
/// follows once a frame or a `run` asks for one, if the thread is either not
/// paused or run by a caller.
fn next() -> Next {
    let mut g = state();
    loop {
        let runs = !g.paused || g.start_cnt < g.run_until;
        if let Some(job) = g.job.take() {
            return Next::Job(job);
        }
        if runs && g.asked {
            g.asked = false;
            g.wake_at = None;
            let frame_end = g.frame_end.take();
            let left_ms = frame_end.map_or(0.0, |end| end - super::now());
            let deadline_ms = core::mem::take(&mut g.deadline).max(left_ms);
            g.start_cnt += 1;
            return Next::Turn {
                deadline_ms,
                framed: frame_end.is_some(),
            };
        }
        g = wait(&CORE_CV, g);
    }
}

fn ask_turn(g: &mut Loop, deadline_ms: f64) {
    g.asked = true;
    g.deadline = g.deadline.max(deadline_ms);
    CORE_CV.notify_all();
}

/// Asks for a turn at the next frame, from whichever thread learned that the
/// core has something to take: the runtime after a frame for the core, or a
/// device at a completion.
pub fn wake() {
    state().want = true;
}

/// An input came now, so the turns are interactive for a while (`Pace`).
pub fn input() {
    state().pacing.input(super::now());
}

/// The program's numbers for frame pacing, given as the runtime boots.
pub fn pace(pace: Pace) {
    state().pacing.set(pace);
}

/// The core asked for turns no closer than `interval_ms` (SYS_PACE).
pub(super) fn ask(interval_ms: f64) {
    state().pacing.ask(interval_ms);
}

/// Kicks the turn a frame was granted at, once the runtime's thread has
/// queued its grant for the core; it is to end by `end_at`, on the clock's
/// ms.
pub(super) fn kick(end_at: f64) {
    let mut g = state();
    g.asked = true;
    g.frame_end = Some(g.frame_end.map_or(end_at, |end| end.max(end_at)));
    CORE_CV.notify_all();
}

/// Whether the runtime may queue a frame's grant for the core: turns run.
pub(super) fn running() -> bool {
    !state().paused
}

/// The platform's display link calls this at each display frame, `age_ms` after
/// that frame's time.
pub fn vsync(age_ms: f64) {
    let at = super::now() - age_ms;
    // The lock is let go before the grant goes on, since the runtime's thread
    // takes it to begin the turn.
    let grant = granted(&mut state(), at);
    if let Some(grant) = grant {
        super::serve::frame(grant);
    }
}

/// The grant the frame at `at` begins a turn with, if it begins one. A frame
/// that comes while a frame's turn is pending gives that turn its later
/// deadline instead.
fn granted(g: &mut Loop, at: f64) -> Option<Grant> {
    g.pacing.frame(at);
    if g.paused {
        return None;
    }
    let grant = g.pacing.grant(at, super::now());
    if let Some(end) = g.frame_end.as_mut() {
        *end = end.max(grant.deadline_at);
        return None;
    }
    let due = g.want || g.wake_at.is_some_and(|w| w <= Instant::now());
    if !due || !g.pacing.admit(at) {
        return None;
    }
    g.want = false;
    Some(grant)
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

/// From now on, only a turn a caller runs (`run`) may begin. A caller's
/// work still runs. What waits for the core still reaches it: one turn begins
/// at once to take it, as SYS_SUSPEND must reach the core before the platform
/// freezes the process.
pub fn pause() {
    let waiting = super::serve::waiting();
    let mut g = state();
    g.paused = true;
    if waiting {
        g.run_until = g.run_until.max(g.start_cnt + 1);
        ask_turn(&mut g, 0.0);
    }
}

/// Turns begin again at the display's frames, the next frame beginning one.
pub fn resume() {
    let mut g = state();
    g.paused = false;
    g.want = true;
    // A kick that came before the pause is still pending.
    CORE_CV.notify_all();
}

/// What answers a call on the core's own thread, given its frame's offset.
type Handler = fn(at: u32) -> i32;

/// What answers the core's calls of a major on the core's own thread
/// (`handle`), by the major.
static HANDLERS: Mutex<Vec<(u32, Handler)>> = Mutex::new(Vec::new());

/// Answers each call the core makes of a command under `major`, one of the
/// program's (at or above PROGRAM_MAJOR), with `answer`, on the core's thread
/// before the call returns, given the call's frame by its offset; the
/// program's answerer hears none of them. A major handled again takes the
/// later `answer`.
pub fn handle(major: u32, answer: Handler) {
    assert!(
        major >= PROGRAM_MAJOR,
        "major {major:#x} is xpute's own, below PROGRAM_MAJOR"
    );
    let mut handlers = HANDLERS.lock().unwrap_or_else(PoisonError::into_inner);
    handlers.retain(|&(m, _)| m != major);
    handlers.push((major, answer));
}

/// The core makes a call on its own thread. A call of a handled major is
/// answered here (`handle`); any other is handed to its answerer, which runs
/// the runtime's answer while the core waits.
pub(super) fn call(at: u32) -> i32 {
    let mem = xpute_core::mem::core();
    // A frame that is not the core's, or at offset 0, is the core's bug.
    let ours = super::serve::in_core(mem, at, WORD_BYTES);
    xpute_kit::ensure!(ours, EFAULT, at);
    let major = cmd_major(Frame::at(mem, at).cmd);
    let handlers = HANDLERS.lock().unwrap_or_else(PoisonError::into_inner);
    if let Some(&(_, answer)) = handlers.iter().find(|&&(m, _)| m == major) {
        drop(handlers);
        return answer(at);
    }
    drop(handlers);
    let asker = if IN_JOB.with(Cell::get) {
        Asker::Job
    } else {
        Asker::Turn
    };
    let mut g = state();
    g.call = Some((at, asker));
    g.ret = None;
    RUNTIME_CV.notify_all();
    let tell = asker == Asker::Turn && g.runner_cnt == 0;
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
    RUNS.with(|r| r.set(true));
    let g = state();
    let _ = answer_here(g, |a| a == Asker::Turn);
}

/// Whether the core has stopped.
pub fn terminated() -> bool {
    state().terminated
}

/// Runs a turn that ends no sooner than `deadline_ms` after it starts, and
/// waits until a turn begun after this call has ended, answering the turns'
/// calls meanwhile. Returns when the core wants the next turn, in ms, or
/// `None` if the core has stopped.
pub fn run(deadline_ms: f64) -> Option<f64> {
    let _call = in_call();
    let mut g = state();
    if g.terminated {
        return None;
    }
    RUNS.with(|r| r.set(true));
    let want = g.start_cnt + 1;
    ask_turn(&mut g, deadline_ms);
    g.runner_cnt += 1;
    g.run_until = g.run_until.max(want);
    let out = loop {
        if g.terminated {
            break None;
        }
        let answered;
        (g, answered) = answer_here(g, |a| a == Asker::Turn);
        if answered {
            continue;
        }
        if g.end_cnt >= want {
            break Some(g.wake_ms);
        }
        g = wait(&RUNTIME_CV, g);
    };
    g.runner_cnt -= 1;
    if g.runner_cnt == 0 {
        g.run_until = g.start_cnt;
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
    RUNTIME_CV.notify_all();
    drop(g);
    super::serve::news();
}

pub(super) fn run_on<R: Send>(f: impl FnOnce() -> R + Send) -> Option<R> {
    let _call = in_call();
    let _one = JOBS.lock().unwrap_or_else(PoisonError::into_inner);
    let runs = RUNS.with(Cell::get);
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
        (g, answered) = answer_here(g, |a| a == Asker::Job || runs);
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

#[cfg(test)]
#[path = "thread.test.rs"]
mod test;
