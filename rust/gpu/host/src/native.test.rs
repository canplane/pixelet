// pxl-gpu-host/native.test.rs

use std::sync::Mutex;

use super::*;

static STATES: Mutex<Vec<String>> = Mutex::new(Vec::new());

extern "C" fn record(state: *const u8, len: usize) {
    // SAFETY: the host's UTF-8, `len` bytes.
    let s = unsafe { core::slice::from_raw_parts(state, len) };
    STATES
        .lock()
        .unwrap()
        .push(String::from_utf8_lossy(s).into_owned());
}

/// What the host has of a GPU is told as it changes, and kept as the state
/// now, which a UI that starts after it was told reads (`state`).
#[test]
fn late_ui_reads_state() {
    assert_eq!(state(), None, "none before the guest's first ask");
    state_on(Some(record));
    report("present");
    state_on(None);
    assert_eq!(*STATES.lock().unwrap(), ["present"]);
    assert_eq!(state(), Some("present"));
    report("waiting");
    assert_eq!(state(), Some("waiting"));
    assert_eq!(
        *STATES.lock().unwrap(),
        ["present"],
        "told to none while none hears"
    );
}
