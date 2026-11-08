// xpute-runtime/native/thread.test.rs

use super::*;

/// A core that stops ends its own thread, not the process, and does so outside
/// the panic hook. The call it stopped in and every later call return `None`,
/// and the hook is left free for the process's next panic.
#[test]
fn a_stop_ends_the_core_alone() {
    let vcpu = Vcpu {
        turn: |_| -1.0,
        ready: || true,
    };
    start("core", 1 << 20, vcpu, || {});
    assert_eq!(run(|| 1), Some(1));
    assert_eq!(run::<()>(|| panic!("the core stops")), None);
    assert_eq!(run(|| 2), None);
    let taken = std::panic::take_hook();
    std::panic::set_hook(taken);
}
