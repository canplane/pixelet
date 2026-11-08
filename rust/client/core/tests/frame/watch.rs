//! The harness's own clock. It tracks what the harness waits on now, and a thread ends the
//! process as failed, saying why, once one wait outlasts `WAIT` or the whole run outlasts
//! `RUN`. A hang then fails the run instead of holding the device, and the GPU's lock with
//! it, forever.

use std::sync::Mutex;
use std::time::{Duration, Instant};

/// The longest one wait may take: a turn rung, a call answered, the device's work done.
const WAIT: Duration = Duration::from_secs(60);

/// The longest the whole run may take.
const RUN: Duration = Duration::from_secs(600);

/// How often the clock checks the wait.
const LOOK: Duration = Duration::from_millis(250);

/// What is waited on, and since when.
static WAITING: Mutex<Option<(Instant, String)>> = Mutex::new(None);

/// Records that the harness waits on `what` from now.
pub fn waiting(what: String) {
    *WAITING.lock().unwrap_or_else(|e| e.into_inner()) = Some((Instant::now(), what));
}

pub fn start() {
    let began = Instant::now();
    std::thread::spawn(move || loop {
        std::thread::sleep(LOOK);
        let now = waiting_now();
        let over = match &now {
            Some((since, what)) if since.elapsed() > WAIT => {
                Some(format!("{what}: no end in {WAIT:?}"))
            }
            _ if began.elapsed() > RUN => Some(format!(
                "the run took past {RUN:?}, waiting on {}",
                now.map_or_else(|| "nothing".to_string(), |(_, what)| what)
            )),
            _ => None,
        };
        if let Some(why) = over {
            eprintln!("frames failed: {why}");
            std::process::exit(1);
        }
    });
}

fn waiting_now() -> Option<(Instant, String)> {
    WAITING.lock().unwrap_or_else(|e| e.into_inner()).clone()
}
