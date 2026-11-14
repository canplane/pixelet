// pxl-core/domain/layer/object/repository.test.rs

use pxl_spec::asset::marker::MARKER;
use pxl_spec::bake::constant::OBJECT_COARSEST;
use pxl_spec::space::coord::zoom::Zoom;
use pxl_spec::space::hierarchy::morton::{key_of, key_of_name};
use xpute_golden::Golden;

use super::*;
use crate::sys::io::request;

/// The golden top rows for each case's writes. The store's fold and the
/// source's fold (ts/client/src/service/sys/fs/object_tree.ts) must both produce
/// them.
const GOLDEN: &str = include_str!(concat!(env!("PXL_REPO"), "/spec/golden/object/top.tsv"));

/// A leaf at `zoom`, `x` shards east of the first.
fn leaf_at(zoom: Zoom, x: i32, at: [i32; 3]) -> u128 {
    let shard = 1 << (zoom - SHARD_ZOOM);
    key_of(zoom, [at[0] + x * shard, at[1], at[2]])
}

/// A leaf at `OBJECT_COARSEST`, `x` shards east of the first.
fn leaf(x: i32, at: [i32; 3]) -> u128 {
    leaf_at(OBJECT_COARSEST, x, at)
}

/// Empties the store and the repository's state, and opens a request
/// scheduler for `want` to declare files to.
fn fresh() {
    object_mut().clear();
    *state() = State::default();
    request::open(|_| WANT_CAP);
}

/// Every leaf the store holds, by key, with its text.
fn leaves() -> Vec<(u128, u32, Option<String>)> {
    let object = object();
    let mut out = Vec::new();
    object.each(&mut |e| {
        out.push((
            key_of(e.zoom, e.at),
            e.asset_id,
            object.text(e).map(str::to_string),
        ));
    });
    out.sort_unstable_by_key(|&(key, _, _)| key);
    out
}

fn written() -> [Write; 4] {
    [
        Write {
            at: leaf(0, [5, 7, 2]),
            asset_id: 3,
            ext: None,
        },
        // Below sea level, in the lower half of the shard's height.
        Write {
            at: leaf(0, [5, 8, -3]),
            asset_id: 4,
            ext: None,
        },
        Write {
            at: leaf(0, [6, 9, 1]),
            asset_id: MARKER,
            ext: Some(Ext::MarkerText("here".to_string())),
        },
        Write {
            at: leaf(1, [5, 7, 2]),
            asset_id: 3,
            ext: None,
        },
    ]
}

/// The writes of `shard` among `written`, as its file holds them.
fn shard_file(shard: Quadrant) -> Vec<u8> {
    let own: Vec<Write> = written()
        .into_iter()
        .filter(|w| shard_of(w.at) == shard)
        .collect();
    encode_writes(format::OBJECT, &own).unwrap()
}

/// A top file holding `rows`, each `<shard, fold>`.
fn top_file(rows: &[(u64, u32)]) -> Vec<u8> {
    let shard: Vec<String> = rows.iter().map(|r| Quadrant::of_id(r.0).key()).collect();
    let shard: Vec<&str> = shard.iter().map(String::as_str).collect();
    let fold: Vec<u32> = rows.iter().map(|r| r.1).collect();
    let mut tree = TreeView::new();
    tree.branch(Some(&mut |b| {
        b.strs(Some(&shard)).u32_array(Some(&fold));
    }));
    let pkt = encoder()
        .encode(
            &tree,
            TreeEncoderConfig {
                init_cap: 1 << 10,
                max_cap: None,
            },
        )
        .unwrap();
    [&[format::OBJECT_TOP][..], &pkt].concat()
}

/// The top's rows that the store's folds make of the shards `writes` lie
/// in, in shard order.
fn top_of(writes: &[Write]) -> Vec<(u64, u32)> {
    let mut shards: Vec<Quadrant> = writes.iter().map(|w| shard_of(w.at)).collect();
    shards.sort_unstable();
    shards.dedup();
    let mut rows = Vec::new();
    for shard in shards {
        if let Some(word) = object().word_at(shard.at) {
            rows.push((shard.id(), word));
        }
    }
    rows
}

/// A shard's file reads back as its own leaves, records and all.
#[test]
fn shard_file_round_trip() {
    crate::on_core(|| {
        fresh();
        let shard = shard_of(leaf(0, [5, 7, 2]));
        let writes = decode_shard(shard, &shard_file(shard)).unwrap();
        apply(writes);
        let own: Vec<_> = {
            fresh();
            apply(written());
            leaves()
                .into_iter()
                .filter(|&(key, _, _)| shard_of(key) == shard)
                .collect()
        };
        assert_eq!(own.len(), 3);
        fresh();
        apply(decode_shard(shard, &shard_file(shard)).unwrap());
        assert_eq!(leaves(), own);
    });
}

/// A file of another format, or another shard's file, is refused whole.
#[test]
fn foreign_file_refused() {
    crate::on_core(|| {
        let shard = shard_of(leaf(0, [5, 7, 2]));
        let mut file = shard_file(shard);
        assert!(decode_shard(shard_of(leaf(1, [5, 7, 2])), &file).is_err());
        file[0] = format::OBJECT_EDIT;
        assert!(decode_shard(shard, &file).is_err());
        assert!(decode_shard(shard, &[]).is_err());
        assert!(decode_top(&file).is_err());
    });
}

/// An edit batch reads back as its writes in the order they were made,
/// erases included.
#[test]
fn edit_batch_round_trip() {
    crate::on_core(|| {
        let mut writes = written().to_vec();
        writes.push(Write {
            at: leaf(0, [5, 7, 2]),
            asset_id: 0,
            ext: None,
        });
        let batch = encode_writes(format::OBJECT_EDIT, &writes).unwrap();
        assert_eq!(batch.first(), Some(&format::OBJECT_EDIT));
        let read = decode_writes(format::OBJECT_EDIT, &batch).unwrap();
        let rows = |w: &[Write]| -> Vec<(u128, u32, Option<String>)> {
            w.iter()
                .map(|w| {
                    let text = w.ext.as_ref().map(|Ext::MarkerText(t)| t.clone());
                    (w.at, w.asset_id, text)
                })
                .collect()
        };
        assert_eq!(rows(&read), rows(&writes));
    });
}

/// Before the top is read, only the top is wanted and no edit is allowed
/// anywhere. An absent top means no shard holds anything, so every shard is
/// taken as empty without asking.
#[test]
fn absent_top_reads_every_shard() {
    crate::on_core(|| {
        fresh();
        let shard = shard_of(leaf(0, [5, 7, 2]));
        want([shard].into_iter(), |_| Order::LAST);
        assert_eq!(state().wants.len(), 1);
        assert_eq!(state().wants[0].0, TOP);
        assert!(!holds(&shard.region()));
        admit(TOP, None).unwrap();
        assert!(holds(&shard.region()));
        want([shard].into_iter(), |_| Order::LAST);
        assert!(state().wants.is_empty());
    });
}

/// The top's folds stand in for the shards not read yet and give way to a
/// shard's leaves once it is read. A shard the top lists is wanted; one it
/// does not list is taken as empty.
#[test]
fn top_folds_until_shard_read() {
    crate::on_core(|| {
        let (shard, other) = (shard_of(leaf(0, [5, 7, 2])), shard_of(leaf(2, [5, 7, 2])));
        // The folds of the written shard, as the store makes them.
        fresh();
        apply(decode_shard(shard, &shard_file(shard)).unwrap());
        let rows = top_of(&written()[..3]);
        assert_eq!(rows.len(), 1);
        fresh();
        admit(TOP, Some(&top_file(&rows))).unwrap();
        assert_eq!(object().word_at(shard.at), Some(rows[0].1));
        assert!(leaves().is_empty());
        assert!(!holds(&shard.region()) && holds(&other.region()));
        want([shard, other].into_iter(), |_| Order::LAST);
        assert_eq!(state().wants.len(), 1);
        assert_eq!(state().wants[0].0, shard.id());
        admit(shard.id(), Some(&shard_file(shard))).unwrap();
        assert!(holds(&shard.region()));
        assert_eq!(top_of(&written()[..3]), rows);
        assert_eq!(leaves().len(), 3);
    });
}

/// A file that does not decode is refused: it is named to the client, not
/// wanted, and wanted again as new once the client empties it.
#[test]
fn refused_file_named() {
    crate::on_core(|| {
        fresh();
        let shard = shard_of(leaf(0, [5, 7, 2]));
        admit(TOP, Some(&top_file(&[(shard.id(), 0)]))).unwrap();
        let foreign = [format::SETTINGS, 1, 2, 3];
        assert!(admit(shard.id(), Some(&foreign)).is_err());
        assert_eq!(refused_files(), &[shard.id()]);
        assert!(!holds(&shard.region()));
        want([shard].into_iter(), |_| Order::LAST);
        assert!(state().wants.is_empty());
        assert!(on_emptied(shard.id()));
        assert!(refused_files().is_empty() && !on_emptied(shard.id()));
        want([shard].into_iter(), |_| Order::LAST);
        assert_eq!(state().wants.len(), 1);
    });
}

/// The writes since the last batch go up as one, and none is kept after.
#[test]
fn save_sends_one_batch() {
    crate::on_core(|| {
        fresh();
        for w in written() {
            send(w);
        }
        assert_eq!(state().unsent.len(), 4);
        save();
        assert!(state().unsent.is_empty());
    });
}

/// Applied in order to an empty store, each case's writes fold to the golden
/// top rows, the same rows the source's fold is tested against.
#[test]
fn top_golden() {
    crate::on_core(|| {
        let g = Golden::load(GOLDEN);
        let mut cases = 0;
        g.each("case", |case| {
            fresh();
            let mut writes = Vec::new();
            g.each(&format!("{case}.write"), |w| {
                let (key, id) = g.s(w).split_once(' ').unwrap();
                writes.push(Write {
                    at: key_of_name::<3>(key).unwrap(),
                    asset_id: id.parse().unwrap(),
                    ext: None,
                });
            });
            // A case may fold to no row.
            let mut rows = Vec::new();
            while g.has(&format!("{case}.top.{}", rows.len())) {
                let row = g.s(&format!("{case}.top.{}", rows.len()));
                let f: Vec<&str> = row.split(' ').collect();
                rows.push((
                    Quadrant::of_key(f[0]).unwrap().id(),
                    f[1].parse::<u32>().unwrap(),
                ));
            }
            apply(writes.clone());
            assert_eq!(top_of(&writes), rows, "{case}");
            cases += 1;
        });
        assert!(cases > 0);
    });
}
