// xpute-link-gen/wit.test.rs

use std::fs;
use std::path::{Path, PathBuf};

use super::*;

/// A directory of WIT packages for one test, removed with it.
struct Dir(PathBuf);

impl Dir {
    fn new(packages: &[(&str, &str)]) -> Dir {
        let dir = std::env::temp_dir().join(format!("xpute-link-gen-{}", std::process::id()));
        for (name, wit) in packages {
            fs::create_dir_all(dir.join(name)).unwrap();
            fs::write(dir.join(name).join("package.wit"), wit).unwrap();
        }
        Dir(dir)
    }
}

impl Drop for Dir {
    fn drop(&mut self) {
        let _ = fs::remove_dir_all(&self.0);
    }
}

/// A world that imports two interfaces whose TypeScript modules share a
/// name is refused, as the TypeScript tables name each by its module.
#[test]
fn imports_sharing_ts_module_refused() {
    let dir = Dir::new(&[
        ("b", "package b:y;\ninterface file { f: func(); }\n"),
        ("a", "package a:x;\ninterface file { f: func(); }\nworld w { import file; import b:y/file; }\n"),
    ]);
    let xpute = Path::new(env!("CARGO_MANIFEST_DIR")).join("../..");
    let link = WitTarget { wit: "link/wit", rs: None, ts: None, cpp: None, side: None };
    let b = WitTarget { wit: "b", rs: None, ts: None, cpp: None, side: None };
    let a = WitTarget { wit: "a", rs: None, ts: None, cpp: None, side: None };
    let refused = wit_outputs(&dir.0, &a, &[(&xpute, &link), (&dir.0, &b)]).err();
    assert_eq!(
        refused.as_deref(),
        Some("a:x: its world imports a:x/file and b:y/file, whose TypeScript modules are both file, and the TypeScript tables name each by its module")
    );
}
