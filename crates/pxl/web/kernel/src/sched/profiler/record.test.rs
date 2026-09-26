// pxl-kernel/sched/profiler/record.test.rs

use crate::sched::profiler::spec::GPU_SPAN_NAMES;

#[test]
fn the_profiler_names_the_renderers_spans_in_its_order() {
    assert_eq!(GPU_SPAN_NAMES, pxl_render::gpu::renderer::GPU_SPANS);
}
