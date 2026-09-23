// pxl-kernel/io/load.test.rs

use super::*;

#[test]
fn a_rotation_key_halfway_is_the_slerp_and_a_trs_matrix_places_a_point() {
    let ch = Channel {
        joint: 0,
        path: R,
        step: false,
        times: vec![0.0, 1.0],
        values: vec![0.0, 0.0, 0.0, 1.0, 0.0, 0.0, (0.5f32).sqrt(), (0.5f32).sqrt()],
    };
    let mut out = [0.0, 0.0, 0.0, 0.0, 0.0, 0.0, 1.0, 1.0, 1.0, 1.0];
    sample(&ch, 0.5, &mut out);
    // Halfway to a quarter turn about Z is an eighth.
    let half = (core::f32::consts::PI / 8.0).sin();
    assert!((out[5] - half).abs() < 1e-5);
    let m = mat_trs(&[1.0, 2.0, 3.0, 0.0, 0.0, (0.5f32).sqrt(), (0.5f32).sqrt(), 2.0, 2.0, 2.0]);
    // (1, 0, 0) scaled by 2 and turned a quarter about Z is (0, 2, 0), then moved.
    let x = m[0] + m[12];
    let y = m[1] + m[13];
    assert!((x - 1.0).abs() < 1e-5 && (y - 4.0).abs() < 1e-5, "{x} {y}");
}

/// The regression `Entry::Arrived` exists for: an asset whose bytes are in but
/// which nothing has parsed is not wanted. It used to be, because the read's
/// credit is released the moment the bytes land while the entry stayed
/// `Pending`, so every turn of a parse started another read of the same model —
/// one was read thirty-eight times, each read holding another block of the same
/// bytes.
#[test]
fn an_asset_whose_bytes_are_in_is_not_asked_for_again() {
    let _g = crate::global::serial();
    let l = loader();
    for n in 0..LOADS {
        l.gltf[n] = None;
        l.image[n] = None;
    }
    let load = Load::of(0).expect("a load");
    let n = load.number() as usize;

    loader().gltf[n] = Some(Entry::Pending);
    assert_eq!(load_wants().len(), 1, "wanted while nothing has it");

    loader().gltf[n] = Some(Entry::Arrived(7, 11));
    assert!(load_wants().is_empty(), "not wanted once the bytes are in");
    assert_eq!(next_arrival(), Some((n, 7, 11)), "and it is what the parse takes next");

    loader().gltf[n] = Some(Entry::Failed);
    assert!(load_wants().is_empty(), "nor once it is broken");
    assert_eq!(next_arrival(), None);
}
