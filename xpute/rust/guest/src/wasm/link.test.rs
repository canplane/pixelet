// xpute-guest/wasm/link.test.rs

use super::*;

#[test]
fn a_module_imports_its_memory_and_puts_its_stack_first() {
    let plan = LinkPlan {
        stack_bytes: 1 << 20,
        global_base: None,
        initial_bytes: None,
    };
    assert_eq!(args(&plan), ["--import-memory", "--stack-first", "-zstack-size=1048576"]);
}

#[test]
fn a_front_and_a_floor_follow_the_stack() {
    let plan = LinkPlan {
        stack_bytes: 1 << 20,
        global_base: Some(1 << 21),
        initial_bytes: Some(4 * WASM_PAGE_BYTES),
    };
    assert_eq!(args(&plan)[3..], ["--global-base=2097152", "--initial-memory=262144"]);
}

#[test]
#[should_panic(expected = "WebAssembly pages")]
fn a_floor_is_whole_pages() {
    args(&LinkPlan {
        stack_bytes: 1 << 20,
        global_base: None,
        initial_bytes: Some(WASM_PAGE_BYTES + 1),
    });
}
