// xpute-native-runtime/link.init.test.rs

//! A guest that fails while it lays its link traps, and tells no stop, as the
//! host cannot hear one yet (xpute-guest link.rs `in_init`).

use xpute_kit::status::errno::Errno;
use xpute_link::{OPEN, ROOT};
use xpute_native_runtime::native::Boot;

#[path = "app.test.rs"]
mod app;

use app::End;

// A cap that is no multiple of 8 is the guest's bug in `link_init`.
xpute_guest::link!(4097, || {}, || {}, || false);

fn boot(_: std::path::PathBuf) -> Boot {
    Boot {
        stack_sz: 1 << 20,
        servers: Vec::new(),
        imports: &[],
        later: &[],
    }
}

#[test]
fn guest_panic_in_init_traps_only() {
    let ui = app::start(boot, &[]);
    let mut messages = Vec::new();
    app::post(ui, 1, ROOT, OPEN, Some(app::lowered(&[])));
    assert_eq!(app::answer_to(ui, 1, &mut messages), Errno::EPIPE as i64);
    assert_eq!(app::ends(), [End::Trap("the guest trapped".into())]);
}
