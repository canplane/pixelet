// pxl-render/frame/draw.test.rs

use super::*;

#[test]
fn a_free_cancels_an_upload_not_yet_taken_and_survives_one_taken() {
    let mut d = DrawList::default();
    let a = d.upload(100, 40, 2, 3);
    let b = d.upload(200, 40, 2, 3);
    d.free(a);
    let lists = d.take();
    assert_eq!((lists.uploads.len() / UPLOAD_WORDS, lists.frees.len()), (1, 0), "a cancelled, b taken, no free");
    d.free(b);
    let lists = d.take();
    assert_eq!((lists.uploads.len() / UPLOAD_WORDS, lists.frees.len()), (0, 1));
    d.begin_frame();
    d.draw(PIPELINE_CHUNK, b, 2, 3, [1.0, 2.0, 3.0], 1.0, 1.0);
    let lists = d.take();
    assert_eq!((lists.draws.len() / DRAW_WORDS, lists.uniforms.len() / UNIFORM_FLOATS), (1, 1));
}
