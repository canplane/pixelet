// pxl_core/render/draw.test.rs

use super::*;

#[test]
fn a_free_cancels_an_upload_not_yet_taken_and_survives_one_taken() {
    let mut d = DrawList::default();
    let a = d.upload(100, 40, 2, 3);
    let b = d.upload(200, 40, 2, 3);
    d.free(a);
    let lanes = d.take();
    assert_eq!((lanes[1], lanes[3]), (1, 0), "a cancelled, b taken, no free");
    d.free(b);
    let lanes = d.take();
    assert_eq!((lanes[1], lanes[3]), (0, 1));
    d.begin_frame();
    d.draw(PIPELINE_CHUNK, b, 2, 3, [1.0, 2.0, 3.0], 1.0, 1.0);
    let lanes = d.take();
    assert_eq!((lanes[5], lanes[7]), (1, 1));
}
