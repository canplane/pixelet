// xpute-runtime-native/link.calls.test.rs

//! A function without `async` the UI implements across its crossing refuses
//! to start, naming its interface, and the guest is never entered (link.rs
//! `init`).

use std::sync::atomic::{AtomicBool, Ordering};

use xpute_kit::status::errno::Errno;
use xpute_link::{Handle, NAMED, OPEN, ROOT};
use xpute_runtime_native::native::Boot;

#[path = "app.test.rs"]
mod app;

use app::Exit;

static ENTERED: AtomicBool = AtomicBool::new(false);

xpute_guest::link!(
    4096,
    || ENTERED.store(true, Ordering::Relaxed),
    || {},
    || false
);

/// An interface the UI implements, one of whose methods answers at once.
const CALLED: Handle = NAMED | 0x30;

fn boot(_: std::path::PathBuf) -> Boot {
    Boot {
        stack_sz: 1 << 20,
        mounts: Vec::new(),
        imports: &[("called", CALLED)],
        calls: &[(CALLED, 0)],
    }
}

#[test]
fn call_implemented_by_ui_refused() {
    let ui = app::start(boot, &[CALLED]);
    let mut msgs = Vec::new();
    app::post(ui, 1, ROOT, OPEN, Some(app::lowered(&[])));
    assert_eq!(app::answer_to(ui, 1, &mut msgs), Errno::EPIPE as i64);
    assert_eq!(
        app::exits(),
        [Exit::Trap(
            "a function without async is implemented by the UI: called".into()
        )]
    );
    assert!(
        !ENTERED.load(Ordering::Relaxed),
        "the guest is never entered"
    );
}
