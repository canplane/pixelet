// pxl-kernel/domain/layer/object/repository.test.rs

use pxl_spec::asset::marker::MARKER;
use pxl_spec::bake::constant::OBJECT_COARSEST;
use pxl_spec::space::coord::zoom::Zoom;
use pxl_spec::space::hierarchy::morton::key_of;
use xpute_kit::golden::Golden;

use super::*;
use crate::sys;
use crate::sys::io::request;

/// The top's rows for the writes of each case, as the store folds them and
/// as the source folds them (ts/client/src/driver/namespace/object_tree.ts).
const GOLDEN: &str = include_str!("../../../../../../spec/golden/object/top.tsv");

/// A leaf at `zoom`, `x` shards east of the first.
fn leaf_at(zoom: Zoom, x: i32, at: [i32; 3]) -> u128 {
    let shard = 1 << (zoom - SHARD_ZOOM);
    key_of(zoom, [at[0] + x * shard, at[1], at[2]])
}

/// A leaf at `OBJECT_COARSEST`, `x` shards east of the first.
fn leaf(x: i32, at: [i32; 3]) -> u128 {
    leaf_at(OBJECT_COARSEST, x, at)
}

/// The store and the files read emptied, and a scheduler to want them of.
fn fresh() {
    object().clear();
    *state() = State::default();
    request::open(|_| WANTS);
}

/// Every leaf the store holds, by key, with its text.
fn held() -> Vec<(u128, u32, Option<String>)> {
    let object = object();
    let mut out = Vec::new();
    object.each(&mut |e| {
        out.push((
            key_of(e.zoom, e.at),
            e.state,
            object.text(e).map(str::to_string),
        ));
    });
    out.sort_unstable_by_key(|&(key, _, _)| key);
    out
}

fn written() -> [Write; 4] {
    [
        Write {
            key: leaf(0, [5, 7, 2]),
            id: 3,
            ext: None,
        },
        // Under sea level, the shard's other half of its height.
        Write {
            key: leaf(0, [5, 8, -3]),
            id: 4,
            ext: None,
        },
        Write {
            key: leaf(0, [6, 9, 1]),
            id: MARKER,
            ext: Some(Ext::MarkerText("here".to_string())),
        },
        Write {
            key: leaf(1, [5, 7, 2]),
            id: 3,
            ext: None,
        },
    ]
}

/// The writes of `shard` among `written`, as its file holds them.
fn shard_file(shard: Tile) -> Vec<u8> {
    let own: Vec<Write> = written()
        .into_iter()
        .filter(|w| shard_of(w.key) == shard)
        .collect();
    encode_writes(format::OBJECT, &own).unwrap()
}

/// The top holding `rows`, `<shard, z, fold>`.
fn top_file(rows: &[(u64, i32, u32)]) -> Vec<u8> {
    let shard: Vec<u64> = rows.iter().map(|r| r.0).collect();
    let z: Vec<i32> = rows.iter().map(|r| r.1).collect();
    let fold: Vec<u32> = rows.iter().map(|r| r.2).collect();
    let mut tree = TreeView::new();
    tree.branch(Some(&mut |b| {
        b.u64_array(Some(&shard))
            .i32_array(Some(&z))
            .u32_array(Some(&fold));
    }));
    let pkt = encoder()
        .encode(
            &tree,
            TreeEncoderOptions {
                init_cap: 1 << 10,
                max_cap: None,
            },
        )
        .unwrap();
    [&[format::OBJECT_TOP][..], &pkt].concat()
}

/// The top's rows the store's folds make of the shards `writes` lie in, by
/// shard and Z.
fn top_of(writes: &[Write]) -> Vec<(u64, i32, u32)> {
    let mut shards: Vec<Tile> = writes.iter().map(|w| shard_of(w.key)).collect();
    shards.sort_unstable();
    shards.dedup();
    let mut rows = Vec::new();
    for shard in shards {
        for z in SHARD_Z {
            if let Some(word) = object().word_at([shard.at[0], shard.at[1], z]) {
                rows.push((shard.subject(), z, word));
            }
        }
    }
    rows
}

/// A shard's file reads back as its own leaves, records and all.
#[test]
fn shard_file_round_trip() {
    let _g = sys::serial();
    fresh();
    let shard = shard_of(leaf(0, [5, 7, 2]));
    let writes = decode_shard(shard, &shard_file(shard)).unwrap();
    apply(writes);
    let own: Vec<_> = {
        fresh();
        apply(written());
        held()
            .into_iter()
            .filter(|&(key, _, _)| shard_of(key) == shard)
            .collect()
    };
    assert_eq!(own.len(), 3);
    fresh();
    apply(decode_shard(shard, &shard_file(shard)).unwrap());
    assert_eq!(held(), own);
}

/// Another format, or another shard's file, is refused whole.
#[test]
fn foreign_file_refused() {
    let shard = shard_of(leaf(0, [5, 7, 2]));
    let mut file = shard_file(shard);
    assert!(decode_shard(shard_of(leaf(1, [5, 7, 2])), &file).is_err());
    file[0] = format::OBJECT_EDITS;
    assert!(decode_shard(shard, &file).is_err());
    assert!(decode_shard(shard, &[]).is_err());
    assert!(decode_top(&file).is_err());
}

/// An edit batch reads back as the writes in the order made, erases among
/// them.
#[test]
fn edit_batch_round_trip() {
    let mut writes = written().to_vec();
    writes.push(Write {
        key: leaf(0, [5, 7, 2]),
        id: 0,
        ext: None,
    });
    let batch = encode_writes(format::OBJECT_EDITS, &writes).unwrap();
    assert_eq!(batch.first(), Some(&format::OBJECT_EDITS));
    let read = decode_writes(format::OBJECT_EDITS, &batch).unwrap();
    let rows = |w: &[Write]| -> Vec<(u128, u32, Option<String>)> {
        w.iter()
            .map(|w| {
                let text = w.ext.as_ref().map(|Ext::MarkerText(t)| t.clone());
                (w.key, w.id, text)
            })
            .collect()
    };
    assert_eq!(rows(&read), rows(&writes));
}

/// Before the top, only the top is wanted and no edit stands anywhere; no
/// top is no shard holding any, each read empty without asking.
#[test]
fn absent_top_reads_every_shard() {
    let _g = sys::serial();
    fresh();
    let shard = shard_of(leaf(0, [5, 7, 2]));
    want([(shard, Ask::PLAIN)].into_iter());
    assert_eq!(state().wants.len(), 1);
    assert_eq!(state().wants[0].0, TOP);
    assert!(!holds(&shard.region()));
    arrived(TOP, None).unwrap();
    assert!(holds(&shard.region()));
    want([(shard, Ask::PLAIN)].into_iter());
    assert!(state().wants.is_empty());
}

/// The top's folds stand where their shards are not read, and give way to
/// the shard's leaves once it is; a shard it lists is wanted, one it does
/// not is read empty.
#[test]
fn top_folds_until_shard_read() {
    let _g = sys::serial();
    let (shard, other) = (shard_of(leaf(0, [5, 7, 2])), shard_of(leaf(2, [5, 7, 2])));
    // The folds of the written shard, as the store makes them.
    fresh();
    apply(decode_shard(shard, &shard_file(shard)).unwrap());
    let rows = top_of(&written()[..3]);
    assert_eq!(rows.len(), 2);
    fresh();
    arrived(TOP, Some(&top_file(&rows))).unwrap();
    for &(_, z, word) in &rows {
        assert_eq!(object().word_at([shard.at[0], shard.at[1], z]), Some(word));
    }
    assert!(held().is_empty());
    assert!(!holds(&shard.region()) && holds(&other.region()));
    want([(shard, Ask::PLAIN), (other, Ask::PLAIN)].into_iter());
    assert_eq!(state().wants.len(), 1);
    assert_eq!(state().wants[0].0, shard.subject());
    arrived(shard.subject(), Some(&shard_file(shard))).unwrap();
    assert!(holds(&shard.region()));
    assert_eq!(top_of(&written()[..3]), rows);
    assert_eq!(held().len(), 3);
}

/// A file that does not decode is refused: named, not wanted, and wanted
/// again as new once the host empties it.
#[test]
fn refused_file_named() {
    let _g = sys::serial();
    fresh();
    let shard = shard_of(leaf(0, [5, 7, 2]));
    arrived(TOP, Some(&top_file(&[(shard.subject(), 0, 0)]))).unwrap();
    let foreign = [format::SETTINGS, 1, 2, 3];
    assert!(arrived(shard.subject(), Some(&foreign)).is_err());
    assert_eq!(refused(), &[shard.subject()]);
    assert!(!holds(&shard.region()));
    want([(shard, Ask::PLAIN)].into_iter());
    assert!(state().wants.is_empty());
    assert!(on_emptied(shard.subject()));
    assert!(refused().is_empty() && !on_emptied(shard.subject()));
    want([(shard, Ask::PLAIN)].into_iter());
    assert_eq!(state().wants.len(), 1);
}

/// The writes since the last batch go up as one, and none is kept after.
#[test]
fn save_sends_one_batch() {
    let _g = sys::serial();
    fresh();
    for w in written() {
        send(w);
    }
    assert_eq!(state().unsent.len(), 4);
    state().save_due_ms = Some(0.0);
    save();
    assert!(state().unsent.is_empty() && state().save_due_ms.is_none());
}

/// The top's rows the store folds each case's writes to, applied in order
/// from empty, are the record's, which the source's fold is held to too.
#[test]
fn top_golden() {
    let _g = sys::serial();
    let g = Golden::load(GOLDEN);
    let mut cases = 0;
    g.each("case", |case| {
        fresh();
        let mut writes = Vec::new();
        g.each(&format!("{case}.write"), |w| {
            let (key, id) = g.s(w).split_once(' ').unwrap();
            writes.push(Write {
                key: u128::from_str_radix(key, 16).unwrap(),
                id: id.parse().unwrap(),
                ext: None,
            });
        });
        // A case may fold to no row.
        let mut rows = Vec::new();
        while g.has(&format!("{case}.top.{}", rows.len())) {
            let row = g.s(&format!("{case}.top.{}", rows.len()));
            let f: Vec<&str> = row.split(' ').collect();
            rows.push((
                f[0].parse::<u64>().unwrap(),
                f[1].parse::<i32>().unwrap(),
                f[2].parse::<u32>().unwrap(),
            ));
        }
        apply(writes.clone());
        assert_eq!(top_of(&writes), rows, "{case}");
        cases += 1;
    });
    assert!(cases > 0);
}
