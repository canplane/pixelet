// xpute-runtime-native/native.test.rs

//! A failure of the host's own is a stop, then a trap (native.rs `hook`).

use std::rc::Rc;

use xpute_kit::status::errno::Errno;
use xpute_link::{Handle, Lift, Lower, Method, NAMED, OPEN, Object, ROOT, Tag};
use xpute_runtime_native::native::Boot;

#[path = "app.test.rs"]
mod app;

use app::Exit;
use xpute_guest::link;

/// The host's interface whose method fails.
const FAIL: Handle = NAMED | 0x20;
/// The guest's interface, whose method calls `FAIL`.
const RELAY: Handle = NAMED | 0x21;

struct Fail;

impl Object for Fail {
    fn call(&self, _: Method, _: Tag, _: Lift, _: &mut Lower) -> i32 {
        panic!("the host fails");
    }
}

struct Relay;

impl Object for Relay {
    fn call(&self, _: Method, _: Tag, _: Lift, _: &mut Lower) -> i32 {
        link::call(FAIL, 0, None).0
    }
}

xpute_guest::link!(
    4096,
    || link::mount_named(RELAY, Rc::new(Relay)),
    || {},
    || false
);

fn boot(_: std::path::PathBuf) -> Boot {
    Boot {
        stack_sz: 1 << 20,
        mounts: vec![Box::new(|side| side.mount_named(FAIL, Rc::new(Fail)))],
        imports: &[("fail", FAIL)],
        calls: &[],
    }
}

/// The host's failure, as it answers the guest, is told once as a stop and
/// then as a trap; after it the UI's methods answer EPIPE, and a failure on
/// another thread tells nothing more.
#[test]
fn failure_stops_then_traps() {
    let ui = app::start(boot, &[]);
    let mut msgs = Vec::new();
    app::post(ui, 1, RELAY, 0, None);
    app::answer_to(ui, 1, &mut msgs);
    let told = app::exits();
    assert!(
        matches!(&told[..], [Exit::Stop(e, file), Exit::Trap(err)]
            if *e == Errno::ENOTRECOVERABLE as i32
                && file.ends_with("native.test.rs")
                && err.starts_with("the host stopped: panicked at ")
                && err.ends_with("the host fails")),
        "{told:?}"
    );

    app::post(ui, 2, RELAY, 0, None);
    assert_eq!(app::answer_to(ui, 2, &mut msgs), Errno::EPIPE as i64);
    let _ = std::thread::spawn(|| panic!("the host fails again")).join();
    app::post(ui, 3, ROOT, OPEN, Some(app::lowered(&[])));
    assert_eq!(app::answer_to(ui, 3, &mut msgs), Errno::EPIPE as i64);
    assert_eq!(app::exits(), []);
}
