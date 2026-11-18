// xpute-runtime-native/link.test.rs

//! The runtime enters the guest only once every interface its world imports
//! is mounted (link.rs `init`).

use std::rc::Rc;
use std::sync::atomic::{AtomicBool, Ordering};

use xpute_kit::status::errno::Errno;
use xpute_link::{Handle, Lift, Lower, Method, NAMED, OPEN, Object, ROOT, Tag, uri};
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

struct Nothing;

impl Object for Nothing {
    fn call(&self, _: Method, _: Tag, _: Lift, _: &mut Lower) -> i32 {
        0
    }
}

/// An interface implemented under its handle, one whose resources an opener
/// opens, and two with no implementation.
const NAMED_ONE: Handle = NAMED | 0x30;
const OPENED: Handle = NAMED | 0x31;
const MISSING: Handle = NAMED | 0x32;
const MISSING_TOO: Handle = NAMED | 0x33;

fn boot(_: std::path::PathBuf) -> Boot {
    Boot {
        stack_sz: 1 << 20,
        mounts: vec![
            Box::new(|side| side.mount_named(NAMED_ONE, Rc::new(Nothing))),
            Box::new(|side| side.mount(OPENED, uri(&["opened"]), |_, _| Err(Errno::ENOENT))),
        ],
        imports: &[
            ("named", NAMED_ONE),
            ("opened", OPENED),
            ("missing", MISSING),
            ("missing-too", MISSING_TOO),
        ],
        calls: &[],
    }
}

/// A world importing what has no implementation refuses to start, naming each,
/// and the guest is never entered.
#[test]
fn unimplemented_import_refused() {
    let ui = app::start(boot, &[]);
    let mut msgs = Vec::new();
    app::post(ui, 1, ROOT, OPEN, Some(app::lowered(&[])));
    assert_eq!(app::answer_to(ui, 1, &mut msgs), Errno::EPIPE as i64);
    assert_eq!(
        app::exits(),
        [Exit::Trap(
            "no implementation of missing, missing-too".into()
        )]
    );
    assert!(
        !ENTERED.load(Ordering::Relaxed),
        "the guest is never entered"
    );
}
