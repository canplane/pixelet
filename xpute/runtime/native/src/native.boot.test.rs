// xpute-runtime-native/native.boot.test.rs

//! A mount that fails as the runtime boots traps the link before the guest
//! is entered (native.rs `run`).

use std::sync::atomic::{AtomicBool, Ordering};

use xpute_kit::status::errno::Errno;
use xpute_link::{OPEN, ROOT};
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

fn boot(_: std::path::PathBuf) -> Boot {
    Boot {
        stack_sz: 1 << 20,
        mounts: vec![Box::new(|_| panic!("the mount fails"))],
        imports: &[],
        calls: &[],
    }
}

#[test]
fn failing_mount_traps() {
    let ui = app::start(boot, &[]);
    let mut msgs = Vec::new();
    app::post(ui, 1, ROOT, OPEN, Some(app::lowered(&[])));
    assert_eq!(app::answer_to(ui, 1, &mut msgs), Errno::EPIPE as i64);
    let told = app::exits();
    assert!(
        matches!(&told[..], [Exit::Stop(e, _), Exit::Trap(err)]
            if *e == Errno::ENOTRECOVERABLE as i32
                && err.starts_with("the host stopped: panicked at ")
                && err.ends_with("the mount fails")),
        "{told:?}"
    );
    assert!(
        !ENTERED.load(Ordering::Relaxed),
        "the guest is never entered"
    );
}
