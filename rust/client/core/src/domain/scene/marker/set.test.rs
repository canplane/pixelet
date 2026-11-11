// pxl-core/domain/scene/marker/set.test.rs

use super::*;
use pxl_render::frame::shelf::Paint;
use pxl_render::gpu::draw::{lists_bytes, set_lists};
use xpute_core::mem::heap::alloc;

/// A marker's instance holds its stand's Q3 position bit for bit, including
/// an X across the i32's seam and words whose bits read as a NaN, and it holds
/// nothing of the eye. The same markers lay the same words, and the set draws
/// the same record, wherever the eye is. So a moving eye rewrites no instance
/// and changes no draw (pxl-render gpu/draw.rs `Change`).
#[test]
fn a_marker_is_where_it_stands() {
    crate::on_core(|| {
        let stands = [
            [i32::MAX - 3, -0x0040_0001, 0x7fc0_0001],
            [i32::MIN + 3, 512, -1],
        ];
        let paint = Paint::Pill {
            text: "a".into(),
            fill: 0,
            ink: 0,
            logical_w: 24.0,
        };
        let slot = atlas().add(24.0, 30.0, paint) as u32;
        let lay = || {
            stands
                .iter()
                .flat_map(|&s| instance(s, [24.0, 30.0], slot, 1.0, 40.0))
                .collect::<Vec<u32>>()
        };
        let (was, now) = (lay(), lay());
        assert_eq!(was, now);
        for (k, s) in stands.iter().enumerate() {
            let at = k * INSTANCE_WORDS;
            assert_eq!(was[at..at + 3], s.map(|v| v as u32));
        }
        const LIVE: usize = 4;
        let (at, _) = alloc(lists_bytes(LIVE)).unwrap();
        // SAFETY: a block of the heap's, the lists' alone for the test's run.
        unsafe { set_lists(at, LIVE) };
        let mut list = DrawList::default();
        let mut set = Set::default();
        set.write(&mut list, &was);
        set.draw(&mut list);
        let l = list.take();
        assert_eq!(l.draws.of(PIPELINE_MARKER).len(), 1, "the set is drawn");
        assert_eq!(l.changes.len(), 1);
        set.draw(&mut list);
        assert!(list.take().changes.is_empty(), "drawn again as it was");
    });
}
