// xpute-native-runtime/native.boot.test.rs

//! A server that fails as the runtime boots traps the link before the guest
//! is entered (native.rs `run`).

use std::sync::atomic::{AtomicBool, Ordering};

use xpute_kit::status::errno::Errno;
use xpute_link::{OPEN, ROOT};
use xpute_native_runtime::native::Boot;

#[path = "app.test.rs"]
mod app;

use app::End;

static ENTERED: AtomicBool = AtomicBool::new(false);

xpute_guest::link!(
    4096,
    || ENTERED.store(true, Ordering::Relaxed),
    || {},
    || false
);

fn boot(_: std::path::PathBuf) -> Boot {
    Boot {
        stack_sz: 1 << 20,
        servers: vec![Box::new(|_| panic!("the server fails"))],
        imports: &[],
        later: &[],
    }
}

#[test]
fn failing_server_traps() {
    let ui = app::start(boot, &[]);
    let mut messages = Vec::new();
    app::post(ui, 1, ROOT, OPEN, Some(app::lowered(&[])));
    assert_eq!(app::answer_to(ui, 1, &mut messages), Errno::EPIPE as i64);
    let told = app::ends();
    assert!(
        matches!(&told[..], [End::Stop(e, _), End::Trap(err)]
            if *e == Errno::ENOTRECOVERABLE as i32
                && err.starts_with("the host stopped: panicked at ")
                && err.ends_with("the server fails")),
        "{told:?}"
    );
    assert!(
        !ENTERED.load(Ordering::Relaxed),
        "the guest is never entered"
    );
}
