// xpute-runtime-native/remote.test.rs

//! A native reload attaches a new UI to the same runtime (remote.rs `accept`).

use std::rc::Rc;
use std::sync::Mutex;
use std::sync::atomic::{AtomicU32, Ordering};

use xpute_guest::abi::life;
use xpute_guest::link;
use xpute_kit::status::errno::Errno;
use xpute_link::frame::Frame;
use xpute_link::{Handle, Lift, Lower, Method, NAMED, OPEN, Object, ROOT, Tag, uri};
use xpute_runtime_native::listener;
use xpute_runtime_native::native::Boot;

#[path = "app.test.rs"]
mod app;

/// The UI's interface, whose method 0, an `async func`, it answers later.
const UI: Handle = NAMED | 0x10;
/// The guest's interface, whose objects the UI opens at `obj`.
const OBJ: Handle = NAMED | 0x11;

static ATTACH_CNT: AtomicU32 = AtomicU32::new(0);
static DROP_CNT: AtomicU32 = AtomicU32::new(0);
/// How many times the runtime told the app a batch waits.
static NOTIFY_CNT: AtomicU32 = AtomicU32::new(0);
/// The answers to the guest's requests of the UI, in the order heard.
static ANSWERS: Mutex<Vec<i32>> = Mutex::new(Vec::new());

/// At each attach the guest sends the UI a request it answers later.
struct Guest;

impl life::Guest for Guest {
    fn hidden(&self) -> Result<(), Errno> {
        Ok(())
    }

    fn shown(&self) -> Result<(), Errno> {
        Ok(())
    }

    fn attached(&self) -> Result<(), Errno> {
        ATTACH_CNT.fetch_add(1, Ordering::Relaxed);
        link::send(
            UI,
            0,
            None,
            Some(Box::new(|r, _| ANSWERS.lock().unwrap().push(r))),
        );
        Ok(())
    }
}

struct Obj;

impl Object for Obj {
    fn call(&self, _: Method, _: Tag, _: Lift, _: &mut Lower) -> i32 {
        0
    }
}

impl Drop for Obj {
    fn drop(&mut self) {
        DROP_CNT.fetch_add(1, Ordering::Relaxed);
    }
}

xpute_guest::link!(
    4096,
    || {
        life::mount(Rc::new(Guest));
        link::mount(
            OBJ,
            uri(&["obj"]),
            |_, _| Ok(Rc::new(Obj) as Rc<dyn Object>),
        );
    },
    || {},
    || false
);

fn boot(_: std::path::PathBuf) -> Boot {
    Boot {
        stack_sz: 1 << 20,
        mounts: Vec::new(),
        imports: &[("ui", UI)],
        calls: &[],
    }
}

extern "C" fn notified() {
    NOTIFY_CNT.fetch_add(1, Ordering::Relaxed);
}

/// The tag of the request the guest sent the UI of the handle `ui` to answer
/// later, among what that UI heard.
fn request(ui: u32, frames: &mut Vec<Frame>) -> i64 {
    let mut tag = None;
    app::until("the guest's request", || {
        frames.extend(app::take(ui));
        tag = frames
            .iter()
            .find(|m| !m.res && m.handle == UI && m.method == 0)
            .map(|m| i64::from(m.tag));
        tag.is_some()
    });
    tag.unwrap()
}

/// A new UI is told the guest attached; each request the old UI left pending
/// is answered ECANCELED, once; the guest's objects it opened are dropped; and
/// the old UI's take, and its answer to the new UI's request, reach nothing.
#[test]
fn reload() {
    listener::notify_on(Some(notified));
    let old = app::start(boot, &[UI]);
    let mut frames = Vec::new();
    let pending = request(old, &mut frames) as Tag;
    app::post(old, 1, ROOT, OPEN, Some(app::lowered(&uri(&["obj"]))));
    assert_eq!(app::answer_to(old, 1, &mut frames), 0);

    let notify_cnt = NOTIFY_CNT.load(Ordering::Relaxed);
    let new = app::start(boot, &[UI]);
    app::until("the new UI attached", || {
        ATTACH_CNT.load(Ordering::Relaxed) == 2
    });
    app::until("the old request canceled", || {
        *ANSWERS.lock().unwrap() == [Errno::ECANCELED as i32]
    });
    app::until("the old UI's object dropped", || {
        DROP_CNT.load(Ordering::Relaxed) == 1
    });
    app::until("a batch waits for the new UI", || {
        NOTIFY_CNT.load(Ordering::Relaxed) > notify_cnt
    });
    assert!(app::take(old).is_empty(), "the old UI takes nothing of it");

    let mut frames = Vec::new();
    let again = request(new, &mut frames) as Tag;
    xpute_runtime_native::answer(old, pending, 0, None);
    xpute_runtime_native::answer(new, pending, 0, None);
    xpute_runtime_native::answer(old, again, 0, None);
    // A round trip after them, which the guest's thread runs in turn.
    app::post(new, 2, ROOT, OPEN, Some(app::lowered(&uri(&["none"]))));
    assert_eq!(app::answer_to(new, 2, &mut frames), Errno::ENOENT as i64);
    assert_eq!(*ANSWERS.lock().unwrap(), [Errno::ECANCELED as i32]);

    xpute_runtime_native::answer(new, again, 0, None);
    app::until("the new UI's answer heard", || {
        *ANSWERS.lock().unwrap() == [Errno::ECANCELED as i32, 0]
    });
    assert_eq!(app::exits(), []);
}
