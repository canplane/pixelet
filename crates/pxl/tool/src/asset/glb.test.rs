// pxl-tool/asset/glb.test.rs

use std::path::{Path, PathBuf};

use pxl_spec::asset::ingested::INGESTED;

/// The asset tree as the ingest walks it: every `.glb` and `.png` under
/// `asset/`, sorted by path, less `cube/original/` — the source the tiles are
/// resized from, which the client never fetches by key.
fn walked(root: &Path) -> Vec<(String, PathBuf)> {
    fn walk(dir: &Path, root: &Path, out: &mut Vec<(String, PathBuf)>) {
        let Ok(entries) = std::fs::read_dir(dir) else { return };
        for entry in entries.flatten() {
            let path = entry.path();
            if path.is_dir() {
                walk(&path, root, out);
            } else {
                let rel = path.strip_prefix(root).unwrap().to_string_lossy().replace('\\', "/");
                if (rel.ends_with(".glb") || rel.ends_with(".png")) && !rel.starts_with("cube/original/") {
                    out.push((rel, path));
                }
            }
        }
    }
    let mut out = Vec::new();
    walk(root, root, &mut out);
    out.sort_by(|a, b| a.0.cmp(&b.0));
    out
}

/// Every shipped model, measured from the file and held against the table the
/// ingest wrote.
///
/// The table is what a catalog reads a model's size from, so a measurement
/// that drifts from it is a model drawn at the wrong size. It is committed,
/// which makes it a fixture this can be checked against without re-running the
/// ingest — and the walk above has to agree with the ingest's own ordering for
/// the indices to line up, so the row count is asserted first.
#[test]
fn every_shipped_model_measures_what_the_table_says() {
    // `cargo test` runs from the script's own directory rather than the
    // repository root the tools are otherwise invoked from, so the tree is
    // addressed from this file rather than from the working directory.
    let root = crate::root().join("asset");
    let files = walked(&root);
    assert_eq!(files.len(), INGESTED.len(), "the walk found what the table holds");

    let mut models = 0;
    for (i, (rel, path)) in files.iter().enumerate() {
        if !rel.ends_with(".glb") {
            assert!(INGESTED[i].extent.is_none(), "{rel}: an image has no extent");
            continue;
        }
        let bytes = std::fs::read(path).unwrap_or_else(|e| panic!("{}: {e}", path.display()));
        let got = super::extent(&bytes).unwrap_or_else(|e| panic!("{rel}: {e}"));
        let want = INGESTED[i].extent.unwrap_or_else(|| panic!("{rel}: the table has no extent for a model"));
        assert_eq!(got, want, "{rel}");
        models += 1;
    }
    // Not an arbitrary floor: every row the table gives an extent to is a
    // model, so measuring exactly that many is what says none was skipped.
    assert_eq!(models, INGESTED.iter().filter(|r| r.extent.is_some()).count(), "every model with a row was measured");
}
