//! The GPU device under the core's frames, opened beside the core as xpute's GPU runtime
//! opens it. Error scopes stay open where the stream runs, so WebGPU's validation judges
//! each turn's submissions. Between turns the harness waits for
//! the device to finish, so each frame's readback has landed by the next frame.

use std::cell::RefCell;
use std::time::Duration;

use xpute_gpu::runtime::native::{on_stream, with_runtime};

/// How long a turn's work may take on the device before the harness gives up on it.
const WAIT: Duration = Duration::from_secs(60);

/// The filters each turn is judged under.
const FILTERS: [wgpu::ErrorFilter; 2] =
    [wgpu::ErrorFilter::Validation, wgpu::ErrorFilter::Internal];

thread_local! {
    /// The scopes open where the stream runs, one for each filter.
    static SCOPES: RefCell<Vec<wgpu::ErrorScopeGuard>> = const { RefCell::new(Vec::new()) };
}

fn device() -> wgpu::Device {
    with_runtime(|h| h.device().clone()).expect("the device is open")
}

fn push() {
    SCOPES.with(|s| {
        let mut s = s.borrow_mut();
        for f in FILTERS {
            s.push(device().push_error_scope(f));
        }
    });
}

/// Opens the device as the core's manifest asks, and opens the scopes. Returns None when no
/// adapter answers.
pub fn open() -> Option<()> {
    // SAFETY: no directory, so the test's pipelines are kept nowhere.
    if unsafe { pxl_core::xpute_gpu_open(core::ptr::null()) } != 0 {
        return None;
    }
    on_stream(push).expect("the device runs");
    Some(())
}

/// Waits for the device's work to finish, then returns the errors the scopes caught since
/// they opened, one line each, and opens the scopes again at once.
pub fn take() -> Vec<String> {
    let errs = on_stream(|| {
        device()
            .poll(wgpu::PollType::Wait {
                submission_index: None,
                timeout: Some(WAIT),
            })
            .unwrap_or_else(|e| panic!("the device did not finish in {WAIT:?}: {e:?}"));
        let popped: Vec<_> = SCOPES.with(|s| s.borrow_mut().drain(..).rev().collect());
        let errs = popped
            .into_iter()
            .filter_map(|g| pollster::block_on(g.pop()))
            .map(|e| e.to_string())
            .collect();
        push();
        errs
    });
    errs.unwrap_or_default()
}
