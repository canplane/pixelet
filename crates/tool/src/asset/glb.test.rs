// pxl-tool/asset/glb.test.rs

use std::path::{Path, PathBuf};

use regex::Regex;

/// Every `.glb`, `.png` and `.webp` under `asset/`, sorted, less `cube/original/`.
fn walked(root: &Path) -> Vec<(String, PathBuf)> {
    fn walk(dir: &Path, root: &Path, out: &mut Vec<(String, PathBuf)>) {
        let Ok(entries) = std::fs::read_dir(dir) else { return };
        for entry in entries.flatten() {
            let path = entry.path();
            if path.is_dir() {
                walk(&path, root, out);
            } else {
                let rel = path.strip_prefix(root).unwrap().to_string_lossy().replace('\\', "/");
                if (rel.ends_with(".glb") || rel.ends_with(".png") || rel.ends_with(".webp")) && !rel.starts_with("cube/original/") {
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

/// The committed table's rows, path and extent, read from the TypeScript the
/// ingest writes beside pxl-spec's: this crate links none it writes into.
fn committed(root: &Path) -> Vec<(String, Option<[f64; 3]>)> {
    let text = std::fs::read_to_string(root.join("apps/web/src/catalog/asset/ingested.spec.ts")).unwrap();
    let row = Regex::new(r#"(?m)^  "([^"]+)": \{ key: "[^"]*", bytes: \d+(?:, extent: \[([^,\]]+), ([^,\]]+), ([^,\]]+)\])? \},$"#).unwrap();
    row.captures_iter(&text)
        .map(|c| {
            let n = |i: usize| c.get(i).map(|m| m.as_str().parse::<f64>().unwrap());
            (c[1].to_string(), n(2).map(|x| [x, n(3).unwrap(), n(4).unwrap()]))
        })
        .collect()
}

/// Every model measured against the committed table. The walk must order as
/// the ingest does for the rows to line up.
#[test]
fn every_shipped_model_measures_what_the_table_says() {
    let root = crate::root();
    let files = walked(&root.join("asset"));
    let table = committed(&root);
    let paths: Vec<&str> = files.iter().map(|(rel, _)| rel.as_str()).collect();
    let rows: Vec<&str> = table.iter().map(|(rel, _)| rel.as_str()).collect();
    assert_eq!(paths, rows, "the walk found what the table holds");

    for ((rel, path), (_, want)) in files.iter().zip(&table) {
        if !rel.ends_with(".glb") {
            assert!(want.is_none(), "{rel}: an image has no extent");
            continue;
        }
        let bytes = std::fs::read(path).unwrap_or_else(|e| panic!("{}: {e}", path.display()));
        let got = super::extent(&bytes).unwrap_or_else(|e| panic!("{rel}: {e}"));
        assert_eq!(Some(got), *want, "{rel}");
    }
}
