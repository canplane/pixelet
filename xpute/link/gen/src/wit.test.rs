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

/// A world that imports an interface whose module another import, or for one
/// of another package an interface of its own, already names is refused, as
/// the bindings name each import by its module.
#[test]
fn imports_sharing_module_refused() {
    let xpute = Path::new(env!("CARGO_MANIFEST_DIR")).join("../..");
    let link = WitTarget {
        wit: "link/wit",
        rs: None,
        ts: None,
        cpp: None,
        side: None,
    };
    let b = WitTarget {
        wit: "b",
        rs: None,
        ts: None,
        cpp: None,
        side: None,
    };
    let c = WitTarget {
        wit: "c",
        rs: None,
        ts: None,
        cpp: None,
        side: None,
    };
    let a = WitTarget {
        wit: "a",
        rs: None,
        ts: None,
        cpp: None,
        side: None,
    };
    for (own, world, other) in [
        (
            "other",
            "export other; import b:y/file; import c:z/file;",
            "c:z/file, whose module file is b:y/file's",
        ),
        (
            "file",
            "export file; import b:y/file;",
            "b:y/file, whose module file is a:x/file's",
        ),
    ] {
        let dir = Dir::new(&[
            ("b", "package b:y;\ninterface file { f: func(); }\n"),
            ("c", "package c:z;\ninterface file { f: func(); }\n"),
            (
                "a",
                &format!("package a:x;\ninterface {own} {{ f: func(); }}\nworld w {{ {world} }}\n"),
            ),
        ]);
        let refused = wit_outputs(&dir.0, &a, &[(&xpute, &link), (&dir.0, &b), (&dir.0, &c)]).err();
        assert_eq!(
            refused,
            Some(format!("a:x: its world imports {other} too, and the bindings name each import by its module")),
            "{world}"
        );
    }
}
