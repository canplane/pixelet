// pxl-render/gpu/pipeline.test.rs

use super::*;

/// WebGPU makes async pipelines in any order: each one made counts once it
/// is, not only those before the first still being made.
#[test]
fn every_made_pipeline_counts_in_whatever_order_it_is_made() {
    let mut p = PipelineCache::new();
    assert_eq!(p.share(), 0.0, "none asked, none made");
    p.pipeline_len = 4;
    p.words[1] = TICKET;
    p.words[3] = TICKET;
    assert!(!p.made());
    assert_eq!(p.share(), 0.5);
    p.words[0] = TICKET;
    p.words[2] = TICKET;
    assert!(p.made());
    assert_eq!(p.share(), 1.0);
}
