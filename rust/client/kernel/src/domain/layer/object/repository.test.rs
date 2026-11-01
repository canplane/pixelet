// pxl-kernel/domain/layer/object/repository.test.rs

use pxl_spec::asset::marker::MARKER;
use pxl_spec::bake::constant::OBJECT_COARSEST;

use super::*;
use crate::sys;

/// A leaf at `OBJECT_COARSEST`, `x` shards east of the first.
fn leaf(x: i32, at: [i32; 3]) -> u128 {
    let shard = 1 << (OBJECT_COARSEST - SHARD_ZOOM);
    key_of(OBJECT_COARSEST, [at[0] + x * shard, at[1], at[2]])
}

/// The store and the shards read emptied.
fn fresh() {
    object().clear();
    *state() = State::default();
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

/// A shard's file reads back as its own leaves alone, records and all.
#[test]
fn shard_file_round_trip() {
    let _g = sys::serial();
    fresh();
    apply(written());
    let shard = shard_of(leaf(0, [5, 7, 2]));
    let file = encode(shard).unwrap();
    assert_eq!(file.first(), Some(&format::OBJECT));
    let own: Vec<_> = held()
        .into_iter()
        .filter(|&(key, _, _)| shard_of(key) == shard)
        .collect();
    assert_eq!(own.len(), 3);
    fresh();
    decode(shard, &file).unwrap();
    assert_eq!(held(), own);
}

/// A shard with no leaf is written empty: no file.
#[test]
fn empty_shard_written_empty() {
    let _g = sys::serial();
    fresh();
    apply(written());
    assert_eq!(
        encode(shard_of(leaf(2, [0, 0, 0]))).unwrap(),
        Vec::<u8>::new()
    );
}

/// Another format, or another shard's file, is refused whole.
#[test]
fn foreign_file_refused() {
    let _g = sys::serial();
    fresh();
    apply(written());
    let shard = shard_of(leaf(0, [5, 7, 2]));
    let mut file = encode(shard).unwrap();
    fresh();
    assert!(decode(shard_of(leaf(1, [5, 7, 2])), &file).is_err());
    file[0] = format::OBJECT.wrapping_add(1);
    assert!(decode(shard, &file).is_err());
    assert!(decode(shard, &[]).is_err());
    assert!(held().is_empty());
}

/// An absent file is a shard read with none, and an edit may stand there;
/// one that does not decode leaves its shard unread.
#[test]
fn absent_shard_read() {
    let _g = sys::serial();
    fresh();
    let (shard, other) = (shard_of(leaf(0, [5, 7, 2])), shard_of(leaf(1, [5, 7, 2])));
    let mut phases = Phases::default();
    assert!(!holds(&shard.region()));
    on_arrived(shard, None, &mut phases).unwrap();
    assert!(holds(&shard.region()));
    assert!(!holds(&other.region()));
    let both = Aabb {
        min: shard.region().min,
        max: other.region().max,
    };
    assert!(!holds(&both));
}

/// A write marks its shard, and a save writes each marked shard once.
#[test]
fn save_writes_marked_shards() {
    let _g = sys::serial();
    fresh();
    for w in written() {
        send(&w);
    }
    apply(written());
    assert_eq!(state().unsaved.len(), 2);
    state().save_due_ms = Some(0.0);
    save();
    assert!(state().unsaved.is_empty() && state().save_due_ms.is_none());
}
