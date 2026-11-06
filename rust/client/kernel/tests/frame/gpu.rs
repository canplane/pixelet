//! The device under the kernel's frames, as xpute's GPU host opens it beside the guest: an
//! error scope held open on the guest's thread, where every record runs, so each turn's
//! submissions are judged by WebGPU's validation; and the device waited out between turns,
//! so each frame's readback has landed by the next.

use std::cell::RefCell;
use std::time::Duration;

use xpute_gpu::host::native::host;
use xpute_host::native::Guest;

/// How long a turn's work may take on the device before the harness gives up on it.
const WAIT: Duration = Duration::from_secs(60);

/// The filters each turn is judged under.
const FILTERS: [wgpu::ErrorFilter; 2] =
    [wgpu::ErrorFilter::Validation, wgpu::ErrorFilter::Internal];

thread_local! {
    /// The scopes open on the guest's thread, one a filter.
    static SCOPES: RefCell<Vec<wgpu::ErrorScopeGuard>> = const { RefCell::new(Vec::new()) };
}

fn device() -> &'static wgpu::Device {
    host().expect("the device is open").device()
}

fn push() {
    SCOPES.with(|s| {
        let mut s = s.borrow_mut();
        for f in FILTERS {
            s.push(device().push_error_scope(f));
        }
    });
}

/// Opens the device as the kernel's machine asks, its scopes open from here on: none where
/// no adapter answers.
pub fn open(guest: Guest) -> Option<()> {
    if pxl_kernel::xpute_gpu_open() != 0 {
        return None;
    }
    guest.run(push);
    Some(())
}

/// The device's work waited out, then what the scopes held since they opened, each scope
/// opened again at once: an error a line; none where the guest has stopped.
pub fn take(guest: Guest) -> Vec<String> {
    let taken = guest.turn(|| {
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
    taken.unwrap_or_default()
}
