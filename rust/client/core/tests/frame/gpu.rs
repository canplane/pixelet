//! The GPU device under the core's frames, opened beside the core as xpute's GPU runtime
//! opens it. Error scopes stay open on the core's thread, where every recording runs, so
//! WebGPU's validation judges each turn's submissions. Between turns the harness waits for
//! the device to finish, so each frame's readback has landed by the next frame.

use std::cell::RefCell;
use std::time::Duration;

use xpute_gpu::runtime::native::runtime;
use xpute_runtime::native::Core;

/// How long a turn's work may take on the device before the harness gives up on it.
const WAIT: Duration = Duration::from_secs(60);

/// The filters each turn is judged under.
const FILTERS: [wgpu::ErrorFilter; 2] =
    [wgpu::ErrorFilter::Validation, wgpu::ErrorFilter::Internal];

thread_local! {
    /// The scopes open on the core's thread, one for each filter.
    static SCOPES: RefCell<Vec<wgpu::ErrorScopeGuard>> = const { RefCell::new(Vec::new()) };
}

fn device() -> &'static wgpu::Device {
    runtime().expect("the device is open").device()
}

fn push() {
    SCOPES.with(|s| {
        let mut s = s.borrow_mut();
        for f in FILTERS {
            s.push(device().push_error_scope(f));
        }
    });
}

/// Opens the device as the core's machine asks, and opens the scopes. Returns None when no
/// adapter answers.
pub fn open(core: Core) -> Option<()> {
    if pxl_core::xpute_gpu_open() != 0 {
        return None;
    }
    core.run_on(push).expect("the core has stopped");
    Some(())
}

/// Waits for the device's work to finish, then returns the errors the scopes caught since
/// they opened, one line each, and opens the scopes again at once. Returns no errors when
/// the core has stopped.
pub fn take(core: Core) -> Vec<String> {
    let errors = core.run_on(|| {
        device()
            .poll(wgpu::PollType::Wait {
                submission_index: None,
                timeout: Some(WAIT),
            })
            .unwrap_or_else(|e| panic!("the device did not finish in {WAIT:?}: {e:?}"));
        let popped: Vec<_> = SCOPES.with(|s| s.borrow_mut().drain(..).rev().collect());
        let errors = popped
            .into_iter()
            .filter_map(|g| pollster::block_on(g.pop()))
            .map(|e| e.to_string())
            .collect();
        push();
        errors
    });
    errors.unwrap_or_default()
}
