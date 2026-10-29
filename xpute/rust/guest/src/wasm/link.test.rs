// xpute-guest/wasm/link.test.rs

use super::*;

#[test]
fn mem_import_stack_first() {
    let cfg = LinkConfig {
        stack_bytes: 1 << 20,
        global_base: None,
        init_bytes: None,
    };
    assert_eq!(
        args(&cfg),
        ["--import-memory", "--stack-first", "-zstack-size=1048576"]
    );
}

#[test]
fn front_floor_after_stack() {
    let cfg = LinkConfig {
        stack_bytes: 1 << 20,
        global_base: Some(1 << 21),
        init_bytes: Some(4 * WASM_PAGE_BYTES),
    };
    assert_eq!(
        args(&cfg)[3..],
        ["--global-base=2097152", "--initial-memory=262144"]
    );
}

#[test]
#[should_panic(expected = "WebAssembly pages")]
fn floor_whole_pages() {
    args(&LinkConfig {
        stack_bytes: 1 << 20,
        global_base: None,
        init_bytes: Some(WASM_PAGE_BYTES + 1),
    });
}
