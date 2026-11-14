// pxl-render/tests/gpu/lock.rs

//! The GPU of the machine the tests run on, which one user drives at a time across every
//! checkout of the repository: a test process that opens a device holds it from then until it
//! ends, as a run or the shader probe on the machine does (script/perf/measure.py `hold`). A
//! test process that finds it held waits, and says so.

use std::fs::File;
use std::io::Write;
use std::path::PathBuf;
use std::sync::OnceLock;

/// Waits until this process holds the GPU. It holds it until it ends.
pub(super) fn hold() {
    static HELD: OnceLock<File> = OnceLock::new();
    HELD.get_or_init(|| {
        let path = lock_path();
        std::fs::create_dir_all(path.parent().expect("tmp/")).expect("the lock's folder");
        let file = File::options()
            .create(true)
            .truncate(false)
            .write(true)
            .open(&path)
            .expect("the GPU's lock");
        if file.try_lock().is_err() {
            // Written past the test harness, which keeps a test's own lines until it ends.
            let _ = writeln!(
                std::io::stderr(),
                "waiting for gpu, held at {}",
                path.display()
            );
            file.lock().expect("the GPU's lock");
        }
        file
    });
}

/// `tmp/lock/gpu.lock` in the repository's main checkout, which its worktrees share, found as
/// script/perf/measure.py `hold` finds it.
fn lock_path() -> PathBuf {
    let out = std::process::Command::new("git")
        .args(["rev-parse", "--path-format=absolute", "--git-common-dir"])
        .current_dir(env!("CARGO_MANIFEST_DIR"))
        .output()
        .expect("git");
    let common = PathBuf::from(String::from_utf8(out.stdout).expect("a path").trim());
    common
        .parent()
        .expect("the main checkout")
        .join("tmp/lock/gpu.lock")
}
