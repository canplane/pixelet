// xpute-native-runtime/native.test.rs

//! A failure of the host's own is a stop, then a trap (native.rs `hook`).

use std::rc::Rc;

use xpute_kit::status::errno::Errno;
use xpute_link::{Handle, Method, Object, Reply, Tag, NAMED, OPEN, ROOT};
use xpute_native_runtime::native::Boot;

#[path = "app.test.rs"]
mod app;

use app::End;

/// The host's interface whose method fails.
const FAIL: Handle = NAMED | 0x20;
/// The guest's interface, whose method calls `FAIL`.
const RELAY: Handle = NAMED | 0x21;

struct Fail;

impl Object for Fail {
    fn call(&self, _: Method, _: Tag, _: Option<&[u8]>) -> Reply {
        panic!("the host fails");
    }
}

struct Relay;

impl Object for Relay {
    fn call(&self, _: Method, _: Tag, _: Option<&[u8]>) -> Reply {
        Reply::from(xpute_guest::link::call(FAIL, 0, None).0)
    }
}

xpute_guest::link!(4096, || xpute_guest::link::mount_named(RELAY, Rc::new(Relay)), || {}, || false);

fn boot(_: std::path::PathBuf) -> Boot {
    Boot {
        stack_sz: 1 << 20,
        servers: vec![Box::new(|| xpute_native_runtime::link::mount_named(FAIL, Rc::new(Fail)))],
        imports: &[("fail", FAIL)],
        later: &[],
    }
}

/// The host's failure, as it serves the guest, is told once as a stop and
/// then as a trap; after it the UI's methods answer EPIPE, and a failure on
/// another thread tells nothing more.
#[test]
fn failure_stops_then_traps() {
    let ui = app::start(boot, &[]);
    let mut messages = Vec::new();
    app::post(ui, 1, RELAY, 0, None);
    app::answer_to(ui, 1, &mut messages);
    let told = app::ends();
    assert!(
        matches!(&told[..], [End::Stop(e, file), End::Trap(err)]
            if *e == Errno::ENOTRECOVERABLE as i32 && file.ends_with("native.test.rs") && err.starts_with("the host stopped at ")),
        "{told:?}"
    );

    app::post(ui, 2, RELAY, 0, None);
    assert_eq!(app::answer_to(ui, 2, &mut messages), Errno::EPIPE as i64);
    let _ = std::thread::spawn(|| panic!("the host fails again")).join();
    app::post(ui, 3, ROOT, OPEN, Some(app::lowered(&[])));
    assert_eq!(app::answer_to(ui, 3, &mut messages), Errno::EPIPE as i64);
    assert_eq!(app::ends(), []);
}
